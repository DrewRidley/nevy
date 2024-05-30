use bevy::prelude::*;
use connection::*;
use endpoint::endpoint_poll_sys;

#[cfg(feature = "browser")]
mod browser;

mod endpoint;
mod connection;

pub use endpoint::{NativeEndpoint, NativeEndpointPlugin};


pub mod prelude {
    pub use crate::endpoint::{NativeEndpoint, NativeEndpointPlugin};
}