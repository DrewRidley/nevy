use transport_interface::*;

pub struct WasmConnection {
    inner: web_sys::WebTransport,
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

impl<'c> ConnectionRef<'c> for &'c WasmConnection {}
