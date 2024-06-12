use std::any::Any;

use transport_interface::*;

#[derive(Debug)]
pub struct MismatchedType {
    pub expected: &'static str,
    pub actual: &'static str,
}

pub(crate) trait BevyConnectionInner<'c> {
    fn open_stream(
        &mut self,
        creator: StreamCreator,
    ) -> Result<Option<BevyStreamId>, MismatchedType>;
}

pub struct BevyConnectionMut<'c> {
    inner: Box<dyn BevyConnectionInner<'c> + 'c>,
}

impl<'c, C: ConnectionMut<'c>> BevyConnectionInner<'c> for C {
    fn open_stream(
        &mut self,
        creator: StreamCreator,
    ) -> Result<Option<BevyStreamId>, MismatchedType> {
        let description = match creator.description.downcast() {
            Ok(description) => *description,
            Err(description) => {
                return Err(MismatchedType {
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
}

impl<'c> BevyConnectionMut<'c> {
    pub(crate) fn new<C: ConnectionMut<'c> + 'c>(connection_mut: C) -> Self {
        BevyConnectionMut {
            inner: Box::new(connection_mut),
        }
    }

    pub fn open_stream(
        &mut self,
        creator: StreamCreator,
    ) -> Result<Option<BevyStreamId>, MismatchedType> {
        self.inner.open_stream(creator)
    }
}

pub struct StreamCreator {
    description: Box<dyn Any>,
}

impl StreamCreator {
    pub fn new<S: StreamId>(description: S::OpenDescription) -> Self
    where
        S::OpenDescription: 'static,
    {
        StreamCreator {
            description: Box::new(description),
        }
    }
}

trait CloneableDescription {
    fn clone(&self) -> Box<dyn CloneableDescription>;

    fn into_any(self: Box<Self>) -> Box<dyn Any>;
}

pub struct CloneableStreamCreator {
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

impl Clone for CloneableStreamCreator {
    fn clone(&self) -> Self {
        CloneableStreamCreator {
            description: self.description.clone(),
        }
    }
}

impl From<CloneableStreamCreator> for StreamCreator {
    fn from(value: CloneableStreamCreator) -> Self {
        StreamCreator {
            description: value.description.into_any(),
        }
    }
}

impl CloneableStreamCreator {
    pub fn new<S: StreamId>(description: S::OpenDescription) -> Self
    where
        S::OpenDescription: Clone + 'static,
    {
        CloneableStreamCreator {
            description: Box::new(description),
        }
    }
}

pub struct BevyStreamId {
    inner: Box<dyn Any>,
}
