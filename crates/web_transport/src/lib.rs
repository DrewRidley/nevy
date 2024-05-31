//! this is just jacob playing with some ideas. just ignore this crate



use bevy::{ecs::schedule::ScheduleLabel, prelude::*, utils::intern::Interned};
use transport::prelude::*;


/// adds web transport functionality for the [WebTransportEndpoint] component
///
/// depends on [BevyEndpointPlugin]
pub struct WebTransportEndpointPlugin {
    schedule: Interned<dyn ScheduleLabel>,
}

impl WebTransportEndpointPlugin {
    pub fn new(schedule: impl ScheduleLabel) -> Self {
        WebTransportEndpointPlugin {
            schedule: schedule.intern(),
        }
    }
}

impl Default for WebTransportEndpointPlugin {
    fn default() -> Self {
        WebTransportEndpointPlugin::new(PreUpdate)
    }
}

impl Plugin for WebTransportEndpointPlugin {
    fn build(&self, app: &mut App) {
    }
}



/// when on the same entity as an [EndpointState] it will operate as a web transport endpoint
#[derive(Component, Default)]
pub struct WebTransportEndpoint {
    // Connections that have not completed WebTransport negotiations.
    uninitialized_connections: std::collections::HashMap<ConnectionId, Entity>,
}

struct WebTransportEventHandler {

}

impl transport::EndpointEventHandler for WebTransportEventHandler {
    fn new_connection(&mut self, connection: &mut ConnectionState) {
        todo!()
    }

    fn disconnected(&mut self, connection: &mut ConnectionState) {
        todo!()
    }

    fn new_stream(&mut self, connection: &mut ConnectionState, stream_id: StreamId, bi_directional: bool) {
        todo!()
    }

    fn receive_stream_closed(&mut self, connection: &mut ConnectionState, stream_id: StreamId, reset_error: Option<transport::quinn_proto::VarInt>) {
        todo!()
    }
}

