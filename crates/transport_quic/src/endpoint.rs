use crate::{connection::*, QuinnContext};
use log::{debug, error, trace, warn};
use quinn_proto::{
    ConnectionEvent, ConnectionHandle, DatagramEvent, EndpointConfig, Incoming, ServerConfig,
};
use quinn_udp::{UdpSockRef, UdpSocketState};
use std::{
    collections::HashMap,
    io::IoSliceMut,
    net::{SocketAddr, UdpSocket},
    sync::Arc,
};
use transport_interface::*;

/// A transport endpoint facilitated using quinn_proto through a low-level polling methodology.
///
/// Does not use async runtimes and is primarily built for the use in the bevy game engine.
/// To facilitate connections with a browser or other WebTransport peer, use the 'transport_wt' crate.
pub struct QuinnEndpoint {
    endpoint: quinn_proto::Endpoint,
    socket: UdpSocket,
    socket_state: quinn_udp::UdpSocketState,
    local_addr: SocketAddr,
    connections: HashMap<QuinnConnectionId, QuinnConnection>,
    config: quinn_proto::EndpointConfig,
    server_config: Option<quinn_proto::ServerConfig>,
}

impl QuinnEndpoint {
    pub fn new(
        bind_addr: SocketAddr,
        config: Option<EndpointConfig>,
        server_config: Option<ServerConfig>,
    ) -> std::io::Result<Self> {
        let socket = UdpSocket::bind(bind_addr)?;
        let socket_state = UdpSocketState::new(UdpSockRef::from(&socket))?;
        let local_addr = socket.local_addr()?;

        let config = config.unwrap_or_default();
        let endpoint = quinn_proto::Endpoint::new(
            Arc::new(config.clone()),
            server_config.clone().map(Arc::new),
            true,
            None,
        );

        Ok(Self {
            endpoint,
            connections: HashMap::new(),
            local_addr,
            socket,
            socket_state,
            config,
            server_config,
        })
    }

    fn receive_datagrams(&mut self, context: &mut QuinnContext) {
        let mut recv_buffer = std::mem::take(&mut context.recv_buffer);
        let mut send_buffer = std::mem::take(&mut context.send_buffer);

        let min_buffer_len = self.config.get_max_udp_payload_size().min(64 * 1024) as usize
            * self.socket_state.max_gso_segments()
            * quinn_udp::BATCH_SIZE;

        recv_buffer.resize(min_buffer_len, 0);
        let buffer_len = context.recv_buffer.len();

        let mut buffer_chunks = recv_buffer
            .chunks_mut(buffer_len / quinn_udp::BATCH_SIZE)
            .map(IoSliceMut::new);

        //unwrap is safe here because we know we have at least one chunk based on established buffer len.
        let mut buffer_chunks: [IoSliceMut; quinn_udp::BATCH_SIZE] =
            std::array::from_fn(|_| buffer_chunks.next().unwrap());

        let mut metas = [quinn_udp::RecvMeta::default(); quinn_udp::BATCH_SIZE];
        loop {
            match self.socket_state.recv(
                UdpSockRef::from(&self.socket),
                &mut buffer_chunks,
                &mut metas,
            ) {
                Ok(datagram_count) => {
                    self.process_packet(
                        context,
                        datagram_count,
                        &buffer_chunks,
                        &metas,
                        &mut send_buffer,
                    );
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(other) => {
                    error!(
                        "Received an unexpected error while receiving endpoint datagrams: {:?}",
                        other
                    )
                }
            }
        }

        send_buffer.clear();
        recv_buffer.clear();
        context.send_buffer = send_buffer;
        context.recv_buffer = recv_buffer;
    }

    fn process_packet(
        &mut self,
        context: &mut QuinnContext,
        datagram_count: usize,
        buffer_chunks: &[IoSliceMut; quinn_udp::BATCH_SIZE],
        metas: &[quinn_udp::RecvMeta; quinn_udp::BATCH_SIZE],
        send_buffer: &mut Vec<u8>,
    ) {
        trace!(
            "Received {datagram_count} UDP datagrams on {}.",
            self.local_addr
        );
        for (meta, buffer) in metas.iter().zip(buffer_chunks.iter()).take(datagram_count) {
            let mut remaining_data = &buffer[0..meta.len];
            while !remaining_data.is_empty() {
                let stride_length = meta.stride.min(remaining_data.len());
                let data = &remaining_data[0..stride_length];
                remaining_data = &remaining_data[stride_length..];

                let ecn = meta.ecn.map(|ecn| match ecn {
                    quinn_udp::EcnCodepoint::Ect0 => quinn_proto::EcnCodepoint::Ect0,
                    quinn_udp::EcnCodepoint::Ect1 => quinn_proto::EcnCodepoint::Ect1,
                    quinn_udp::EcnCodepoint::Ce => quinn_proto::EcnCodepoint::Ce,
                });

                trace!("Handling UDP datagram with endpoint {}.", self.local_addr);
                let Some(datagram_event) = self.endpoint.handle(
                    std::time::Instant::now(),
                    meta.addr,
                    meta.dst_ip,
                    ecn,
                    data.into(),
                    send_buffer,
                ) else {
                    continue;
                };

                self.process_datagram_event(context, send_buffer, datagram_event);
            }
        }
    }

    pub fn respond(
        &mut self,
        transmit: &quinn_proto::Transmit,
        response_buffer: &[u8],
    ) -> std::io::Result<()> {
        // convert to `quinn_proto` transmit
        let transmit = udp_transmit(transmit, response_buffer);

        // send if there is kernal buffer space, else drop it
        self.socket_state
            .send(UdpSockRef::from(&self.socket), &transmit)
    }

    fn process_datagram_event(
        &mut self,
        context: &mut QuinnContext,
        send_buffer: &mut Vec<u8>,
        event: DatagramEvent,
    ) {
        let transmit = match event {
            DatagramEvent::NewConnection(incoming) => {
                self.accept_incoming(context, incoming, send_buffer)
            }
            DatagramEvent::ConnectionEvent(handle, event) => {
                self.process_connection_event(context, handle, event);
                None
            }
            DatagramEvent::Response(transmit) => Some(transmit),
        };

        if let Some(transmit) = transmit {
            // the transmit failing is equivelant to dropping due to congestion
            let _ = self.respond(&transmit, send_buffer);
        }
    }

    fn accept_incoming(
        &mut self,
        context: &mut QuinnContext,
        incoming: Incoming,
        response_buffer: &mut Vec<u8>,
    ) -> Option<quinn_proto::Transmit> {
        if self.server_config.is_none() {
            warn!("{} attempted to connect to endpoint {} but the endpoint isn't configured as a server", incoming.remote_address(), self.local_addr);
            return Some(self.endpoint.refuse(incoming, response_buffer));
        }

        if !context.accept_connection(&incoming) {
            return Some(self.endpoint.refuse(incoming, response_buffer));
        };

        match self
            .endpoint
            .accept(incoming, std::time::Instant::now(), response_buffer, None)
        {
            Err(err) => return err.response,
            Ok((handle, connection)) => {
                let connection_id = QuinnConnectionId(handle);

                let connection = QuinnConnection::new(connection, connection_id);
                assert!(
                    self.connections.insert(connection_id, connection).is_none(),
                    "Connection handle should not be a duplicate"
                );

                None
            }
        }
    }

    fn process_connection_event(
        &mut self,
        context: &mut QuinnContext,
        handle: ConnectionHandle,
        event: ConnectionEvent,
    ) {
        let Some(connection) = self.connection_mut(QuinnConnectionId(handle), context) else {
            error!(
                "Endpoint {} returned a connection event about a connection that doesn't) exist",
                self.local_addr
            );
            return;
        };

        connection.process_event(event);
    }

    fn update_connections(&mut self, context: &mut QuinnContext) {
        let max_gso_datagrams = self.socket_state.gro_segments();

        for (&connection_id, connection) in self.connections.iter_mut() {
            //Return transmission to endpoint if there is one.
            context.send_buffer.clear();
            if let Some(transmit) = connection.connection.poll_transmit(
                std::time::Instant::now(),
                max_gso_datagrams,
                &mut context.send_buffer,
            ) {
                // the transmit failing is equivelant to dropping due to congestion
                let _ = self.socket_state.send(
                    quinn_udp::UdpSockRef::from(&self.socket),
                    &udp_transmit(&transmit, &context.send_buffer),
                );
            }

            connection.poll_timeouts(context);

            while let Some(endpoint_event) = connection.connection.poll_endpoint_events() {
                if let Some(conn_event) =
                    self.endpoint.handle_event(connection_id.0, endpoint_event)
                {
                    connection.process_event(conn_event);
                }
            }

            connection.poll_events(context);
        }
    }
}

impl Endpoint for QuinnEndpoint {
    type Context = QuinnContext;

    type Connection = QuinnConnection;

    type ConnectInfo = (quinn_proto::ClientConfig, SocketAddr, String);

    fn update(&mut self, context: &mut Self::Context) {
        self.receive_datagrams(context);
        self.update_connections(context);
    }

    fn connection(
        &self,
        id: ConnectionId<Self>,
        _context: &Self::Context,
    ) -> Option<&Self::Connection> {
        self.connections.get(&id)
    }

    fn connection_mut(
        &mut self,
        id: ConnectionId<Self>,
        _context: &mut Self::Context,
    ) -> Option<&mut Self::Connection> {
        self.connections.get_mut(&id)
    }

    fn connect(
        &mut self,
        context: &mut Self::Context,
        info: Self::ConnectInfo,
    ) -> Option<ConnectionId<Self>> {
        let (handle, connection) = self
            .endpoint
            .connect(std::time::Instant::now(), info.0, info.1, info.2.as_str())
            .ok()?;

        let connection_id = QuinnConnectionId(handle);

        assert!(
            self.connections
                .insert(
                    connection_id,
                    QuinnConnection::new(connection, connection_id)
                )
                .is_none(),
            "Connection handle should not be a duplicate"
        );

        Some(connection_id)
    }

    fn poll_event(&mut self, context: &mut Self::Context) -> Option<EndpointEvent<Self>> {
        context.events.pop_front()
    }
}

fn udp_transmit<'a>(
    transmit: &'a quinn_proto::Transmit,
    buffer: &'a [u8],
) -> quinn_udp::Transmit<'a> {
    quinn_udp::Transmit {
        destination: transmit.destination,
        ecn: transmit.ecn.map(|ecn| match ecn {
            quinn_proto::EcnCodepoint::Ect0 => quinn_udp::EcnCodepoint::Ect0,
            quinn_proto::EcnCodepoint::Ect1 => quinn_udp::EcnCodepoint::Ect1,
            quinn_proto::EcnCodepoint::Ce => quinn_udp::EcnCodepoint::Ce,
        }),
        contents: &buffer[0..transmit.size],
        segment_size: transmit.segment_size,
        src_ip: transmit.src_ip,
    }
}
