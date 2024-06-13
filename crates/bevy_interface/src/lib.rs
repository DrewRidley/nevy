use bevy::{ecs::schedule::ScheduleLabel, prelude::*, utils::intern::Interned};

pub mod connections;
pub mod endpoint;

pub mod prelude {
    pub use crate::connections::{BevyStreamId, MismatchedStreamType, StreamDescription};
    pub use crate::endpoint::{
        BevyConnection, BevyEndpoint, ConnectError, Connections, MismatchedEndpointType,
    };
    pub use crate::{Connected, Disconnected, EndpointPlugin};
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
