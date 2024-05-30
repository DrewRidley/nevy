use std::{io::IoSliceMut, net::{SocketAddr, UdpSocket}, sync::Arc};
use bevy::{ecs::system::SystemParam, prelude::*, utils::{hashbrown::HashMap, smallvec::SmallVec, HashSet}};
use bytes::{Buf, Bytes};
use quinn_proto::{Chunk, ConnectError, ConnectionEvent, ConnectionHandle, DatagramEvent, Endpoint, EndpointConfig, ReadableError, RecvStream, SendDatagramError, SendStream, ServerConfig, StreamId, VarInt, VarIntBoundsExceeded, WriteError};
use quinn_udp::{RecvMeta, UdpSockRef, UdpSocketState};
use web_transport_proto::{ConnectRequest, ConnectResponse};

use crate::connection::{ConnectionId, NativeConnection};



/// A single endpoint facilitating connection to peers through a raw UDP socket, facilitated through [quinn_proto].
///
/// This endpoint supports any platform which supports instantiation of a [UdpSocket]. For browsers consider [BrowserEndpoint].
/// If you plan on connecting to a WebTransport server or accepting connections from a WebTransport peer, ensure 'web_transport' is set to true.
#[derive(Component)]
pub struct NativeEndpoint {
    /// The quinn endpoint used to administer the connection.
    pub(crate) endpoint: quinn_proto::Endpoint,
    /// The state associated with every connection administered by this endpoint.
    pub(crate) connections: HashMap<ConnectionHandle, NativeConnection>,

    /// The UdpSocket and its associated state.
    pub(crate) sock: (UdpSocket, UdpSocketState),
    /// The configurations used by this endpoint
    pub(crate) cfg: (EndpointConfig, Option<ServerConfig>),
    /// Whether or not this endpoint should transmit/process additional WebTransport headers.
    /// Enable if one or more peer is expected to use WebTransport, or if you intend to connect to a WebTransport server.
    pub(crate) web_transport: bool
}


pub struct ConnectionNotFound;


impl NativeEndpoint {
    pub fn new(bind_addr: SocketAddr, cfg: Option<EndpointConfig>, server_cfg: Option<ServerConfig>, wt: bool) -> std::io::Result<Self> {
        let cfg = cfg.unwrap_or_default();

        let endpoint =  quinn_proto::Endpoint::new(
            Arc::new(cfg.clone()),
            server_cfg.clone().map(Arc::new),
            true,
            None
        );

        let socket = UdpSocket::bind(bind_addr)?;
        let socket_state = UdpSocketState::new(UdpSockRef::from(&socket))?;

        Ok(Self {
            endpoint,
            connections: HashMap::new(),
            sock: (socket, socket_state),
            cfg: (cfg, server_cfg),
            web_transport: wt
        })
    }

    pub fn connect(&mut self, client_cfg: quinn_proto::ClientConfig, addr: SocketAddr, server_name: &str) -> Result<(), quinn_proto::ConnectError> {
        let (handle, conn) = self.endpoint.connect(std::time::Instant::now(), client_cfg, addr, server_name)?;

        let c = NativeConnection::new(conn, handle);

        if self.connections.insert(handle, c).is_some() {
            error!("New connection attempt has same handle as existing one");
            panic!("Attempted to connect to a peer with same handle as existing one!");
        }

        Ok(())
    }

    /// Attempts to receive data up to 'max_len' from the provided stream.
    /// Will panic if the connection does not contain the specified stream.
    /// Returns a [ReadableError] if the data could not be read or there is no more data available.
    pub fn recv_from_stream(&mut self, conn: ConnectionId, stream: StreamId, max_len: usize) -> Result<Option<Chunk>, ReadableError> {
        let mut stream = self.connections.get_mut(&conn.0).unwrap().conn.recv_stream(stream);
        let mut chunks = stream.read(true)?;

        let data = chunks.next(max_len).unwrap();
        let _ = chunks.finalize();
        Ok(data)
    }

    /// Attempts to write the specified data to the specified stream.
    /// Will panic if the connection does not contain the specified stream.
    /// Will return a [WriteError] if the data could not immediately be written.
    pub fn write_to_stream(&mut self, conn: ConnectionId, stream: StreamId, data: &[u8]) -> Result<usize, WriteError> {
        let mut stream = self.connections.get_mut(&conn.0).unwrap().conn.send_stream(stream);
        stream.write(data)
    }

    /// Send an unreliable datagram to the peer.
    /// Will yield a [SendDatagramError] if the peer is congested or does not support datagrams.
    pub fn send_datagram(&mut self, conn: ConnectionId, data: Bytes) -> Result<(), SendDatagramError> {
        // Use the connection to send the datagram
        self.connections
            .get_mut(&conn.0)
            .ok_or_else(|| SendDatagramError::Disabled)?
            .conn.datagrams().send(data, false)
    }

    /// Receive an unreliable datagram from a peer.
    ///
    /// Will yield [ConnectionNotFound] if the specified connection does not exist.
    /// The inner value will be [None] if there are no datagrams to be read.
    pub fn recv_datagram(&mut self, conn: ConnectionId) -> Result<Option<Bytes>, ConnectionNotFound> {
        Ok(self.connections
            .get_mut(&conn.0)
            .ok_or_else(|| ConnectionNotFound)?
            .conn.datagrams().recv())
    }

    pub fn connections(&self) -> Vec<ConnectionId> {
        self.connections.keys().map(|k| ConnectionId(*k)).collect()
    }
}



fn process_datagram_event(ep: &mut NativeEndpoint, send_buffer: &mut Vec<u8>, event: DatagramEvent) {
    match event {
        DatagramEvent::NewConnection(new_conn) => {
            if ep.cfg.1.is_none() {
                warn!("Received an incoming connection request despite not being configured for listening on endpoint: {:?}", ep.sock.0.local_addr());
                return;
            }

            let mut send_buffer = Vec::new();

            match ep.endpoint.accept(new_conn, std::time::Instant::now(), &mut send_buffer, None) {
                Ok((handle, conn)) => {
                    debug!("Successfully negotiated new connection with peer: {}", conn.remote_address());
                    if ep.connections.insert(handle, NativeConnection::new(conn, handle)).is_some() {
                        error!("A new connection was established using a handle that already exists!");
                    }
                },
                Err(err) => {
                    info!("Received a failed connection attempt from peer with reason: {:?}", err.cause);
                    if let Some(tx) = err.response {
                        trace!("Sending connection failure reason to peer: {}", tx.destination);

                        let Err(e) = respond(ep, &tx, &send_buffer) else {
                            return;
                        };

                        error!("Received an error while attempting to accept connection: {}", e);
                    }
                },
            }
        },

        DatagramEvent::ConnectionEvent(handle, conn_event) => {
            let Some(conn) = ep.connections.get_mut(&handle) else {
                warn!("Received a connection event for a non-existent connection!");
                return;
            };

            conn.handle(conn_event);
        },
        DatagramEvent::Response(tx) => {
            let Err(e) = respond(ep, &tx, &send_buffer) else {
                return;
            };

            error!("Received an error while transmitting a response: {}", e);
        },
    }
}

fn process_endpoint_datagrams(endpoint: &mut NativeEndpoint, recv_buffer: &mut Vec<u8>, mut send_buffer: &mut Vec<u8>) {
    let min_buffer_len =
    endpoint.cfg.0.get_max_udp_payload_size().min(64 * 1024) as usize
    * endpoint.sock.1.max_gso_segments()
    * quinn_udp::BATCH_SIZE;

    recv_buffer.resize(min_buffer_len, 0);

    let buffer_len = recv_buffer.len();

    let mut buffer_chunks = recv_buffer.chunks_mut(buffer_len / quinn_udp::BATCH_SIZE).map(IoSliceMut::new);

    //unwrap is safe here because we know we have at least one chunk based on established buffer len.
    let mut buffer_chunks: [IoSliceMut; quinn_udp::BATCH_SIZE] =  std::array::from_fn(|_| buffer_chunks.next().unwrap());

    let mut metas = [RecvMeta::default(); quinn_udp::BATCH_SIZE];

    loop {
        match endpoint.sock.1.recv(UdpSockRef::from(&endpoint.sock.0), &mut buffer_chunks, &mut metas) {
            Ok(dgram_count) => {
                for (meta, buffer) in metas.iter().zip(buffer_chunks.iter()).take(dgram_count) {
                    let mut remaining_data = &buffer[0..meta.len];

                    while !remaining_data.is_empty() {
                        let stride_length = meta.stride.min(remaining_data.len());
                        let data = &remaining_data[0..stride_length];
                        remaining_data = &remaining_data[stride_length..];

                        let ecn = meta.ecn.map(|ecn| {
                            match ecn {
                                quinn_udp::EcnCodepoint::Ect0 => quinn_proto::EcnCodepoint::Ect0,
                                quinn_udp::EcnCodepoint::Ect1 => quinn_proto::EcnCodepoint::Ect1,
                                quinn_udp::EcnCodepoint::Ce => quinn_proto::EcnCodepoint::Ce,
                            }
                        });

                        let Some(dgram_event) = endpoint.endpoint.handle(
                            std::time::Instant::now(),
                            meta.addr,
                            meta.dst_ip,
                            ecn,
                            data.into(),
                            &mut send_buffer
                        ) else {
                            continue;
                        };

                        process_datagram_event(endpoint, send_buffer, dgram_event);
                    }
                }
            },
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
            Err(other) => {
                error!("Received an unexpected error while receiving endpoint datagrams: {:?}", other)
            }
        }
    }

    recv_buffer.clear();
    send_buffer.clear();
}


pub(crate) fn endpoint_poll_sys(
    mut ep_q: Query<&mut NativeEndpoint>,

    //Buffers that can be re-used by all endpoints for intermediate processing.
    mut send_buffer: Local<Vec<u8>>,
    mut recv_buffer: Local<Vec<u8>>
) {
    for mut endpoint in ep_q.iter_mut() {
        process_endpoint_datagrams(&mut endpoint, &mut recv_buffer, &mut send_buffer);
    }
}

pub(crate) fn udp_transmit<'a>(transmit: &'a quinn_proto::Transmit, buffer: &'a [u8]) -> quinn_udp::Transmit<'a> {
    quinn_udp::Transmit {
        destination: transmit.destination,
        ecn: transmit.ecn.map(|ecn| {
            match ecn {
                quinn_proto::EcnCodepoint::Ect0 => quinn_udp::EcnCodepoint::Ect0,
                quinn_proto::EcnCodepoint::Ect1 => quinn_udp::EcnCodepoint::Ect1,
                quinn_proto::EcnCodepoint::Ce => quinn_udp::EcnCodepoint::Ce,
            }
        }),
        contents: &buffer[0..transmit.size],
        segment_size: transmit.segment_size,
        src_ip: transmit.src_ip,
    }
}

pub(crate) fn respond(ep: &mut NativeEndpoint, transmit: &quinn_proto::Transmit, response_buffer: &[u8]) -> std::io::Result<()> {
    let socket = &mut ep.sock.0;
    let socket_state = &mut ep.sock.1;

    // convert to `quinn_proto` transmit
    let transmit = udp_transmit(transmit, response_buffer);

    // send if there is kernal buffer space, else drop it
    socket_state.send(UdpSockRef::from(&socket), &transmit)
}


use crate::connection::*;

pub struct NativeEndpointPlugin;

impl Plugin for NativeEndpointPlugin {
    fn build(&self, app: &mut App) {
        app
        .add_event::<NewWriteStream>()
        .add_event::<NewReadStream>()
        .add_event::<ClosedStream>()
        .add_event::<Connected>()
        .add_event::<Disconnected>()
        .add_systems(Update, (endpoint_poll_sys, connection_poll_sys));
    }
}
