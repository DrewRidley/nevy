use std::{
    collections::VecDeque,
    future::IntoFuture,
    task::{Context, Waker},
};

use crate::connection::WasmConnection;
use slotmap::SlotMap;
use transport_interface::{ConnectionMut, Endpoint, EndpointEvent};
use wasm_bindgen_futures::JsFuture;
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

    fn update(&mut self) {
        for (connection_id, connection) in self.connections.iter_mut() {
            connection.update(connection_id, &mut self.events);
        }
    }

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
        let wt = WebTransport::new(&info).ok()?;
        let future = JsFuture::from(wt.ready());
        let context = Context::from_waker(Waker::noop());
        let future = Box::pin(future);

        let wasm = WasmConnection {
            inner: wt,
            connect_future: Some(future),
        };

        let connection_id = self.connections.insert(wasm);

        Some((connection_id, self.connection_mut(connection_id).unwrap()))
    }

    fn poll_event(&mut self) -> Option<transport_interface::EndpointEvent<Self>>
    where
        Self: Sized,
    {
        None
    }
}
