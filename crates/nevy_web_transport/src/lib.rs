pub mod connection;
pub mod endpoint;
pub mod streams;

pub mod prelude {
    pub use crate::endpoint::WebTransportEndpoint;

    pub use crate::connection::{WebTransportConnectionMut, WebTransportConnectionRef};

    pub use crate::streams::{
        WebTransportRecvStreamMut, WebTransportSendStreamMut, WebTransportStreamId,
    };
}
