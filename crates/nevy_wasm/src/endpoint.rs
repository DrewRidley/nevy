use slotmap::SlotMap;
use transport_interface::Endpoint;

use crate::connection::{WasmConnection, WasmConnectionId};

pub struct WasmEndpoint {
    connections: SlotMap<WasmConnectionId, WasmConnection>,
}

impl Endpoint for WasmEndpoint {
    type Connection<'c> = &'c mut WasmConnection;

    type ConnectionId = WasmConnectionId;

    type ConnectDescription = String;

    type IncomingConnectionInfo<'i> = ();

    fn update(&mut self, handler: &mut impl transport_interface::EndpointEventHandler<Self>) {
        todo!()
    }

    fn connection<'c>(
        &'c self,
        id: Self::ConnectionId,
    ) -> Option<<Self::Connection<'c> as transport_interface::ConnectionMut>::NonMut<'c>> {
        todo!()
    }

    fn connection_mut<'a>(&'a mut self, id: Self::ConnectionId) -> Option<Self::Connection<'a>> {
        todo!()
    }

    fn connect<'c>(
        &'c mut self,
        info: Self::ConnectDescription,
    ) -> Option<(Self::ConnectionId, Self::Connection<'c>)> {
        todo!()
    }
}
