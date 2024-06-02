use std::{
    collections::VecDeque,
    future::{Future, IntoFuture},
    task::{Context, Poll, RawWaker, RawWakerVTable, Waker},
};

use transport_interface::*;
use wasm_bindgen::JsValue;
use web_sys::{WebTransportBidirectionalStream, WebTransportReceiveStream};

use crate::{
    endpoint::{WasmConnectionId, WasmEndpoint},
    error::WebError,
    reader::Reader,
    recv::RecvStream,
    send::SendStream,
};

pub struct WasmConnection {
    pub(crate) inner: web_sys::WebTransport,
    pub(crate) connect_future:
        Option<std::pin::Pin<Box<dyn Future<Output = Result<JsValue, JsValue>>>>>,

    accept_uni_future: std::pin::Pin<Box<dyn Future<Output = AcceptUniResult>>>,
    accept_bi_future: std::pin::Pin<Box<dyn Future<Output = AcceptBiResult>>>,
}

type AcceptUniResult = Result<RecvStream, WebError>;
type AcceptBiResult = Result<(SendStream, RecvStream), WebError>;

impl WasmConnection {
    pub(crate) fn update(
        &mut self,
        connection_id: WasmConnectionId,
        events: &mut VecDeque<EndpointEvent<WasmEndpoint>>,
    ) -> bool {
        //Connect us.
        if let Some(mut future) = self.connect_future.take() {
            let mut context = Context::from_waker(Waker::noop());
            let connect_result = match future.as_mut().poll(&mut context) {
                Poll::Ready(connect_result) => connect_result,
                Poll::Pending => return true,
            };

            match connect_result {
                Ok(_) => (),
                Err(_) => {
                    events.push_back(EndpointEvent {
                        connection_id,
                        event: ConnectionEvent::Disconnected,
                    });
                    return false;
                }
            }
        }

        let mut ctx = Context::from_waker(Waker::noop());
        match self.accept_bi_future.as_mut().poll(&mut ctx) {
            Poll::Pending => (),
            Poll::Ready(res) => {
                if let Ok((send, recv)) = res {

                }
            }
        }

        true
    }

    pub async fn accept_uni(&mut self) -> Result<RecvStream, WebError> {
        let mut reader = Reader::new(&self.inner.incoming_unidirectional_streams())?;
        let stream: WebTransportReceiveStream = reader.read().await?.expect("closed without error");
        let recv = RecvStream::new(stream)?;
        Ok(recv)
    }

    pub async fn accept_bi(&mut self) -> Result<(SendStream, RecvStream), WebError> {
        let mut reader = Reader::new(&self.inner.incoming_bidirectional_streams())?;
        let stream: WebTransportBidirectionalStream =
            reader.read().await?.expect("closed without error");

        let send = SendStream::new(stream.writable())?;
        let recv = RecvStream::new(stream.readable())?;

        Ok((send, recv))
    }
}

impl<'c> ConnectionMut<'c> for &'c mut WasmConnection {
    type NonMut<'b> = &'b WasmConnection
    where
        Self: 'b;

    fn as_ref<'b>(&'b self) -> Self::NonMut<'b> {
        self
    }

    fn disconnect(&mut self) {
        self.inner.close();
    }
}

impl<'c> ConnectionRef<'c> for &'c WasmConnection {
    type ConnectionStats = ();

    fn get_stats(&self) -> Self::ConnectionStats {
        ()
    }
}
