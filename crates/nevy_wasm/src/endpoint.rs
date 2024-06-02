use transport_interface::{ConnectionMut, Endpoint};
use wasm_bindgen::JsValue;

#[derive(Debug)]
pub struct WebError {
    value: JsValue,
}

impl From<JsValue> for WebError {
    fn from(value: JsValue) -> Self {
        Self { value }
    }
}

impl std::fmt::Display for WebError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Print out the JsValue as a string
        match self.value.as_string() {
            Some(s) => write!(f, "{}", s),
            None => write!(f, "{:?}", self.value),
        }
    }
}

impl From<&str> for WebError {
    fn from(value: &str) -> Self {
        Self {
            value: value.into(),
        }
    }
}

pub struct WasmEndpoint {
    inner: Option<web_sys::WebTransport>,
}

impl WasmEndpoint {
    fn new(url: &str) -> Result<Self, WebError> {
        Ok(Self {
            inner: Some(web_sys::WebTransport::new(url)?),
        })
    }
}

impl<'c> ConnectionMut<'c> for &'c mut WasmEndpoint {
    type NonMut<'b> = &'b WasmEndpoint;

    fn as_ref<'b>(&'b self) -> Self::NonMut<'b> {
        todo!()
    }

    fn disconnect(&mut self) {
        todo!()
    }
}

impl Endpoint for WasmEndpoint {
    type Connection<'c> = &'c mut Self;

    type ConnectionId = ();
    type ConnectInfo = ();

    fn update(&mut self) {}

    fn connection<'c>(
        &'c self,
        id: Self::ConnectionId,
    ) -> Option<<Self::Connection<'c> as transport_interface::ConnectionMut>::NonMut<'c>> {
    }

    fn connection_mut<'a>(&'a mut self, id: Self::ConnectionId) -> Option<Self::Connection<'a>> {
        None
    }

    fn connect<'c>(
        &'c mut self,
        info: Self::ConnectInfo,
    ) -> Option<(Self::ConnectionId, Self::Connection<'c>)> {
        todo!()
    }

    fn poll_event(&mut self) -> Option<transport_interface::EndpointEvent<Self>>
    where
        Self: Sized,
    {
        todo!()
    }
}
