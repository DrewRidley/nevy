use bevy::{prelude::*, utils::HashMap};
use transport_interface::*;

use crate::{BevyConnection, Connected, Disconnected};

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
pub struct Connections<'w, 's, E: Endpoint + Send + Sync + 'static>
where
    E::ConnectionId: Send + Sync + 'static,
{
    commands: Commands<'w, 's>,
    endpoint_q: Query<'w, 's, &'static mut BevyEndpoint<E>>,
    connection_q: Query<'w, 's, (&'static Parent, &'static BevyConnection<E>)>,
}

impl<'w, 's, E: Endpoint + Send + Sync + 'static> Connections<'w, 's, E>
where
    E::ConnectionId: Send + Sync,
{
    pub fn connect(&mut self, endpoint_entity: Entity, info: E::ConnectInfo) -> Option<Entity> {
        let mut endpoint = self.endpoint_q.get_mut(endpoint_entity).ok()?;

        let (connection_id, connection) = endpoint.endpoint.connect(info)?;
        let addr = connection.peer_addr();
        drop(connection);

        let connection_entity = self
            .commands
            .spawn(BevyConnection::<E>::new(connection_id))
            .id();

        debug!(
            "an Endpoint<{}> is connecting to {}",
            std::any::type_name::<E>(),
            addr
        );

        Some(connection_entity)
    }

    pub fn disconnect(&mut self, connection_entity: Entity) {
        let Ok((connection_parent, connection)) = self.connection_q.get(connection_entity) else {
            return;
        };

        let Ok(mut endpoint) = self.endpoint_q.get_mut(connection_parent.get()) else {
            return;
        };

        let _ = endpoint.endpoint.disconnect(connection.connection_id);
    }
}

pub(crate) fn update_endpoints<E: Endpoint + Send + Sync + 'static>(
    mut commands: Commands,
    mut endpoint_q: Query<(Entity, &mut BevyEndpoint<E>)>,
    mut connected_w: EventWriter<Connected>,
    mut disconnected_w: EventWriter<Disconnected>,
) where
    E::ConnectionId: Send + Sync,
{
    for (endpoint_entity, mut endpoint) in endpoint_q.iter_mut() {
        endpoint.endpoint.update();

        while let Some(EndpointEvent {
            connection_id,
            event,
        }) = endpoint.endpoint.poll_event()
        {
            match event {
                ConnectionEvent::Connected => {
                    let &mut connection_entity = endpoint
                        .connections
                        .entry(connection_id.clone())
                        .or_insert_with(|| {
                            commands.spawn(BevyConnection::<E>::new(connection_id)).id()
                        });

                    connected_w.send(Connected {
                        endpoint_entity,
                        connection_entity,
                    });
                }
                ConnectionEvent::Disconnected => {
                    if let Some(connection_entity) = endpoint.connections.remove(&connection_id) {
                        disconnected_w.send(Disconnected {
                            endpoint_entity,
                            connection_entity,
                        });
                    }
                }
            }
        }
    }
}
