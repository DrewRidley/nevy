use bytes::{Buf, Bytes};
use js_sys::{Reflect, Uint8Array};
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::{WebTransportSendStream, WritableStream, WritableStreamDefaultWriter};

use crate::error::WebError;

pub struct SendStream {
    stream: WebTransportSendStream,
    writer: Writer,
}

struct Writer {
    inner: WritableStreamDefaultWriter,
}

impl SendStream {
    pub fn new(stream: WebTransportSendStream) -> Result<Self, WebError> {
        let writer = Writer::new(&stream)?;
        Ok(Self { stream, writer })
    }

    pub async fn write(&mut self, buf: &[u8]) -> Result<usize, WebError> {
        self.writer.write(&Uint8Array::from(buf)).await?;
        Ok(buf.len())
    }

    pub async fn write_buf<B: Buf>(&mut self, buf: &mut B) -> Result<usize, WebError> {
        let chunk = buf.chunk();
        self.writer.write(&Uint8Array::from(chunk)).await?;
        Ok(chunk.len())
    }

    pub async fn write_chunk(&mut self, buf: Bytes) -> Result<(), WebError> {
        self.write(&buf).await.map(|_| ())
    }

    pub fn reset(self, reason: &str) {
        self.writer.close(reason);
    }

    pub fn set_priority(&mut self, order: i32) {
        Reflect::set(&self.stream, &"sendOrder".into(), &order.into())
            .expect("failed to set priority");
    }
}

impl Writer {
    fn new(stream: &WritableStream) -> Result<Self, WebError> {
        let inner = stream.get_writer()?.unchecked_into();
        Ok(Self { inner })
    }

    async fn write(&mut self, v: &JsValue) -> Result<(), WebError> {
        JsFuture::from(self.inner.write_with_chunk(v)).await?;
        Ok(())
    }

    fn close(self, reason: &str) {
        let str = JsValue::from_str(reason);
        let _ = self.inner.abort_with_reason(&str);
    }
}

impl Drop for Writer {
    fn drop(&mut self) {
        let _ = self.inner.close();
        self.inner.release_lock();
    }
}
