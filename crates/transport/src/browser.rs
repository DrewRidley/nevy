use std::{error, fmt};

use bevy::{app::Plugin, log::warn, tasks::{block_on, AsyncComputeTaskPool, Task}};
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;
use web_sys::WebTransport;


#[derive(Debug)]
pub struct WebError {
    value: JsValue,
}

impl From<JsValue> for WebError {
    fn from(value: JsValue) -> Self {
        Self { value }
    }
}

impl error::Error for WebError {}

impl fmt::Display for WebError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
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


/// A WebTransport endpoint specifically designed for use in the browser via wasm.
///
/// A endpoint may exist without a connection to a server, so operations must be guarded accordingly.
#[derive(Default)]
pub struct BrowserEndpoint {
    inner: Option<web_sys::WebTransport>,
    // Whenever a connection request is made, the url is stored here.
    // The async system will actually instantiate the 'inner' value using this later.
    connect_task: Option<Task<Result<WebTransport, WebError>>>
}

impl BrowserEndpoint {
    /// Blocks until the endpoint is successfully connected.
    fn connect(&mut self, url: String) -> Result<(), WebError> {
        if self.connect_task.is_some() {
            warn!("Attempted to connect to a server while there is already an existing connection!\nDropping in favor of new connection.");
        }

        let pool = AsyncComputeTaskPool::get();
        self.connect_task = Some(pool.spawn_local(async move {
            let inner = web_sys::WebTransport::new(&url)?;
            JsFuture::from(inner.ready()).await?;

            Ok(inner)
        }));


        Ok(())
    }
}


fn connect_sys() {

}


pub struct BrowserEndpointPlugin;

impl Plugin for BrowserEndpointPlugin {
    fn build(&self, app: &mut bevy::prelude::App) {

    }
}