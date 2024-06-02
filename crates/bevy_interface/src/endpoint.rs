use bevy::{prelude::*, utils::HashMap};
use transport_interface::*;

use crate::BevyConnection;

#[derive(Component)]
pub struct BevyEndpoint<E: Endpoint> {
    endpoint: E,
    connections: HashMap<E::ConnectionId, Entity>,
}

impl<E: Endpoint> BevyEndpoint<E> {
    pub fn new(endpoint: E) -> Self {
        BevyEndpoint {
            endpoint,
            connections: HashMap::new(),
        }
    }
}

/// system parameter used for connecting and disconnecting endpoints
#[derive(bevy::ecs::system::SystemParam)]
pub struct Endpoints<'w, 's, E: Endpoint + Send + Sync + 'static>
where
    E::ConnectionId: Send + Sync + 'static,
{
    commands: Commands<'w, 's>,
    endpoint_q: Query<'w, 's, &'static mut BevyEndpoint<E>>,
}

impl<'w, 's, E: Endpoint + Send + Sync + 'static> Endpoints<'w, 's, E>
where
    E::ConnectionId: Send + Sync + 'static,
{
    pub fn connect(&mut self, endpoint_entity: Entity, info: E::ConnectInfo) -> Option<Entity> {
        let mut endpoint = self.endpoint_q.get_mut(endpoint_entity).ok()?;

        let connection_id = endpoint.endpoint.connect(info)?;

        let connection_entity = self
            .commands
            .spawn(BevyConnection::<E>::new(connection_id))
            .id();

        Some(connection_entity)
    }
}

pub(crate) fn update_endpoints<E: Endpoint + Send + Sync + 'static>(
    mut commands: Commands,
    mut endpoint_q: Query<(Entity, &mut BevyEndpoint<E>)>,
) {
    for (endpoint_entity, mut endpoint) in endpoint_q.iter_mut() {
        endpoint.endpoint.update();

        while let Some(EndpointEvent {
            connection_id,
            event,
        }) = endpoint.endpoint.poll_event()
        {
            match event {
                ConnectionEvent::Connected => {
                    let connection_entity = endpoint
                        .connections
                        .entry(connection_id.clone())
                        .or_insert_with(|| {
                            commands.spawn(BevyConnection::<E>::new(connection_id)).id()
                        });
                }
                ConnectionEvent::Disconnected => {}
            }
        }
    }
}
