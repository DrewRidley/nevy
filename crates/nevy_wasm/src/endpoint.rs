use std::collections::VecDeque;

use crate::connection::WasmConnection;
use slotmap::SlotMap;
use transport_interface::{ConnectionMut, Endpoint, EndpointEvent};
use web_sys::WebTransport;

slotmap::new_key_type! {
    pub struct WasmConnectionId;
}

#[derive(Default)]
pub struct WasmEndpoint {
    connections: slotmap::SlotMap<WasmConnectionId, WasmConnection>,
    events: VecDeque<EndpointEvent<WasmEndpoint>>,
}

impl WasmEndpoint {
    fn new() -> Self {
        WasmEndpoint {
            connections: SlotMap::default(),
            events: VecDeque::default(),
        }
    }
}

impl Endpoint for WasmEndpoint {
    type Connection<'c> = &'c mut WasmConnection;
    type ConnectionId = WasmConnectionId;
    type ConnectInfo = String;

    fn update(&mut self) {}

    fn connection<'c>(
        &'c self,
        id: Self::ConnectionId,
    ) -> Option<<Self::Connection<'c> as transport_interface::ConnectionMut>::NonMut<'c>> {
        self.connections.get(id)
    }

    fn connection_mut<'a>(&'a mut self, id: Self::ConnectionId) -> Option<Self::Connection<'a>> {
        self.connections.get_mut(id)
    }

    fn connect<'c>(
        &'c mut self,
        info: Self::ConnectInfo,
    ) -> Option<(Self::ConnectionId, Self::Connection<'c>)> {
        let wasm = WasmConnection {
            inner: WebTransport::new(&info).ok()?,
        };

        Some(self.connections.insert(wasm), wasm);
    }

    fn poll_event(&mut self) -> Option<transport_interface::EndpointEvent<Self>>
    where
        Self: Sized,
    {
        None
    }
}
