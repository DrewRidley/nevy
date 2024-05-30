
use quinn_proto::StreamId;

#[cfg(feature = "browser")]
mod browser;

pub mod endpoint;
pub mod connection;

pub mod bevy;
pub mod web_transport;

use connection::*;
use endpoint::*;


pub mod prelude {
    pub use quinn_proto::StreamId;
    pub use crate::endpoint::{EndpointState, EndpointBuffers};
    pub use crate::connection::{ConnectionState, ConnectionId, WriteError};
}


/// a trait for calling back endpoint events to the relevent implementation
pub trait EndpointEventHandler {
    /// called to ask if a new incoming connection should be accepted
    fn accept_connection(&mut self, incoming: &quinn_proto::Incoming) -> bool {
        true
    }

    /// a new connection has been established
    fn new_connection(&mut self, connection: &mut ConnectionState);

    /// a connection was lost or closed
    fn disconnected(&mut self, connection: &mut ConnectionState);

    /// the peer opened a new stream
    ///
    /// if the stream is bi directional and there is an associated send stream `bi_directional` will be `true`
    fn new_stream(&mut self, connection: &mut ConnectionState, stream_id: StreamId, bi_directional: bool);

    /// a receive stream has been either finished or reset and no more data will be received
    ///
    /// if the stream was reset then an error message is supplied
    fn receive_stream_closed(&mut self, connection: &mut ConnectionState, stream_id: StreamId, reset_error: Option<quinn_proto::VarInt>);
}



// pub struct NativeEndpointPlugin;

// impl Plugin for NativeEndpointPlugin {
//     fn build(&self, app: &mut App) {
//         app
//         .add_event::<NewWriteStream>()
//         .add_event::<NewReadStream>()
//         .add_event::<ClosedStream>()
//         .add_event::<Connected>()
//         .add_event::<Disconnected>()
//         .add_systems(Update, (endpoint_poll_sys, connection_poll_sys));
//     }
// }
