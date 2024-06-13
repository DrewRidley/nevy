use std::any::Any;

use transport_interface::*;

use super::MismatchedStreamType;

pub(crate) trait BevyStreamIdInner: Send + Sync + 'static {
    fn into_any(self: Box<Self>) -> Box<dyn Any>;

    fn clone_inner(&self) -> Box<dyn BevyStreamIdInner>;
}

pub struct BevyStreamId {
    inner: Box<dyn BevyStreamIdInner>,
}

impl<S: StreamId + Send + Sync> BevyStreamIdInner for S {
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
            inner: self.inner.clone_inner(),
        }
    }
}

impl BevyStreamId {
    pub(crate) fn new<T: BevyStreamIdInner>(inner: T) -> BevyStreamId {
        BevyStreamId {
            inner: Box::new(inner),
        }
    }

    pub(crate) fn downcast<T: 'static>(self) -> Result<T, MismatchedStreamType> {
        match self.inner.into_any().downcast() {
            Ok(downcasted) => Ok(*downcasted),
            Err(_) => Err(MismatchedStreamType {
                expected: std::any::type_name::<T>(),
            }),
        }
    }
}
