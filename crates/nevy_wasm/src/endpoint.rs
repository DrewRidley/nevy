use crate::connection::*;
use std::collections::HashMap;
use std::net::SocketAddr;
use transport_interface::*;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;
use web_sys::WebTransport;

/// A transport endpoint facilitated using WebTransport.
///
/// Uses async methods and should be polled manually within update.
pub struct WebTransportEndpoint {
    endpoint: WebTransport,
    local_addr: SocketAddr,
    connections: HashMap<WebTransportConnectionId, WebTransportConnection>,
}

impl WebTransportEndpoint {
    /// Creates a new endpoint, facilitated through WebTransport.
    ///
    /// Requires a bind_addr (consider '0.0.0.0:0' for clients).
    pub fn new(bind_addr: SocketAddr, url: &str) -> Result<Self, JsValue> {
        let endpoint = WebTransport::new(url)?;

        Ok(Self {
            endpoint,
            connections: HashMap::new(),
            local_addr: bind_addr,
        })
    }

    async fn handle_ready(&self) {
        let ready_promise = self.endpoint.ready();
        JsFuture::from(ready_promise).await.unwrap();
    }

    async fn handle_closed(&self) {
        let closed_promise = self.endpoint.closed();
        JsFuture::from(closed_promise).await.unwrap();
    }

    async fn process_event(&mut self, handler: &mut impl EndpointEventHandler<Self>) {
        // Example of handling ready and closed events
        self.handle_ready().await;
        self.handle_closed().await;
    }

    async fn update_connections(&mut self, handler: &mut impl EndpointEventHandler<Self>) {
        for connection in self.connections.values_mut() {
            connection.poll_timeouts().await;
            connection.poll_events(handler).await;
            connection.accept_streams().await;
        }
    }
}

impl Endpoint for WebTransportEndpoint {
    type Connection<'a> = &'a mut WebTransportConnection;
    type ConnectionId = WebTransportConnectionId;
    type ConnectDescription = String;
    type IncomingConnectionInfo<'a> = String;

    fn update(&mut self, handler: &mut impl EndpointEventHandler<Self>) {
        wasm_bindgen_futures::spawn_local(async move {
            self.process_event(handler).await;
            self.update_connections(handler).await;
        });
    }

    fn connection<'c>(
        &'c self,
        id: Self::ConnectionId,
    ) -> Option<<Self::Connection<'c> as ConnectionMut>::NonMut<'c>> {
        self.connections.get(&id)
    }

    fn connection_mut<'c>(&'c mut self, id: Self::ConnectionId) -> Option<Self::Connection<'c>> {
        self.connections.get_mut(&id)
    }

    fn connect<'c>(
        &'c mut self,
        description: Self::ConnectDescription,
    ) -> Option<(Self::ConnectionId, Self::Connection<'c>)> {
        let connection = WebTransport::new(&description).ok()?;
        let connection_id = WebTransportConnectionId::new();

        let connection = WebTransportConnection::new(connection, connection_id);
        assert!(
            self.connections.insert(connection_id, connection).is_none(),
            "Connection handle should not be a duplicate"
        );

        Some((connection_id, &mut self.connections[&connection_id]))
    }

    fn disconnect(&mut self, id: Self::ConnectionId) -> Result<(), ()> {
        if let Some(mut connection) = self.connection_mut(id) {
            connection.disconnect();
            Ok(())
        } else {
            Err(())
        }
    }
}
