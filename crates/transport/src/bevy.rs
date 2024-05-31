
use bevy::{ecs::schedule::ScheduleLabel, prelude::*, utils::intern::Interned};

use crate::{prelude::*, EndpointEventHandler};


/// adds events and functionality for the [Endpoint] component
pub struct BevyEndpointPlugin {
    schedule: Interned<dyn ScheduleLabel>,
}

impl BevyEndpointPlugin {
    pub fn new(schedule: impl ScheduleLabel) -> Self {
        BevyEndpointPlugin {
            schedule: schedule.intern(),
        }
    }
}

impl Default for BevyEndpointPlugin {
    fn default() -> Self {
        BevyEndpointPlugin::new(PreUpdate)
    }
}


impl Plugin for BevyEndpointPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<Connected>();
        app.add_event::<Disconnected>();
        app.add_event::<NewStream>();
        app.add_event::<ReceiveStreamClosed>();

        app.add_systems(self.schedule, update_endpoints);
    }
}



/// when inserted on the same entity as a [NativeEndpoint] the endpoint will be updated and events will be fired
#[derive(Component)]
pub struct BevyEndpoint;

fn update_endpoints(
    mut endpoint_q: Query<(Entity, &mut EndpointState), With<BevyEndpoint>>,
    mut events: BevyEndpointEvents,
    mut buffers: Local<EndpointBuffers>,
) {
    for (endpoint_entity, mut endpoint) in endpoint_q.iter_mut() {
        endpoint.update(&mut buffers, &mut events.handler(endpoint_entity));
    }
}



#[derive(Event)]
pub struct Connected {
    pub endpoint_entity: Entity,
    pub connection_id: ConnectionId,
}

#[derive(Event)]
pub struct Disconnected {
    pub endpoint_entity: Entity,
    pub connection_id: ConnectionId,
}

#[derive(Event)]
pub struct NewStream {
    pub endpoint_entity: Entity,
    pub connection_id: ConnectionId,
    pub stream_id: StreamId,
    pub bi_directional: bool,
}

#[derive(Event)]
pub struct ReceiveStreamClosed {
    pub endpoint_entity: Entity,
    pub connection_id: ConnectionId,
    pub stream_id: StreamId,
    pub reset_error: Option<quinn_proto::VarInt>,
}

#[derive(bevy::ecs::system::SystemParam)]
pub struct BevyEndpointEvents<'w> {
    pub connected: EventWriter<'w, Connected>,
    pub disconnected: EventWriter<'w, Disconnected>,
    pub new_stream: EventWriter<'w, NewStream>,
    pub receive_stream_closed: EventWriter<'w, ReceiveStreamClosed>,
}

pub struct BevyEndpointEventHandler<'a, 'w> {
    pub events: &'a mut BevyEndpointEvents<'w>,
    pub endpoint_entity: Entity,
}

impl<'w> BevyEndpointEvents<'w> {
    pub fn handler<'a>(&'a mut self, endpoint_entity: Entity) -> BevyEndpointEventHandler<'a, 'w> {
        BevyEndpointEventHandler {
            events: self,
            endpoint_entity,
        }
    }
}

impl<'a, 'w> EndpointEventHandler for BevyEndpointEventHandler<'a, 'w> {
    fn new_connection(&mut self, connection: &mut ConnectionState) {
        self.events.connected.send(Connected {
            endpoint_entity: self.endpoint_entity,
            connection_id: connection.connection_id(),
        });
    }

    fn disconnected(&mut self, connection: &mut ConnectionState) {
        self.events.disconnected.send(Disconnected {
            endpoint_entity: self.endpoint_entity,
            connection_id: connection.connection_id(),
        });
    }

    fn new_stream(&mut self, connection: &mut ConnectionState, stream_id: StreamId, bi_directional: bool) {
        self.events.new_stream.send(NewStream {
            endpoint_entity: self.endpoint_entity,
            connection_id: connection.connection_id(),
            stream_id,
            bi_directional,
        });
    }

    fn receive_stream_closed(&mut self, connection: &mut ConnectionState, stream_id: StreamId, reset_error: Option<quinn_proto::VarInt>) {
        self.events.receive_stream_closed.send(ReceiveStreamClosed {
            endpoint_entity: self.endpoint_entity,
            connection_id: connection.connection_id(),
            stream_id,
            reset_error,
        });
    }
}
