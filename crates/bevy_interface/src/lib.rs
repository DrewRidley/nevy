use std::marker::PhantomData;

use bevy::{ecs::schedule::ScheduleLabel, prelude::*, utils::intern::Interned};
use transport_interface::*;

pub mod connection;
pub mod endpoint;
pub mod streams;

pub mod prelude {
    pub use crate::connection::BevyConnection;
    pub use crate::endpoint::{BevyEndpointState, Connections};
    pub use crate::{Connected, Disconnected, EndpointPlugin};
}

/// adds events and update loop for
/// [BevyEndpoint] and [BevyConnection]
/// with an endpoint backend `E`
pub struct EndpointPlugin<E> {
    _p: PhantomData<E>,
    schedule: Interned<dyn ScheduleLabel>,
}

impl<E> EndpointPlugin<E> {
    /// creates a new [EndpointPlugin] that updates in a certain schedule
    fn new(schedule: impl ScheduleLabel) -> Self {
        EndpointPlugin {
            _p: PhantomData,
            schedule: schedule.intern(),
        }
    }
}

impl<E> Default for EndpointPlugin<E> {
    fn default() -> Self {
        EndpointPlugin::new(PreUpdate)
    }
}

impl<E: Endpoint + Send + Sync + 'static> Plugin for EndpointPlugin<E>
where
    E::ConnectionId: Send + Sync,
{
    fn build(&self, app: &mut App) {
        app.add_event::<Connected>();
        app.add_event::<Disconnected>();

        app.add_systems(
            self.schedule,
            (
                endpoint::insert_missing_bevy_endpoints::<E>,
                endpoint::update_endpoints::<E>,
            ),
        );
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
