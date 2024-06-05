use crate::connection::*;
use log::*;
use quinn_proto::{ConnectionEvent, DatagramEvent, Incoming};
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
/// To facilitate connections with a browser or other WebTransport peer, use the 'nevy_web_transport' crate.
pub struct QuinnEndpoint {
    endpoint: quinn_proto::Endpoint,
    socket: UdpSocket,
    socket_state: quinn_udp::UdpSocketState,
    local_addr: SocketAddr,
    connections: HashMap<QuinnConnectionId, QuinnConnection>,
    config: quinn_proto::EndpointConfig,
    server_config: Option<quinn_proto::ServerConfig>,
    recv_buffer: Vec<u8>,
    send_buffer: Vec<u8>,
}

impl QuinnEndpoint {
    /// Creates a new endpoint, facilitated through Quinn.
    ///
    /// Requires a bind_addr (consider '0.0.0.0:0' for clients).
    /// 'config' or 'server_config' can be [None] but never both, since an endpoint must behave as a client, server or both.
    pub fn new(
        bind_addr: SocketAddr,
        config: Option<quinn_proto::EndpointConfig>,
        server_config: Option<quinn_proto::ServerConfig>,
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
            recv_buffer: Vec::new(),
            send_buffer: Vec::new(),
        })
    }

    // Receive UDP datagrams for internal processing.
    fn receive_datagrams(&mut self, handler: &mut impl EndpointEventHandler<Self>) {
        let mut recv_buffer = std::mem::take(&mut self.recv_buffer);

        let min_buffer_len = self.config.get_max_udp_payload_size().min(64 * 1024) as usize
            * self.socket_state.max_gso_segments()
            * quinn_udp::BATCH_SIZE;

        recv_buffer.resize(min_buffer_len, 0);
        let buffer_len = recv_buffer.len();

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
                    self.process_packet(datagram_count, &buffer_chunks, &metas, handler);
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

        self.send_buffer.clear();
        recv_buffer.clear();
        self.recv_buffer = recv_buffer;
    }

    // Process a single UDP packet through the endpoint.
    fn process_packet(
        &mut self,
        datagram_count: usize,
        buffer_chunks: &[IoSliceMut; quinn_udp::BATCH_SIZE],
        metas: &[quinn_udp::RecvMeta; quinn_udp::BATCH_SIZE],
        handler: &mut impl EndpointEventHandler<Self>,
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
                    &mut self.send_buffer,
                ) else {
                    continue;
                };

                self.process_datagram_event(datagram_event, handler);
            }
        }
    }

    // Process an event associated with a datagram.
    fn process_datagram_event(
        &mut self,
        event: DatagramEvent,
        handler: &mut impl EndpointEventHandler<Self>,
    ) {
        let transmit = match event {
            DatagramEvent::NewConnection(incoming) => self.accept_incoming(incoming, handler),
            DatagramEvent::ConnectionEvent(handle, event) => {
                let connection_id = QuinnConnectionId(handle);
                self.process_connection_event(connection_id, event);
                None
            }
            DatagramEvent::Response(transmit) => Some(transmit),
        };

        if let Some(transmit) = transmit {
            // the transmit failing is equivelant to dropping due to congestion
            let _ = self.socket_state.send(
                quinn_udp::UdpSockRef::from(&self.socket),
                &udp_transmit(&transmit, &self.send_buffer),
            );
        }
    }

    // Accept an incoming connection and optionally return data to transmit to callee.
    fn accept_incoming(
        &mut self,
        incoming: Incoming,
        handler: &mut impl EndpointEventHandler<Self>,
    ) -> Option<quinn_proto::Transmit> {
        if self.server_config.is_none() {
            warn!("{} attempted to connect to endpoint {} but the endpoint isn't configured as a server", incoming.remote_address(), self.local_addr);
            return Some(self.endpoint.refuse(incoming, &mut self.send_buffer));
        }

        if !handler.connection_request(&incoming) {
            return Some(self.endpoint.refuse(incoming, &mut self.send_buffer));
        };

        match self.endpoint.accept(
            incoming,
            std::time::Instant::now(),
            &mut self.send_buffer,
            None,
        ) {
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

    // Process an event associated with a connection.
    fn process_connection_event(
        &mut self,
        connection_id: QuinnConnectionId,
        event: ConnectionEvent,
    ) {
        let Some(connection) = self.connection_mut(connection_id) else {
            error!(
                "Endpoint {} returned a connection event about a connection that doesn't) exist",
                self.local_addr
            );
            return;
        };

        connection.process_event(event);
    }

    // Update the internal connections, polling/advancing their state.
    fn update_connections(&mut self, handler: &mut impl EndpointEventHandler<Self>) {
        let max_gso_datagrams = self.socket_state.gro_segments();

        for (&connection_id, connection) in self.connections.iter_mut() {
            //Return transmission to endpoint if there is one.
            self.send_buffer.clear();
            if let Some(transmit) = connection.connection.poll_transmit(
                std::time::Instant::now(),
                max_gso_datagrams,
                &mut self.send_buffer,
            ) {
                // the transmit failing is equivelant to dropping due to congestion
                let _ = self.socket_state.send(
                    quinn_udp::UdpSockRef::from(&self.socket),
                    &udp_transmit(&transmit, &self.send_buffer),
                );
            }

            connection.poll_timeouts();

            while let Some(endpoint_event) = connection.connection.poll_endpoint_events() {
                if let Some(conn_event) =
                    self.endpoint.handle_event(connection_id.0, endpoint_event)
                {
                    connection.process_event(conn_event);
                }
            }

            connection.poll_events(handler);

            connection.accept_streams();
        }
    }
}

impl Endpoint for QuinnEndpoint {
    type Connection<'a> = &'a mut QuinnConnection;

    type ConnectionId = QuinnConnectionId;

    type ConnectInfo<'a> = (quinn_proto::ClientConfig, SocketAddr, &'a str);

    type IncomingConnectionInfo<'a> = &'a quinn_proto::Incoming;

    // Processes timeouts, received datagrams and other events.
    fn update(&mut self, handler: &mut impl EndpointEventHandler<Self>) {
        self.receive_datagrams(handler);
        self.update_connections(handler);
    }

    // Retrieve a reference to a particular [QuinnConnection].
    fn connection<'c>(
        &'c self,
        id: Self::ConnectionId,
    ) -> Option<<Self::Connection<'c> as ConnectionMut>::NonMut<'c>> {
        self.connections.get(&id)
    }

    // Returns a mutable reference to a particular [QuinnConnection].
    fn connection_mut<'c>(&'c mut self, id: Self::ConnectionId) -> Option<Self::Connection<'c>> {
        self.connections.get_mut(&id)
    }

    /// Connect to a peer, specified by [Self::ConnectInfo].
    fn connect<'c, 'a>(
        &'c mut self,
        info: Self::ConnectInfo<'a>,
    ) -> Option<(Self::ConnectionId, Self::Connection<'c>)> {
        let (handle, connection) = self
            .endpoint
            .connect(std::time::Instant::now(), info.0, info.1, info.2)
            .ok()?;

        let connection_id = QuinnConnectionId(handle);

        let std::collections::hash_map::Entry::Vacant(entry) =
            self.connections.entry(connection_id)
        else {
            panic!("Connection handle should not be a duplicate");
        };

        let connection = entry.insert(QuinnConnection::new(connection, connection_id));

        Some((connection_id, connection))
    }

    // Disconnect a specific connection.
    //
    // Returns [Err()] if the connection never existed.
    fn disconnect(&mut self, id: Self::ConnectionId) -> Result<(), ()> {
        if let Some(mut connection) = self.connection_mut(id) {
            connection.disconnect();
            Ok(())
        } else {
            Err(())
        }
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
