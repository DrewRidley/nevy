use crate::error::WebError;
use bytes::{BufMut, Bytes, BytesMut};
use js_sys::{Reflect, Uint8Array};
use std::cmp;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::{
    ReadableStream, ReadableStreamDefaultReader, ReadableStreamReadResult,
    WebTransportReceiveStream,
};

pub struct RecvStream {
    reader: Reader,
    buffer: BytesMut,
}

struct Reader {
    inner: ReadableStreamDefaultReader,
}

impl RecvStream {
    pub fn new(stream: WebTransportReceiveStream) -> Result<Self, WebError> {
        if stream.locked() {
            return Err("locked".into());
        }

        let reader = Reader::new(&stream)?;

        Ok(Self {
            reader,
            buffer: BytesMut::new(),
        })
    }

    pub async fn read(&mut self, buf: &mut [u8]) -> Result<Option<usize>, WebError> {
        Ok(self.read_chunk(buf.len()).await?.map(|chunk| {
            let size = chunk.len();
            buf[..size].copy_from_slice(&chunk);
            size
        }))
    }

    pub async fn read_buf<B: BufMut>(&mut self, buf: &mut B) -> Result<bool, WebError> {
        Ok(match self.read_chunk(buf.remaining_mut()).await? {
            Some(chunk) => {
                buf.put(chunk);
                true
            }
            None => false,
        })
    }

    pub async fn read_chunk(&mut self, max: usize) -> Result<Option<Bytes>, WebError> {
        if !self.buffer.is_empty() {
            let size = cmp::min(max, self.buffer.len());
            let data = self.buffer.split_to(size).freeze();
            return Ok(Some(data));
        }

        let mut data: Bytes = match self.reader.read::<Uint8Array>().await? {
            Some(data) => data.to_vec().into(),
            None => return Ok(None),
        };

        if data.len() > max {
            self.buffer.extend_from_slice(&data.split_off(max));
        }

        Ok(Some(data))
    }

    pub fn stop(self, reason: &str) {
        self.reader.close(reason);
    }
}

impl Reader {
    fn new(stream: &ReadableStream) -> Result<Self, WebError> {
        let inner = stream.get_reader().unchecked_into();
        Ok(Self { inner })
    }

    async fn read<T: JsCast>(&mut self) -> Result<Option<T>, WebError> {
        let result: ReadableStreamReadResult = JsFuture::from(self.inner.read()).await?.into();

        if Reflect::get(&result, &"done".into())?.is_truthy() {
            return Ok(None);
        }

        let res = Reflect::get(&result, &"value".into())?.dyn_into()?;
        Ok(Some(res))
    }

    fn close(self, reason: &str) {
        let str = JsValue::from_str(reason);
        let _ = self.inner.cancel_with_reason(&str);
    }
}

impl Drop for Reader {
    fn drop(&mut self) {
        let _ = self.inner.cancel();
        self.inner.release_lock();
    }
}
