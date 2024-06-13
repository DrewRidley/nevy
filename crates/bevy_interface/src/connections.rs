use std::any::Any;

use transport_interface::*;

#[derive(Debug)]
pub struct MismatchedStreamType {
    pub expected: &'static str,
    pub actual: &'static str,
}

pub(crate) trait BevyConnectionInner<'c> {
    fn open_stream(
        &mut self,
        creator: StreamDescription,
    ) -> Result<Option<BevyStreamId>, MismatchedStreamType>;

    fn send_stream_mut(&mut self, id: BevyStreamId) -> Result<Option<BevySendStream>, MismatchedStreamType>
}

pub struct BevyConnectionMut<'c> {
    inner: Box<dyn BevyConnectionInner<'c> + 'c>,
}

impl<'c, C: ConnectionMut<'c>> BevyConnectionInner<'c> for C {
    fn open_stream(
        &mut self,
        creator: StreamDescription,
    ) -> Result<Option<BevyStreamId>, MismatchedStreamType> {
        let description = match creator.description.downcast() {
            Ok(description) => *description,
            Err(description) => {
                return Err(MismatchedStreamType {
                    expected: std::any::type_name::<<C::StreamType as StreamId>::OpenDescription>(),
                    actual: std::any::type_name_of_val(&*description),
                })
            }
        };

        let Some(stream_id) = self.open_stream(description) else {
            return Ok(None);
        };

        Ok(Some(BevyStreamId {
            inner: Box::new(stream_id),
        }))
    }

    fn send_stream_mut(&mut self, id: BevyStreamId) -> Result<Option<BevySendStream>, MismatchedStreamType> {
        todo!()
    }
}

impl<'c> BevyConnectionMut<'c> {
    pub(crate) fn new<C: ConnectionMut<'c> + 'c>(connection_mut: C) -> Self {
        BevyConnectionMut {
            inner: Box::new(connection_mut),
        }
    }

    pub fn open_stream(
        &mut self,
        creator: StreamDescription,
    ) -> Result<Option<BevyStreamId>, MismatchedStreamType> {
        self.inner.open_stream(creator)
    }
}

pub struct StreamDescription {
    description: Box<dyn Any>,
}

impl StreamDescription {
    pub fn new<S: StreamId>(description: S::OpenDescription) -> Self
    where
        S::OpenDescription: 'static,
    {
        StreamDescription {
            description: Box::new(description),
        }
    }
}

trait CloneableDescription {
    fn clone(&self) -> Box<dyn CloneableDescription>;

    fn into_any(self: Box<Self>) -> Box<dyn Any>;
}

pub struct CloneableStreamDescription {
    description: Box<dyn CloneableDescription>,
}

impl<T: Clone + 'static> CloneableDescription for T {
    fn clone(&self) -> Box<dyn CloneableDescription> {
        Box::new(self.clone())
    }

    fn into_any(self: Box<Self>) -> Box<dyn Any> {
        self
    }
}

impl Clone for CloneableStreamDescription {
    fn clone(&self) -> Self {
        CloneableStreamDescription {
            description: self.description.clone(),
        }
    }
}

impl From<CloneableStreamDescription> for StreamDescription {
    fn from(value: CloneableStreamDescription) -> Self {
        StreamDescription {
            description: value.description.into_any(),
        }
    }
}

impl CloneableStreamDescription {
    pub fn new<S: StreamId>(description: S::OpenDescription) -> Self
    where
        S::OpenDescription: Clone + 'static,
    {
        CloneableStreamDescription {
            description: Box::new(description),
        }
    }
}

trait BevyStreamIdInner {
    fn into_any(self: Box<Self>) -> Box<dyn Any>;

    fn clone_inner(&self) -> Box<dyn BevyStreamIdInner>;
}

pub struct BevyStreamId {
    inner: Box<dyn BevyStreamIdInner>,
}

impl<S: StreamId> BevyStreamIdInner for S {
    fn into_any(self: Box<Self>) -> Box<dyn Any> {
        self
    }

    fn clone_inner(&self) -> Box<dyn BevyStreamIdInner> {
        Box::new(self.clone())
    }
}

impl Clone for BevyStreamId {
    fn clone(&self) -> Self {
        BevyStreamId {
            inner: self.inner.clone_inner()
        }
    }
}

trait BevySendStreamInner<'s> {}

pub struct BevySendStream<'s> {
    inner: Box<dyn BevySendStreamInner<'s> + 's>,
}
