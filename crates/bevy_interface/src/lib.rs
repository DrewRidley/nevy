use bevy::{ecs::schedule::ScheduleLabel, prelude::*, utils::intern::Interned};

pub mod connections;
pub mod description;
pub mod endpoint;
pub mod stream_headers;

pub mod prelude {
    pub use crate::connections::{BevyStreamEvent, BevyStreamId};
    pub use crate::description::{CloneableDescription, Description};
    pub use crate::endpoint::{BevyConnection, BevyEndpoint, ConnectError, Connections};
    pub use crate::stream_headers::{
        headers::{HeaderId, HeaderPlugin},
        EndpointStreamHeaders, HeaderStreamEvent, HeaderStreamEventType, HeaderStreamId,
        StreamHeaderPlugin,
    };
    pub use crate::{Connected, Disconnected, EndpointPlugin};
    pub use transport_interface::StreamEventType;
}

#[derive(Debug)]
pub struct MismatchedType {
    pub expected: &'static str,
}

/// adds events and update loop for
/// [BevyEndpoint] and [BevyConnection]
pub struct EndpointPlugin {
    schedule: Interned<dyn ScheduleLabel>,
}

impl EndpointPlugin {
    /// creates a new [EndpointPlugin] that updates in a certain schedule
    fn new(schedule: impl ScheduleLabel) -> Self {
        EndpointPlugin {
            schedule: schedule.intern(),
        }
    }
}

impl Default for EndpointPlugin {
    fn default() -> Self {
        EndpointPlugin::new(PreUpdate)
    }
}

impl Plugin for EndpointPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<Connected>();
        app.add_event::<Disconnected>();

        app.add_systems(self.schedule, endpoint::update_endpoints);
    }
}

/// fired when a connection is successful
#[derive(Event)]
pub struct Connected {
    pub endpoint_entity: Entity,
    pub connection_entity: Entity,
}

/// fired when an existing connection has disconnected
///
/// a matching [Connected] event will not have been fired if this was a connection attempt that failed
#[derive(Event)]
pub struct Disconnected {
    pub endpoint_entity: Entity,
    pub connection_entity: Entity,
}
