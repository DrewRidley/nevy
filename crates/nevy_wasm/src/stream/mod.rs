use crate::{connection::WasmConnection, error::WebError};
use transport_interface::{ConnectionMut, ErrorFatality, RecvStreamMut, SendStreamMut, StreamId};
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::{WebTransport, WebTransportBidirectionalStream, WebTransportSendStream};

pub(crate) mod reader;
pub(crate) mod recv;
pub(crate) mod send;
pub(crate) mod writer;

use {recv::RecvStream, send::SendStream};

#[derive(Clone, Copy, Eq, PartialEq, Hash)]
pub struct WasmStreamId(u64);

impl WasmStreamId {
    pub async fn open_bi(
        transport: &mut WebTransport,
    ) -> Result<(SendStream, Option<RecvStream>), WebError> {
        let stream: WebTransportBidirectionalStream =
            JsFuture::from(transport.create_bidirectional_stream())
                .await?
                .dyn_into()?;

        let send = SendStream::new(stream.writable())?;
        let recv = RecvStream::new(stream.readable())?;

        Ok((send, Some(recv)))
    }

    pub async fn open_uni(
        transport: &mut WebTransport,
    ) -> Result<(SendStream, Option<RecvStream>), WebError> {
        let stream: WebTransportSendStream =
            JsFuture::from(transport.create_unidirectional_stream())
                .await?
                .dyn_into()?;

        let send = SendStream::new(stream)?;
        Ok((send, None))
    }
}

impl StreamId for WasmStreamId {
    type Connection<'c> = &'c mut WasmConnection;
    type SendMut<'s> = &'s mut WasmSendStream;
    type RecvMut<'s> = &'s mut WasmRecvStream;

    type OpenDescription = ();
    fn open<'c>(
        connection: &mut Self::Connection<'c>,
        description: Self::OpenDescription,
    ) -> Option<Self>
    where
        Self: Sized,
    {
        //Here we allocate an ID and box a future with that ID.
        //We then return the same ID to the client for use.
    }

    fn get_send<'c, 's>(
        self,
        connection: &'s mut Self::Connection<'c>,
    ) -> Option<Self::SendMut<'s>> {
        connection.send_stream(self)
    }

    fn get_recv<'c, 's>(
        self,
        connection: &'s mut Self::Connection<'c>,
    ) -> Option<Self::RecvMut<'s>> {
        connection.recv_stream(self)
    }

    fn poll_events<'c>(
        connection: &mut Self::Connection<'c>,
    ) -> Option<transport_interface::StreamEvent<Self>>
    where
        Self: Sized,
    {
        None
    }
}

pub struct WasmSendStream {
    send: SendStream,
}

impl WasmSendStream {
    pub(crate) fn new(send: SendStream) -> Self {
        Self { send }
    }
}

#[derive(Debug)]
pub struct SendBufferFull;

/// A full send buffer is 'fatal' because it will not be spontaneously resolved if invoked again.
/// The only way to continue writing is to flush the buffer back to the socket again, which happens once per [Update].
impl ErrorFatality for SendBufferFull {
    fn is_fatal(&self) -> bool {
        true
    }
}

impl<'s> SendStreamMut<'s> for &'s mut WasmSendStream {
    type SendError = SendBufferFull;
    type CloseDescription = String;

    fn send(&mut self, data: &[u8]) -> Result<usize, Self::SendError> {
        todo!()
    }

    fn close(&mut self, description: Self::CloseDescription) -> Result<(), ()> {
        todo!()
    }

    fn is_open(&self) -> bool {
        todo!()
    }
}

pub struct WasmRecvStream {
    recv: RecvStream,
}

impl WasmRecvStream {
    pub(crate) fn new(recv: RecvStream) -> Self {
        Self { recv }
    }
}

impl<'s> RecvStreamMut<'s> for &'s mut WasmRecvStream {
    type ReadError = ();

    type CloseDescription = ();

    fn recv(&mut self, limit: usize) -> Result<Box<[u8]>, Self::ReadError> {
        todo!()
    }

    fn close(&mut self, description: Self::CloseDescription) -> Result<(), ()> {
        todo!()
    }

    fn is_open(&self) -> bool {
        todo!()
    }
}
