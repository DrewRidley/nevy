use futures_lite::future::{block_on, poll_once, Boxed, BoxedLocal};
use futures_lite::FutureExt;
use log::{debug, info, trace, warn};
use slotmap::{new_key_type, SlotMap};
use std::collections::VecDeque;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;
use web_sys::WebTransport;

use crate::error::WebError;
use crate::stream::{RecvStream, SendStream};

new_key_type! {
    /// A ephemeral identifier used to track a particular local stream.
    ///
    /// A single [StreamKey] may be associated with both a [SendStream] and [RecvStream]
    /// if the stream is of the bidirectional type.
    pub struct StreamKey;
}

/// The type of stream.
///
/// Methods to get a send or recv stream may return [None] if a stream is not of the requested direction.
pub(crate) enum StreamType {
    Send(SendStream),
    Recv(RecvStream),
    Bidirectional(SendStream, RecvStream),
}

/// The current state of the stream.
///
/// A stream may be currently Opening, in which case the runtime must poll the future.
/// Otherwise, the stream is ready and it's writer/reader can be used.
pub(crate) enum StreamState {
    Opening(Boxed<Result<StreamType, WebError>>),
    Ready(StreamType),
    Closed,
}

/// A wrapper containing the state of the stream.
pub struct Stream {
    state: StreamState,
}

pub struct WasmConnection {
    transport: WebTransport,
    ready_future: Option<BoxedLocal<Result<JsValue, JsValue>>>,

    streams: SlotMap<StreamKey, Stream>,
    pending_streams: Vec<StreamKey>,
    pending_events: VecDeque<StreamEvent>,
}

/// A event associated with a particular [StreamKey].
///
pub enum StreamEvent {
    Opened(StreamKey),
    Closed(StreamKey),
    Error(StreamKey, WebError),
}

impl WasmConnection {
    pub fn new(transport: WebTransport) -> Self {
        let promise = JsFuture::from(transport.ready());
        let future = Some(async move { promise.await }.boxed_local());

        Self {
            transport,
            ready_future: future,
            streams: SlotMap::with_key(),
            pending_events: VecDeque::new(),
            pending_streams: vec![],
        }
    }

    /// Opens a stream.
    ///
    /// The opened stream is facilitated through manual polling of the inserted future.
    /// The returned [StreamKey] may be unavailable for reading/writing for a few ticks.
    /// User applications should not expect immediate usage of the stream.
    pub fn open_stream(&mut self, is_bidirectional: bool) -> StreamKey {
        let future = self.create_stream_future(is_bidirectional);
        let stream = Stream {
            state: StreamState::Opening(future),
        };
        self.streams.insert(stream)
    }

    fn create_stream_future(&self, is_bidirectional: bool) -> Boxed<Result<StreamType, WebError>> {
        async move { Err(WebError::from(JsValue::null())) }.boxed()
    }

    /// Internally polls for the latest stream events.
    pub(crate) fn poll_events(&mut self) -> Option<StreamEvent> {
        if let Some(event) = self.pending_events.pop_front() {
            return Some(event);
        }
        None
    }

    /// Tries to progress the internal futures used to open or accept streams.
    ///
    /// Uses the internal futures directly to accomplish this.
    pub(crate) fn update(&mut self) {
        // If we are not ready, let's try to progress the ready state and then return.
        if let Some(not_ready) = &mut self.ready_future {
            trace!("Progressing the 'ready' promise.");
            if let Some(result) = block_on(poll_once(not_ready)) {
                debug!("'ready()' promise was resolved to: {:?}", result);
                self.ready_future = None;
            };
            return;
        }

        //Progress stream opening.
        self.pending_streams.retain(|key| {
            if let Some(stream) = self.streams.get_mut(*key) {

                //If we are currently opening, lets progress the future.
                //Otherwise, we can just remove the entry from the list.
                let StreamState::Opening(open) = &mut stream.state else {
                    trace!("A pending stream was already open! Removing it from 'pending_streams'.");
                    return false;
                };

                let Some(poll_result) = block_on(poll_once(open)) else {
                    //We need to retain the future since its not finished progressing.
                    trace!("Stream future is not resolved. Continuing.");
                    return true;
                };

                debug!("Stream was ready. Progressing state enum and removing entry.");
                stream.state = StreamState::Ready(poll_result.unwrap());
                false
            } else {
                // Stream doesn't exist, remove it from pending_streams
                debug!("A stream was marked as pending but did not exist in the streams map. Removing from pending streams.");
                false
            }
        });
    }

    /// Fetches a mutable [SendStream] identified by the specified [StreamKey].
    ///
    /// Will return [None] if the stream does not exist, or is a unidirectional receive stream.
    pub fn send_stream(&mut self, key: StreamKey) -> Option<&mut SendStream> {
        if let Some(stream) = self.streams.get_mut(key) {
            match &mut stream.state {
                StreamState::Ready(StreamType::Send(send)) => Some(send),
                StreamState::Ready(StreamType::Bidirectional(send, _)) => Some(send),
                _ => None,
            }
        } else {
            None
        }
    }

    /// Fetches a mutable [RecvStream] identified by the specified [StreamKey].
    ///
    /// Will return [None] if the stream does not exist, or is a unidirectional send stream.
    pub fn recv_stream(&mut self, key: StreamKey) -> Option<&mut RecvStream> {
        if let Some(stream) = self.streams.get_mut(key) {
            match &mut stream.state {
                StreamState::Ready(StreamType::Recv(recv)) => Some(recv),
                StreamState::Ready(StreamType::Bidirectional(_, recv)) => Some(recv),
                _ => None,
            }
        } else {
            None
        }
    }

    /// Close the stream associated with the specified key.
    pub fn close_stream(&mut self, key: StreamKey) {
        if let Some(stream) = self.streams.get_mut(key) {
            stream.state = StreamState::Closed;
            self.pending_events.push_back(StreamEvent::Closed(key));
        }
    }
}
