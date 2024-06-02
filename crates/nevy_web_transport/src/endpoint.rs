use std::{
    collections::{HashMap, VecDeque},
    net::SocketAddr,
};

use nevy_quic::prelude::*;
use transport_interface::*;

use crate::connection::{
    WebTransportConnection, WebTransportConnectionMut, WebTransportConnectionRef,
};

pub struct WebTransportEndpoint {
    quinn: QuinnEndpoint,
    connections: HashMap<QuinnConnectionId, WebTransportConnection>,
    events: VecDeque<EndpointEvent<WebTransportEndpoint>>,
}

impl WebTransportEndpoint {
    pub fn new(
        bind_addr: SocketAddr,
        config: Option<quinn_proto::EndpointConfig>,
        server_config: Option<quinn_proto::ServerConfig>,
    ) -> std::io::Result<Self> {
        let quinn = QuinnEndpoint::new(bind_addr, config, server_config)?;

        Ok(WebTransportEndpoint {
            quinn,
            connections: HashMap::new(),
            events: VecDeque::new(),
        })
    }
}

impl Endpoint for WebTransportEndpoint {
    type Connection<'a> = WebTransportConnectionMut<'a>;

    type ConnectionId = QuinnConnectionId;

    type ConnectInfo = (quinn_proto::ClientConfig, SocketAddr, String);

    fn update(&mut self) {
        self.quinn.update();

        while let Some(EndpointEvent {
            connection_id,
            event,
        }) = self.quinn.poll_event()
        {
            match event {
                ConnectionEvent::Connected => {
                    if !self.connections.contains_key(&connection_id) {
                        // will not exist in the case of an incoming connection
                        self.connections
                            .insert(connection_id, WebTransportConnection::new());
                    }

                    self.connection_mut(connection_id).unwrap().connected();
                }
                ConnectionEvent::Disconnected => {}
            }
        }

        for (&connection_id, web_transport) in self.connections.iter_mut() {
            let Some(quinn) = self.quinn.connection_mut(connection_id) else {
                continue;
            };

            WebTransportConnectionMut {
                quinn,
                web_transport,
                events: &mut self.events,
                connection_id,
            }
            .update();
        }
    }

    fn connection<'a>(
        &'a self,
        id: Self::ConnectionId,
    ) -> Option<<Self::Connection<'a> as transport_interface::ConnectionMut>::NonMut> {
        Some(WebTransportConnectionRef {
            quinn: self.quinn.connection(id)?,
            web_transport: self.connections.get(&id)?,
        })
    }

    fn connection_mut<'a>(&'a mut self, id: Self::ConnectionId) -> Option<Self::Connection<'a>> {
        Some(WebTransportConnectionMut {
            quinn: self.quinn.connection_mut(id)?,
            web_transport: self.connections.get_mut(&id)?,
            events: &mut self.events,
            connection_id: id,
        })
    }

    fn connect(&mut self, info: Self::ConnectInfo) -> Option<Self::ConnectionId> {
        let connection_id = self.quinn.connect(info)?;
        assert!(
            self.connections
                .insert(connection_id, WebTransportConnection::new())
                .is_none(),
            "Should not get duplicate connection id"
        );
        Some(connection_id)
    }

    fn poll_event(&mut self) -> Option<transport_interface::EndpointEvent<Self>>
    where
        Self: Sized,
    {
        self.events.pop_front()
    }
}
