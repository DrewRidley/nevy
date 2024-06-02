pub mod connection;
pub mod endpoint;
pub mod quinn_stream;

pub use quinn_proto;

pub mod quinn {
    pub use quinn_proto::VarInt;
}

pub mod prelude {
    pub use crate::connection::*;
    pub use crate::endpoint::*;
    pub use crate::quinn_stream::*;
}
