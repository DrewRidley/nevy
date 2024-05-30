use std::{io::IoSliceMut, net::{SocketAddr, UdpSocket}, sync::Arc};

use bevy::{prelude::*, utils::hashbrown::HashMap};
use quinn_proto::{DatagramEvent, EndpointConfig, ServerConfig};
use quinn_udp::{RecvMeta, UdpSockRef, UdpSocketState};

use crate::{connection::{ConnectionId, ConnectionState}, EndpointEventHandler};



/// A single endpoint facilitating connection to peers through a raw UDP socket, facilitated through [quinn_proto].
///
/// This endpoint supports any platform which supports instantiation of a [UdpSocket]. For browsers consider [BrowserEndpoint].
/// If you plan on connecting to a WebTransport server or accepting connections from a WebTransport peer, ensure 'web_transport' is set to true.
#[derive(Component)]
pub struct EndpointState {
    /// The quinn endpoint state.
    pub(crate) endpoint: quinn_proto::Endpoint,
    /// The connection states for this endpoint.
    pub(crate) connections: HashMap<ConnectionId, ConnectionState>,
    pub(crate) socket: UdpSocket,
    pub(crate) socket_state: UdpSocketState,
    pub(crate) local_addr: SocketAddr,
    /// The endpoint configuration.
    pub(crate) config: EndpointConfig,
    /// the configuration used by this endpoint when accepting connections, if it is configured as a server
    pub(crate) server_config: Option<ServerConfig>,
}


pub struct ConnectionNotFound;

impl EndpointState {
    pub fn new(bind_addr: SocketAddr, config: Option<EndpointConfig>, server_config: Option<ServerConfig>) -> std::io::Result<Self> {
        let config = config.unwrap_or_default();

        let endpoint =  quinn_proto::Endpoint::new(
            Arc::new(config.clone()),
            server_config.clone().map(Arc::new),
            true,
            None
        );

        let socket = UdpSocket::bind(bind_addr)?;
        let socket_state = UdpSocketState::new(UdpSockRef::from(&socket))?;

        Ok(Self {
            endpoint,
            connections: HashMap::new(),
            local_addr: socket.local_addr()?,
            socket,
            socket_state,
            config,
            server_config,
        })
    }

    pub fn connect(&mut self, client_cfg: quinn_proto::ClientConfig, addr: SocketAddr, server_name: &str) -> Result<&mut ConnectionState, quinn_proto::ConnectError> {
        let (handle, connection) = self.endpoint.connect(std::time::Instant::now(), client_cfg, addr, server_name)?;
        let connection_id = ConnectionId(handle);

        let connection = ConnectionState::new(connection, connection_id);

        let bevy::utils::hashbrown::hash_map::Entry::Vacant(entry) = self.connections.entry(connection_id) else {
            panic!("Attempted to connect to a peer with same handle as existing one!");
        };

        Ok(entry.insert(connection))
    }

    pub fn connections(&self) -> impl Iterator<Item = ConnectionId> + '_ {
        self.connections.keys().copied()
    }

    pub fn get_connection(&self, connection_id: ConnectionId) -> Option<&ConnectionState> {
        self.connections.get(&connection_id)
    }

    pub fn get_connection_mut(&mut self, connection_id: ConnectionId) -> Option<&mut ConnectionState> {
        self.connections.get_mut(&connection_id)
    }

    /// public facing update method for the endpoint
    pub fn update(&mut self, buffers: &mut EndpointBuffers, event_handler: &mut impl EndpointEventHandler) {
        self.process_endpoint_datagrams(buffers, event_handler);
        self.poll_connections(buffers, event_handler);
    }



    /// reads datagrams from the socket and processes them
    fn process_endpoint_datagrams(&mut self, buffers: &mut EndpointBuffers, event_handler: &mut impl EndpointEventHandler) {
        let min_buffer_len = self.config.get_max_udp_payload_size().min(64 * 1024) as usize
            * self.socket_state.max_gso_segments()
            * quinn_udp::BATCH_SIZE;

        buffers.recv_buffer.resize(min_buffer_len, 0);
        let buffer_len = buffers.recv_buffer.len();

        let mut buffer_chunks = buffers.recv_buffer.chunks_mut(buffer_len / quinn_udp::BATCH_SIZE).map(IoSliceMut::new);

        //unwrap is safe here because we know we have at least one chunk based on established buffer len.
        let mut buffer_chunks: [IoSliceMut; quinn_udp::BATCH_SIZE] =  std::array::from_fn(|_| buffer_chunks.next().unwrap());

        let mut metas = [RecvMeta::default(); quinn_udp::BATCH_SIZE];

        loop {
            match self.socket_state.recv(UdpSockRef::from(&self.socket), &mut buffer_chunks, &mut metas) {
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

                            let Some(datagram_event) = self.endpoint.handle(
                                std::time::Instant::now(),
                                meta.addr,
                                meta.dst_ip,
                                ecn,
                                data.into(),
                                &mut buffers.send_buffer,
                            ) else {
                                continue;
                            };

                            self.process_datagram_event(&buffers.send_buffer, datagram_event, event_handler);
                        }
                    }
                },
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(other) => {
                    error!("Received an unexpected error while receiving endpoint datagrams: {:?}", other)
                }
            }
        }

        buffers.recv_buffer.clear();
        buffers.send_buffer.clear();
    }

    /// processes a datagram event generated after processing a datagram
    fn process_datagram_event(&mut self, send_buffer: &Vec<u8>, event: DatagramEvent, event_handler: &mut impl EndpointEventHandler) {
        match event {
            DatagramEvent::NewConnection(incoming) => {
                if self.server_config.is_none() {
                    warn!("Received an incoming connection request despite not being configured for listening on endpoint {}", self.local_addr);
                    return;
                }

                let mut response_buffer = Vec::new();

                let transmit = if event_handler.accept_connection(&incoming) {
                    // accept connection

                    match self.endpoint.accept(incoming, std::time::Instant::now(), &mut response_buffer, None) {
                        Ok((handle, connection)) => {
                            let connection_id = ConnectionId(handle);
                            // connection successful

                            let addr = connection.remote_address();
                            debug!("Accepting connection on endpoint {} with {}", self.local_addr, addr);

                            if let Some(existing_connection) = self.connections.insert(connection_id, ConnectionState::new(connection, connection_id)) {
                                error!("A new connection to {} was established on {} using a handle that already existed for connection {}", addr, self.local_addr, existing_connection.remote_address());
                            }

                            return;
                        },

                        Err(err) => {
                            // connection unsuccessful

                            info!("Failed to accept incoming connection: {:?}", err.cause);

                            let Some(transmit) = err.response else {
                                return;
                            };
                            // response needed

                            transmit
                        },
                    }
                } else {
                    // refuse connection

                    self.endpoint.refuse(incoming, &mut response_buffer)
                };

                trace!("Sending connection failure reason to peer: {}", transmit.destination);

                if let Err(err) = self.respond(&transmit, &response_buffer) {
                    error!("Failed to transmit connection response to {}: {}", transmit.destination, err);
                };
            },

            DatagramEvent::ConnectionEvent(handle, conn_event) => {
                let connection_id = ConnectionId(handle);

                let Some(connection) = self.connections.get_mut(&connection_id) else {
                    warn!("Received a connection event for a non-existent connection!");
                    return;
                };

                connection.handle(conn_event);
            },
            DatagramEvent::Response(transmit) => {
                if let Err(err) = self.respond(&transmit, send_buffer) {
                    error!("Failed to transmit a response: {}", err);
                };
            },
        }
    }

    pub(crate) fn respond(&mut self, transmit: &quinn_proto::Transmit, response_buffer: &[u8]) -> std::io::Result<()> {
        // convert to `quinn_proto` transmit
        let transmit = udp_transmit(transmit, response_buffer);

        // send if there is kernal buffer space, else drop it
        self.socket_state.send(UdpSockRef::from(&self.socket), &transmit)
    }

    /// updates connection state
    fn poll_connections(&mut self, buffers: &mut EndpointBuffers, event_handler: &mut impl EndpointEventHandler) {
        for connection in self.connections.values_mut() {
            connection.poll_connection(
                &mut self.endpoint,
                UdpSockRef::from(&self.socket),
                &mut self.socket_state,
                buffers,
                event_handler,
            );
        }
    }
}


//Buffers that can be reused by all endpoints for intermediate processing.
#[derive(Default)]
pub struct EndpointBuffers {
    pub(crate) send_buffer: Vec<u8>,
    pub(crate) recv_buffer: Vec<u8>,
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
