pub mod connection;
pub mod endpoint;

use std::collections::VecDeque;

use endpoint::*;
use transport_interface::EndpointEvent;

#[derive(Default)]
pub struct QuinnContext {
    pub(crate) events: VecDeque<EndpointEvent<QuinnEndpoint>>,
    pub(crate) recv_buffer: Vec<u8>,
    pub(crate) send_buffer: Vec<u8>,
}

impl QuinnContext {
    pub(crate) fn accept_connection(&mut self, _incoming: &quinn_proto::Incoming) -> bool {
        true
    }
}

pub mod quinn {
    pub use quinn_proto::VarInt;
}

pub mod prelude {
    pub use crate::connection::{QuinnConnection, QuinnConnectionId};
    pub use crate::endpoint::QuinnEndpoint;
    pub use crate::QuinnContext;
}
