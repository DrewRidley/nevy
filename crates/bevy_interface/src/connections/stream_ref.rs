use std::any::Any;

use transport_interface::*;

trait BevySendStreamInner<'s> {
    fn send(&mut self, data: &[u8]) -> Result<usize, Box<dyn Any>>;
}

pub struct BevySendStream<'s> {
    inner: Box<dyn BevySendStreamInner<'s> + 's>,
}

impl<'s, S: SendStreamMut<'s>> BevySendStreamInner<'s> for S {
    fn send(&mut self, data: &[u8]) -> Result<usize, Box<dyn Any>> {
        self.send(data)
            .map_err(|err| -> Box<dyn Any> { Box::new(err) })
    }
}

impl<'s> BevySendStream<'s> {
    pub fn send(&mut self, data: &[u8]) -> Result<usize, Box<dyn Any>> {
        self.inner.send(data)
    }
}
