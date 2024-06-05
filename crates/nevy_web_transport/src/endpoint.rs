use std::{collections::HashMap, net::SocketAddr};

use nevy_quic::prelude::*;
use transport_interface::*;

use crate::connection::{
    WebTransportConnection, WebTransportConnectionMut, WebTransportConnectionRef,
};

pub struct WebTransportEndpoint {
    quinn: QuinnEndpoint,
    connections: HashMap<QuinnConnectionId, WebTransportConnection>,
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
        })
    }
}

struct QuinnEventHandler<'a> {
    connections: Vec<QuinnConnectionId>,
    on_request: &'a mut dyn EndpointEventHandler<WebTransportEndpoint>,
}

impl<'a> EndpointEventHandler<QuinnEndpoint> for QuinnEventHandler<'a> {
    fn connection_request(&mut self, incoming: &quinn_proto::Incoming) -> bool {
        self.on_request.connection_request(incoming)
    }

    fn connected(&mut self, connection_id: QuinnConnectionId) {
        self.connections.push(connection_id);
    }

    fn disconnected(&mut self, _connection_id: QuinnConnectionId) {
        todo!()
    }
}

impl Endpoint for WebTransportEndpoint {
    type Connection<'a> = WebTransportConnectionMut<'a>;

    type ConnectionId = QuinnConnectionId;

    type ConnectInfo<'a> = (quinn_proto::ClientConfig, SocketAddr, &'a str);

    type IncomingConnectionInfo<'a> = &'a quinn_proto::Incoming;

    fn update(&mut self, handler: &mut impl EndpointEventHandler<WebTransportEndpoint>) {
        let mut quinn_handler = QuinnEventHandler {
            connections: Vec::new(),
            on_request: handler,
        };

        self.quinn.update(&mut quinn_handler);

        for connection_id in quinn_handler.connections {
            if !self.connections.contains_key(&connection_id) {
                // will not exist in the case of an incoming connection
                self.connections
                    .insert(connection_id, WebTransportConnection::new());
            }

            self.connection_mut(connection_id).unwrap().connected();
        }

        for (&connection_id, web_transport) in self.connections.iter_mut() {
            let Some(quinn) = self.quinn.connection_mut(connection_id) else {
                continue;
            };

            WebTransportConnectionMut {
                quinn,
                web_transport,
                connection_id,
            }
            .update(handler);
        }
    }

    fn connection<'c>(
        &'c self,
        id: Self::ConnectionId,
    ) -> Option<<Self::Connection<'c> as transport_interface::ConnectionMut>::NonMut<'c>> {
        Some(WebTransportConnectionRef {
            quinn: self.quinn.connection(id)?,
        })
    }

    fn connection_mut<'c>(&'c mut self, id: Self::ConnectionId) -> Option<Self::Connection<'c>> {
        Some(WebTransportConnectionMut {
            quinn: self.quinn.connection_mut(id)?,
            web_transport: self.connections.get_mut(&id)?,
            connection_id: id,
        })
    }

    fn connect<'c, 'a>(
        &'c mut self,
        info: Self::ConnectInfo<'a>,
    ) -> Option<(Self::ConnectionId, Self::Connection<'c>)> {
        let (connection_id, quinn) = self.quinn.connect(info)?;

        let std::collections::hash_map::Entry::Vacant(entry) =
            self.connections.entry(connection_id)
        else {
            panic!("Connection handle should not be a duplicate");
        };

        let web_transport = entry.insert(WebTransportConnection::new());

        Some((
            connection_id,
            WebTransportConnectionMut {
                quinn,
                web_transport,
                connection_id,
            },
        ))
    }
}
