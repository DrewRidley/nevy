use std::{
    collections::{HashSet, VecDeque},
    default,
    future::Future,
    pin::Pin,
};

use futures_lite::stream::race;
use slotmap::new_key_type;
use transport_interface::{ConnectionMut, ConnectionRef, StreamEvent};
use web_sys::{
    wasm_bindgen::{closure::Closure, JsCast, JsValue},
    WebTransportBidirectionalStream, WebTransportReceiveStream, WebTransportSendStream,
};
use web_transport_wasm::Session;

use crate::{
    reader::{Reader, WebError},
    stream::WasmStreamId,
};

new_key_type! {
    pub struct WasmConnectionId;
}

struct ConnectedWasmSession {
    /// The underlying wasm session used to establish new streams, read, or write data.
    session: web_sys::WebTransport,
    /// A future that accepts either unidirectional or bidirectional streams from the peer.
    /// This future will always exist in the [WasmSession::Connected] state and must be polled accordingly.
    accept_future: Box<dyn Future<Output = ()>>,

    /// A future used to populate the internal recv buffers from the async methods.
    recv_future: Box<dyn Future<Output = ()>>,
    /// A future used to progress all outstanding writes.
    send_future: Box<dyn Future<Output = ()>>,
}

/// The wasm session state.
enum WasmSession {
    /// The session is disconnected and awaiting a new connection attempt.
    Disconnected,
    /// The session is currently connecting with the specified future that must be polled to progress the connection.
    Connecting(Box<dyn Future<Output = Result<Session, WebError>>>),
    /// The session is currently connected.
    Connected(ConnectedWasmSession),
}

pub struct WasmConnection {
    /// The session. May or may not contain a valid session depending on the state.
    session: WasmSession,
    /// A collection of events associated with this connection that can be read by the manager process.
    /// This is disconnected from the session state because stream events (such as disconnected) can still be read
    /// even if the connection is no longer valid.
    stream_events: VecDeque<StreamEvent<WasmConnectionId>>,
}

impl WasmConnection {
    pub(crate) fn new() -> Self {
        WasmConnection {
            session: WasmSession::Disconnected,
            stream_events: VecDeque::new(),
        }
    }

    async fn accept_uni(&mut self) {}

    async fn accept_bi(&mut self) -> Result<WebTransportBidirectionalStream, WebError> {
        if let WasmSession::Connected(session) = self.session {
            let transport = session.session;

            let mut reader = Reader::new(&transport.incoming_bidirectional_streams())?;
            let stream: WebTransportBidirectionalStream =
                reader.read().await?.expect("Closed without error");

            Ok(stream)
        }
    }
}

impl<'c> ConnectionMut<'c> for &'c mut WasmConnection {
    type NonMut<'b> = &'b WasmConnection where Self: 'b;
    type StreamType = WasmStreamId;

    fn as_ref<'b>(&'b self) -> Self::NonMut<'b> {
        self
    }

    fn disconnect(&mut self) {
        todo!();
    }
}

impl<'c> ConnectionRef<'c> for &'c WasmConnection {
    type ConnectionStats = ();

    fn get_stats(&self) -> Self::ConnectionStats {
        todo!()
    }
}
