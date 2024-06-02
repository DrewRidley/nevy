use std::marker::PhantomData;

use bevy::{ecs::schedule::ScheduleLabel, prelude::*, utils::intern::Interned};
use transport_interface::*;

pub mod connection;
pub mod endpoint;
pub mod streams;

use connection::*;
use endpoint::*;

pub struct EndpointPlugin<E> {
    _p: PhantomData<E>,
    schedule: Interned<dyn ScheduleLabel>,
}

impl<E> EndpointPlugin<E> {
    fn new(schedule: impl ScheduleLabel) -> Self {
        EndpointPlugin {
            _p: PhantomData,
            schedule: schedule.intern(),
        }
    }
}

impl<E: transport_interface::Endpoint + Send + Sync + 'static> Plugin for EndpointPlugin<E> {
    fn build(&self, app: &mut App) {
        app.add_event::<Connected>();
    }
}

/// fired when a connection is successful
#[derive(Event)]
pub struct Connected {
    pub endpoint_entity: Entity,
    pub connection_entity: Entity,
}
