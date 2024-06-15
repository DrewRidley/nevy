use transport_interface::{ErrorFatality, RecvStreamMut, SendStreamMut, StreamId};

use crate::connection::WasmConnection;

/// stream id for a quinn stream
#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct WasmStreamId(pub(crate) u64);

impl StreamId for WasmStreamId {
    type Connection<'c> = &'c mut WasmConnection;
    type SendMut<'s> = WasmSendStreamMut;
    type RecvMut<'s> = WasmRecvStream;

    // Since WASM can only initiate, theres only a single direction here.
    type OpenDescription = ();

    fn open<'c>(
        connection: &mut Self::Connection<'c>,
        description: Self::OpenDescription,
    ) -> Option<Self>
    where
        Self: Sized,
    {
        todo!();
    }

    fn get_send<'c, 's>(
        self,
        connection: &'s mut Self::Connection<'c>,
    ) -> Option<Self::SendMut<'s>> {
        todo!()
    }

    fn get_recv<'c, 's>(
        self,
        connection: &'s mut Self::Connection<'c>,
    ) -> Option<Self::RecvMut<'s>> {
        todo!()
    }

    fn poll_events<'c>(
        connection: &mut Self::Connection<'c>,
    ) -> Option<transport_interface::StreamEvent<Self>>
    where
        Self: Sized,
    {
        todo!()
    }
}

pub struct WasmSendStreamMut(web_sys::WebTransportSendStream);

#[derive(PartialEq, Eq)]
pub enum WasmSendError {
    Blocked,
    NoStream,
}

impl ErrorFatality for WasmSendError {
    fn is_fatal(&self) -> bool {
        *self == WasmSendError::Blocked
    }
}

impl<'s> SendStreamMut<'s> for WasmSendStreamMut {
    type SendError = WasmSendError;
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

pub struct WasmRecvStream(web_sys::WebTransportReceiveStream);

#[derive(PartialEq, Eq)]
pub enum WasmReceiveError {
    Blocked,
    Finished,
    NoStream,
}

impl ErrorFatality for WasmReceiveError {
    fn is_fatal(&self) -> bool {
        *self == WasmReceiveError::NoStream
    }
}

impl<'s> RecvStreamMut<'s> for WasmRecvStream {
    type ReadError = WasmReceiveError;
    type CloseDescription = String;

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
