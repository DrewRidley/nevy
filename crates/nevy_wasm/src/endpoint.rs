use transport_interface::Endpoint;

use crate::connection::WasmConnection;

pub struct WasmEndpoint {}

impl Endpoint for WasmEndpoint {
    type Connection<'c> = &'c mut WasmConnection
    where
        Self: 'c;

    type ConnectionId = u32;

    type ConnectDescription = String;

    /// With [web_sys::WebTransport], it is not possible to receive inbound connections.
    /// Browsers cannot currently accept connections from peers.
    type IncomingConnectionInfo<'i> = ();

    fn update(&mut self, handler: &mut impl transport_interface::EndpointEventHandler<Self>) {
        todo!()
    }

    fn connection<'c>(
        &'c self,
        id: Self::ConnectionId,
    ) -> Option<<Self::Connection<'c> as transport_interface::ConnectionMut>::NonMut<'c>> {
        None
    }

    fn connection_mut<'a>(&'a mut self, id: Self::ConnectionId) -> Option<Self::Connection<'a>> {
        todo!()
    }

    fn connect<'c>(
        &'c mut self,
        description: Self::ConnectDescription,
    ) -> Option<(Self::ConnectionId, Self::Connection<'c>)> {
        todo!()
    }
}
