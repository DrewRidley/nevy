use quinn_proto::StreamId;

// target_arch = "wasm32",
#[cfg(all(feature = "browser"))]
mod browser;

#[cfg(not(target_arch = "wasm32"))]
pub mod connection;
#[cfg(not(target_arch = "wasm32"))]
pub mod endpoint;

#[cfg(not(target_arch = "wasm32"))]
pub mod bevy;
#[cfg(not(target_arch = "wasm32"))]
pub mod web_transport;

use connection::*;
use endpoint::*;

#[cfg(not(target_arch = "wasm32"))]
pub mod prelude {
    pub use crate::connection::{ConnectionId, ConnectionState, WriteError};
    pub use crate::endpoint::{EndpointBuffers, EndpointState};
    pub use quinn_proto::StreamId;
}

pub use quinn_proto;


/// a trait for calling back endpoint events to the relevent implementation
pub trait EndpointEventHandler {
    /// called to ask if a new incoming connection should be accepted
    #[allow(unused_variables)]
    fn accept_connection(&mut self, incoming: &quinn_proto::Incoming) -> bool {
        true
    }

#[cfg(target_arch = "wasm32")]
pub mod prelude {}

/// a trait for calling back endpoint events to the relevent implementation
pub trait EndpointEventHandler {

#[cfg(target_arch = "wasm32")]
pub mod prelude {}

/// a trait for calling back endpoint events to the relevent implementation
pub trait EndpointEventHandler {
    /// a new connection has been established
    fn new_connection(&mut self, connection: &mut ConnectionState);

    /// a connection was lost or closed
    fn disconnected(&mut self, connection: &mut ConnectionState);

    /// the peer opened a new stream
    ///
    /// if the stream is bi directional and there is an associated send stream `bi_directional` will be `true`
    fn new_stream(
        &mut self,
        connection: &mut ConnectionState,
        stream_id: StreamId,
        bi_directional: bool,
    );

    /// a receive stream has been either finished or reset and no more data will be received
    ///
    /// if the stream was reset then an error message is supplied
    fn receive_stream_closed(
        &mut self,
        connection: &mut ConnectionState,
        stream_id: StreamId,
        reset_error: Option<quinn_proto::VarInt>,
    );
}

pub trait NewStreamHandler: Send + Sync {
    /// return false to close the stream and not return the stream id to the application
    fn new_stream(&self, connection: &mut ConnectionState, stream_id: StreamId, direction: quinn_proto::Dir) -> bool {
        true
    }
}
