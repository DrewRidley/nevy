use transport_interface::*;

mod stream_access;
mod stream_description;
mod stream_id;
pub use stream_access::*;
pub use stream_description::*;
pub use stream_id::*;

#[derive(Debug)]
pub struct MismatchedStreamType {
    pub expected: &'static str,
}

trait BevyConnectionInner<'c> {
    fn open_stream(
        &mut self,
        description: StreamDescription,
    ) -> Result<Option<BevyStreamId>, MismatchedStreamType>;

    fn send_stream(
        &mut self,
        stream_id: BevyStreamId,
    ) -> Result<Option<BevySendStream>, MismatchedStreamType>;

    fn recv_stream(
        &mut self,
        stream_id: BevyStreamId,
    ) -> Result<Option<BevyRecvStream>, MismatchedStreamType>;

    fn poll_stream_events(&mut self) -> Option<BevyStreamEvent>;
}

/// type erased mutable access to a connection
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

    fn send_stream(
        &mut self,
        stream_id: BevyStreamId,
    ) -> Result<Option<BevySendStream>, MismatchedStreamType> {
        let stream_id = stream_id.downcast()?;

        let Some(send_stream) = self.send_stream(stream_id) else {
            return Ok(None);
        };

        Ok(Some(BevySendStream::new(send_stream)))
    }

    fn recv_stream(
        &mut self,
        stream_id: BevyStreamId,
    ) -> Result<Option<BevyRecvStream>, MismatchedStreamType> {
        let stream_id = stream_id.downcast()?;

        let Some(recv_stream) = self.recv_stream(stream_id) else {
            return Ok(None);
        };

        Ok(Some(BevyRecvStream::new(recv_stream)))
    }

    fn poll_stream_events(&mut self) -> Option<BevyStreamEvent> {
        self.poll_stream_events().map(Into::into)
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
        self.inner.send_stream(stream_id)
    }

    pub fn recv_stream(
        &mut self,
        stream_id: BevyStreamId,
    ) -> Result<Option<BevyRecvStream>, MismatchedStreamType> {
        self.inner.recv_stream(stream_id)
    }

    pub fn poll_stream_events(&mut self) -> Option<BevyStreamEvent> {
        self.inner.poll_stream_events()
    }
}

/// type erased stream event
pub struct BevyStreamEvent {
    pub stream_id: BevyStreamId,
    pub peer_generated: bool,
    pub event_type: StreamEventType,
}

impl<S: BevyStreamIdInner> From<StreamEvent<S>> for BevyStreamEvent {
    fn from(value: StreamEvent<S>) -> Self {
        BevyStreamEvent {
            stream_id: BevyStreamId::new(value.stream_id),
            peer_generated: value.peer_generated,
            event_type: value.event_type,
        }
    }
}
