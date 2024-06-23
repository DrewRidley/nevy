use std::collections::HashMap;

use bytes::Bytes;
use futures_lite::{
    future::{block_on, poll_once, race, BoxedLocal},
    FutureExt,
};
use js_sys::{Reflect, Uint8Array};
use transport_interface::{ConnectionMut, ConnectionRef};
use wasm_bindgen::JsCast;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;
use web_sys::{
    ReadableStream, ReadableStreamDefaultReader, WebTransport, WebTransportBidirectionalStream,
    WebTransportCloseInfo, WebTransportReceiveStream, WebTransportSendStream,
};

use crate::{
    error::WebError,
    stream::reader::Reader,
    stream::writer::Writer,
    stream::{recv::RecvStream, send::SendStream},
    stream::{WasmRecvStream, WasmSendStream, WasmStreamId},
};

pub struct WasmConnectionId(u64);

pub struct WasmConnection {
    /// If exists, we are connecting.
    inner: web_sys::WebTransport,
    ready_future: Option<BoxedLocal<Result<JsValue, JsValue>>>,
    accept_stream_future: Option<BoxedLocal<Result<JsValue, JsValue>>>,
    /// contains futures for streams that haven't been opened yet
    pub(crate) unopened_streams: Vec<(
        WasmStreamId,
        BoxedLocal<Result<(SendStream, Option<RecvStream>), WebError>>,
    )>,

    /// Contains all of the completed streams ready to receive data on.
    recv_streams: HashMap<WasmStreamId, WasmRecvStream>,
    /// Contaisn all of the completed streams ready to write to.
    send_streams: HashMap<WasmStreamId, WasmSendStream>,
}

impl WasmConnection {
    pub(crate) fn new(url: &str) -> Self {
        let transport = web_sys::WebTransport::new(url).unwrap();
        let ready_future = Some(JsFuture::from(transport.ready()).boxed_local());

        let incoming_bidir = transport.incoming_bidirectional_streams();
        let incoming_unidir = transport.incoming_unidirectional_streams();

        let accept_stream_future = async move {
            let bidir: ReadableStreamDefaultReader = incoming_bidir.get_reader().unchecked_into();
            let unidir: ReadableStreamDefaultReader = incoming_unidir.get_reader().unchecked_into();
            race(JsFuture::from(bidir.read()), JsFuture::from(unidir.read())).await
        }
        .boxed_local();

        Self {
            inner: transport,
            ready_future,
            accept_stream_future: Some(accept_stream_future),
            unopened_streams: vec![],
            recv_streams: HashMap::new(),
            send_streams: HashMap::new(),
        }
    }

    async fn accept_uni(transport: &WebTransport) -> Result<RecvStream, WebError> {
        let mut reader = Reader::new(&transport.incoming_unidirectional_streams())?;
        let stream: WebTransportReceiveStream = reader.read().await?.expect("closed without error");
        let recv = RecvStream::new(stream)?;
        Ok(recv)
    }

    async fn accept_bi(transport: &WebTransport) -> Result<(SendStream, RecvStream), WebError> {
        let mut reader = Reader::new(&transport.incoming_bidirectional_streams())?;
        let stream: WebTransportBidirectionalStream =
            reader.read().await?.expect("closed without error");

        let send = SendStream::new(stream.writable())?;
        let recv = RecvStream::new(stream.readable())?;

        Ok((send, recv))
    }

    pub async fn send_datagram(
        transport: &mut WebTransport,
        payload: Bytes,
    ) -> Result<(), WebError> {
        let mut writer = Writer::new(&transport.datagrams().writable())?;
        writer.write(&Uint8Array::from(payload.as_ref())).await?;
        Ok(())
    }

    pub async fn recv_datagram(transport: &mut WebTransport) -> Result<Bytes, WebError> {
        let mut reader = Reader::new(&transport.datagrams().readable())?;
        let data: Uint8Array = reader.read().await?.unwrap_or_default();
        Ok(data.to_vec().into())
    }

    pub fn close(self, code: u32, reason: &str) {
        let mut info = WebTransportCloseInfo::new();
        info.close_code(code);
        info.reason(reason);
        self.inner.close_with_close_info(&info);
    }

    fn update(&mut self) {
        if let Some(r_future) = self.ready_future.as_mut() {
            match block_on(poll_once(r_future)) {
                Some(Ok(_)) => {
                    self.ready_future = None;
                }
                Some(Err(_)) => {
                    self.ready_future = None;
                    // Handle error if needed
                }
                None => return,
            }
        }

        if let Some(a_future) = self.accept_stream_future.as_mut() {
            match block_on(poll_once(a_future)) {
                Some(Ok(_)) => {
                    let incoming_bidir = self.inner.incoming_bidirectional_streams();
                    let incoming_unidir = self.inner.incoming_unidirectional_streams();

                    let new_accept_stream_future = async move {
                        let bidir: ReadableStreamDefaultReader =
                            incoming_bidir.get_reader().unchecked_into();
                        let unidir: ReadableStreamDefaultReader =
                            incoming_unidir.get_reader().unchecked_into();
                        race(JsFuture::from(bidir.read()), JsFuture::from(unidir.read())).await
                    }
                    .boxed_local();

                    self.accept_stream_future = Some(new_accept_stream_future);
                }
                Some(Err(_)) => {
                    self.accept_stream_future = None;
                    // Handle error if needed
                }
                None => {}
            }
        }

        self.unopened_streams.retain_mut(|(stream_id, future)| {
            match block_on(poll_once(future)) {
                Some(Ok((send, recv))) => {
                    self.send_streams
                        .insert(*stream_id, WasmSendStream::new(send));
                    if let Some(recv) = recv {
                        self.recv_streams
                            .insert(*stream_id, WasmRecvStream::new(recv));
                    }
                    false
                }
                Some(Err(_)) => {
                    // Handle error if needed
                    false
                }
                None => true,
            }
        });
    }

    fn get_stats(&self) -> () {
        ()
    }

    fn disconnect(&mut self) {
        self.inner.close();
        // Clean up resources
        self.ready_future = None;
        self.accept_stream_future = None;
        self.unopened_streams.clear();
        self.recv_streams.clear();
        self.send_streams.clear();
    }
}

impl<'c> ConnectionRef<'c> for &'c WasmConnection {
    type ConnectionStats = ();

    fn get_stats(&self) -> Self::ConnectionStats {
        // Implement this method to return connection statistics
        ()
    }
}

impl<'c> ConnectionMut<'c> for &'c mut WasmConnection {
    type NonMut<'b> = &'b WasmConnection where 'c: 'b;
    type StreamType = WasmStreamId;

    fn as_ref<'b>(&'b self) -> Self::NonMut<'b> {
        self
    }

    fn disconnect(&mut self) {
        self.inner.close();
        self.ready_future = None;
        self.accept_stream_future = None;
        self.unopened_streams.clear();
        self.recv_streams.clear();
        self.send_streams.clear();
    }
}
