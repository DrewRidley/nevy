use std::collections::{HashSet, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;
use web_sys::WebTransport;

use transport_interface::*;

use crate::stream::WebTransportStreamId;

pub struct WebTransportConnectionId(u64);

impl WebTransportConnectionId {
    pub(crate) fn new() -> Self {
        static CONNECTION_ID_COUNTER: AtomicU64 = AtomicU64::new(1);
        WebTransportConnectionId(CONNECTION_ID_COUNTER.fetch_add(1, Ordering::SeqCst))
    }
}

pub struct WebTransportConnection {
    pub(crate) connection: WebTransport,
    pub(crate) connection_id: WebTransportConnectionId,
    pub(crate) stream_events: VecDeque<StreamEvent<WebTransportStreamId>>,
    pub(crate) open_send_streams: HashSet<WebTransportStreamId>,
    pub(crate) open_recv_streams: HashSet<WebTransportStreamId>,
}

impl WebTransportConnection {
    pub(crate) fn new(connection: WebTransport, connection_id: WebTransportConnectionId) -> Self {
        WebTransportConnection {
            connection,
            connection_id,
            stream_events: VecDeque::new(),
            open_send_streams: HashSet::new(),
            open_recv_streams: HashSet::new(),
        }
    }

    pub async fn poll_timeouts(&self) {
        // No explicit timeout handling required for WebTransport
    }

    pub async fn poll_events(&self, handler: &mut impl EndpointEventHandler<WebTransportEndpoint>) {
        // Handle WebTransport events like readiness, closures, etc.

        // Example for handling readiness:
        let ready_promise = self.connection.ready();
        if let Ok(_) = JsFuture::from(ready_promise).await {
            handler.connected(self.connection_id);
        }

        // Example for handling closed:
        let closed_promise = self.connection.closed();
        if let Ok(_) = JsFuture::from(closed_promise).await {
            handler.disconnected(self.connection_id);
        }
    }

    pub async fn accept_streams(&self) {
        //.incoming_bidirectional_streams returns a ReadableStream
        let incoming_bidi_streams = self.connection.incoming_bidirectional_streams();
        let incoming_uni_streams = self.connection.incoming_unidirectional_streams();

        let reader = incoming_bidi_streams
            .get_reader()
            .dyn_into::<web_sys::ReadableStreamDefaultReader>()
            .unwrap();
        while let Ok(stream) = JsFuture::from(reader.read()).await {
            let new_stream = stream.dyn_into::<web_sys::ReadableStream>().unwrap();
            let stream_id = WebTransportStreamId::new();
            self.open_recv_streams.insert(stream_id);
            self.stream_events.push_back(StreamEvent {
                stream_id,
                peer_generated: true,
                event_type: StreamEventType::NewRecvStream,
            });
        }

        let reader_uni = incoming_uni_streams
            .get_reader()
            .dyn_into::<web_sys::ReadableStreamDefaultReader>()
            .unwrap();
        while let Ok(stream) = JsFuture::from(reader_uni.read()).await {
            let new_stream = stream.dyn_into::<web_sys::ReadableStream>().unwrap();
            let stream_id = WebTransportStreamId::new();
            self.open_recv_streams.insert(stream_id);
            self.stream_events.push_back(StreamEvent {
                stream_id,
                peer_generated: true,
                event_type: StreamEventType::NewRecvStream,
            });
        }
    }

    pub fn side(&self) -> web_sys::WebTransport {
        self.connection.clone()
    }
}

impl<'c> ConnectionMut<'c> for &'c mut WebTransportConnection {
    type NonMut<'b> = &'b WebTransportConnection where Self: 'b;

    type StreamType = WebTransportStreamId;

    fn as_ref<'b>(&'b self) -> Self::NonMut<'b> {
        self
    }

    fn disconnect(&mut self) {
        self.connection.close();
    }
}

impl<'c> ConnectionRef<'c> for &'c WebTransportConnection {
    type ConnectionStats = WebTransportConnectionId;

    fn get_stats(&self) -> WebTransportConnectionId {
        self.connection_id
    }
}
