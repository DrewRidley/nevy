pub use bevy_interface as bevy;

pub use nevy_messaging as messaging;

#[cfg(feature = "quic")]
pub use nevy_quic as quic;

#[cfg(feature = "web_transport")]
pub use nevy_web_transport as web_transport;

pub mod prelude {
    pub use bevy_interface::prelude::*;

    pub use nevy_messaging::prelude::*;

    #[cfg(feature = "quic")]
    pub use nevy_quic::prelude::*;

    #[cfg(feature = "web_transport")]
    pub use nevy_web_transport::prelude::*;
}
