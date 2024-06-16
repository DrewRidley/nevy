use std::any::Any;

use transport_interface::*;

use crate::{description::Description, MismatchedType};

pub trait StreamError {
    fn is_fatal(&self) -> bool;

    fn into_any(self: Box<Self>) -> Box<dyn Any>;
}

impl<E: ErrorFatality + 'static> StreamError for E {
    fn is_fatal(&self) -> bool {
        self.is_fatal()
    }

    fn into_any(self: Box<Self>) -> Box<dyn Any> {
        self
    }
}

pub(crate) trait BevySendStreamInner<'s> {
    fn send(&mut self, data: &[u8]) -> Result<usize, Box<dyn StreamError>>;

    fn close(&mut self, description: Description) -> Result<Result<(), ()>, MismatchedType>;

    fn is_open(&self) -> bool;
}

/// type erased mutable access to a send stream
pub struct BevySendStream<'s> {
    inner: Box<dyn BevySendStreamInner<'s> + 's>,
}

impl<'s, S: SendStreamMut<'s>> BevySendStreamInner<'s> for S {
    fn send(&mut self, data: &[u8]) -> Result<usize, Box<dyn StreamError>> {
        self.send(data)
            .map_err(|err| -> Box<dyn StreamError> { Box::new(err) })
    }

    fn close(&mut self, description: Description) -> Result<Result<(), ()>, MismatchedType> {
        let description = description.downcast()?;

        Ok(self.close(description))
    }

    fn is_open(&self) -> bool {
        self.is_open()
    }
}

impl<'s> BevySendStream<'s> {
    pub(crate) fn new<S: BevySendStreamInner<'s> + 's>(inner: S) -> Self {
        BevySendStream {
            inner: Box::new(inner),
        }
    }

    pub fn send(&mut self, data: &[u8]) -> Result<usize, Box<dyn StreamError>> {
        self.inner.send(data)
    }

    pub fn close(&mut self, description: Description) -> Result<Result<(), ()>, MismatchedType> {
        self.inner.close(description)
    }

    pub fn is_open(&self) -> bool {
        self.inner.is_open()
    }
}

pub(crate) trait BevyRecvStreamInner<'s> {
    fn recv(&mut self, limit: usize) -> Result<Box<[u8]>, Box<dyn StreamError>>;

    fn close(&mut self, description: Description) -> Result<Result<(), ()>, MismatchedType>;

    fn is_open(&self) -> bool;
}

/// type erased mutable access to a receive stream
pub struct BevyRecvStream<'s> {
    inner: Box<dyn BevyRecvStreamInner<'s> + 's>,
}

impl<'s, S: RecvStreamMut<'s>> BevyRecvStreamInner<'s> for S {
    fn recv(&mut self, limit: usize) -> Result<Box<[u8]>, Box<dyn StreamError>> {
        self.recv(limit)
            .map_err(|err| -> Box<dyn StreamError> { Box::new(err) })
    }

    fn close(&mut self, description: Description) -> Result<Result<(), ()>, MismatchedType> {
        let description = description.downcast()?;

        Ok(self.close(description))
    }

    fn is_open(&self) -> bool {
        self.is_open()
    }
}

impl<'s> BevyRecvStream<'s> {
    pub(crate) fn new<S: BevyRecvStreamInner<'s> + 's>(inner: S) -> Self {
        BevyRecvStream {
            inner: Box::new(inner),
        }
    }

    pub fn recv(&mut self, limit: usize) -> Result<Box<[u8]>, Box<dyn StreamError>> {
        self.inner.recv(limit)
    }

    pub fn close(&mut self, description: Description) -> Result<Result<(), ()>, MismatchedType> {
        self.inner.close(description)
    }

    pub fn is_open(&self) -> bool {
        self.inner.is_open()
    }
}
