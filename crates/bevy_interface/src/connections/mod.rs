use std::any::Any;

use transport_interface::*;

mod stream_description;
mod stream_id;
mod stream_ref;
pub use stream_description::*;
pub use stream_id::*;
pub use stream_ref::*;

#[derive(Debug)]
pub struct MismatchedStreamType {
    pub expected: &'static str,
}

pub(crate) trait BevyConnectionInner<'c> {
    fn open_stream(
        &mut self,
        description: StreamDescription,
    ) -> Result<Option<BevyStreamId>, MismatchedStreamType>;

    fn send_stream_mut(
        &mut self,
        stream_id: BevyStreamId,
    ) -> Result<Option<BevySendStream>, MismatchedStreamType>;
}

pub struct BevyConnectionMut<'c> {
    inner: Box<dyn BevyConnectionInner<'c> + 'c>,
}

impl<'c, C: ConnectionMut<'c>> BevyConnectionInner<'c> for C
where
    C::StreamType: Send + Sync,
{
    fn open_stream(
        &mut self,
        description: StreamDescription,
    ) -> Result<Option<BevyStreamId>, MismatchedStreamType> {
        let description = description.downcast()?;

        let Some(stream_id) = self.open_stream(description) else {
            return Ok(None);
        };

        Ok(Some(BevyStreamId::new(stream_id)))
    }

    fn send_stream_mut(
        &mut self,
        stream_id: BevyStreamId,
    ) -> Result<Option<BevySendStream>, MismatchedStreamType> {
        let stream_id = stream_id.downcast()?;

        let Some(send_stream) = self.send_stream(stream_id) else {
            return Ok(None);
        };

        // Ok(Some(BevySendStream {
        //     inner: Box::new(send_stream),
        // }))
        todo!();
    }
}

impl<'c> BevyConnectionMut<'c> {
    pub(crate) fn new<C: ConnectionMut<'c> + 'c>(connection_mut: C) -> Self
    where
        C::StreamType: Send + Sync,
    {
        BevyConnectionMut {
            inner: Box::new(connection_mut),
        }
    }

    pub fn open_stream(
        &mut self,
        description: StreamDescription,
    ) -> Result<Option<BevyStreamId>, MismatchedStreamType> {
        self.inner.open_stream(description)
    }

    pub fn send_stream(
        &mut self,
        stream_id: BevyStreamId,
    ) -> Result<Option<BevySendStream>, MismatchedStreamType> {
        self.inner.send_stream_mut(stream_id)
    }
}
