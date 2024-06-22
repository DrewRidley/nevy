use std::collections::{HashSet, VecDeque};
use transport_interface::*;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;
use web_sys::{
    ReadableStream, ReadableStreamDefaultReader, WebTransportSendStream,
    WritableStreamDefaultWriter,
};

use crate::connection::WebTransportConnection;

#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct WebTransportStreamId(u64);

impl WebTransportStreamId {
    pub(crate) fn new() -> Self {
        static STREAM_ID_COUNTER: std::sync::atomic::AtomicU64 =
            std::sync::atomic::AtomicU64::new(1);
        WebTransportStreamId(STREAM_ID_COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst))
    }
}

pub struct WebTransportSendStreamMut<'s> {
    stream: WebTransportSendStream,
    stream_id: WebTransportStreamId,
    open_streams: &'s mut HashSet<WebTransportStreamId>,
}

pub struct WebTransportRecvStreamMut<'s> {
    events: &'s mut VecDeque<StreamEvent<WebTransportStreamId>>,
    stream_id: WebTransportStreamId,
    stream: ReadableStream,
    open_streams: &'s mut HashSet<WebTransportStreamId>,
}

#[derive(Debug)]
pub enum WebTransportSendError {
    Blocked,
    NoStream,
}

#[derive(Debug)]
pub enum WebTransportReadError {
    Blocked,
    Finished,
    NoStream,
}

impl ErrorFatality for WebTransportSendError {
    fn is_fatal(&self) -> bool {
        match self {
            WebTransportSendError::Blocked => false,
            WebTransportSendError::NoStream => true,
        }
    }
}

impl ErrorFatality for WebTransportReadError {
    fn is_fatal(&self) -> bool {
        match self {
            WebTransportReadError::Blocked => false,
            WebTransportReadError::Finished => false,
            WebTransportReadError::NoStream => true,
        }
    }
}

impl<'s> SendStreamMut<'s> for WebTransportSendStreamMut<'s> {
    type SendError = WebTransportSendError;

    type CloseDescription = JsValue;

    fn send(&mut self, data: &[u8]) -> Result<usize, Self::SendError> {
        let writer = WritableStreamDefaultWriter::new(&self.stream).unwrap();
        let chunk = JsValue::from_serde(data).unwrap();
        wasm_bindgen_futures::spawn_local(async move {
            JsFuture::from(writer.write_with_chunk(&chunk))
                .await
                .expect("Error writing to stream");
        });

        Ok(data.len())
    }

    fn close(&mut self, description: Self::CloseDescription) -> Result<(), ()> {
        wasm_bindgen_futures::spawn_local(async move {
            JsFuture::from(self.stream.close_with_reason(description))
                .await
                .expect("Error closing stream");
        });

        self.open_streams.remove(&self.stream_id);
        Ok(())
    }

    fn is_open(&self) -> bool {
        self.open_streams.contains(&self.stream_id)
    }
}

impl<'s> RecvStreamMut<'s> for WebTransportRecvStreamMut<'s> {
    type ReadError = WebTransportReadError;

    type CloseDescription = JsValue;

    fn recv(&mut self, limit: usize) -> Result<Box<[u8]>, Self::ReadError> {
        let reader = ReadableStreamDefaultReader::new(&self.stream).unwrap();
        wasm_bindgen_futures::spawn_local(async move {
            match JsFuture::from(reader.read()).await {
                Ok(chunk) => {
                    let array = js_sys::Uint8Array::new(&chunk);
                    if array.length() == 0 {
                        self.open_streams.remove(&self.stream_id);
                        self.events.push_back(StreamEvent {
                            stream_id: self.stream_id,
                            peer_generated: true,
                            event_type: StreamEventType::ClosedRecvStream,
                        });
                        Err(WebTransportReadError::Finished)
                    } else {
                        Ok(array.to_vec().into_boxed_slice())
                    }
                }
                Err(_) => Err(WebTransportReadError::Blocked),
            }
        });

        Err(WebTransportReadError::Blocked)
    }

    fn close(&mut self, description: Self::CloseDescription) -> Result<(), ()> {
        wasm_bindgen_futures::spawn_local(async move {
            JsFuture::from(self.stream.close_with_reason(description))
                .await
                .expect("Error closing stream");
        });

        self.open_streams.remove(&self.stream_id);
        Ok(())
    }

    fn is_open(&self) -> bool {
        self.open_streams.contains(&self.stream_id)
    }
}

impl StreamId for WebTransportStreamId {
    type Connection<'c> = &'c mut WebTransportConnection;

    type SendMut<'s> = WebTransportSendStreamMut<'s>;

    type RecvMut<'s> = WebTransportRecvStreamMut<'s>;

    type OpenDescription = ();

    fn open<'c>(connection: &mut Self::Connection<'c>, _: Self::OpenDescription) -> Option<Self> {
        let send_stream = connection.connection.create_bidirectional_stream().unwrap();
        let stream_id = WebTransportStreamId::new();
        connection.open_send_streams.insert(stream_id);
        connection.open_recv_streams.insert(stream_id);

        Some(stream_id)
    }

    fn get_send<'c, 's>(
        self,
        connection: &'s mut Self::Connection<'c>,
    ) -> Option<Self::SendMut<'s>> {
        if !connection.open_send_streams.contains(&self) {
            return None;
        }
        let send_stream = connection.connection.create_bidirectional_stream().unwrap();
        Some(WebTransportSendStreamMut {
            stream: send_stream,
            stream_id: self,
            open_streams: &mut connection.open_send_streams,
        })
    }

    fn get_recv<'c, 's>(
        self,
        connection: &'s mut Self::Connection<'c>,
    ) -> Option<Self::RecvMut<'s>> {
        if !connection.open_recv_streams.contains(&self) {
            return None;
        }
        let recv_stream = connection.connection.create_bidirectional_stream().unwrap();
        Some(WebTransportRecvStreamMut {
            events: &mut connection.stream_events,
            stream_id: self,
            stream: recv_stream,
            open_streams: &mut connection.open_recv_streams,
        })
    }

    fn poll_events<'c>(connection: &mut Self::Connection<'c>) -> Option<StreamEvent<Self>> {
        connection.stream_events.pop_front()
    }
}
