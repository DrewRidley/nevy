use bevy::{prelude::*, utils::HashMap};
use transport_interface::*;

use crate::{connection::BevyConnection, Connected, Disconnected};

/// the component that holds state and represents a networking endpoint
///
/// use the [Connections] system parameter to manage connections
#[derive(Component)]
pub struct BevyEndpoint<E: Endpoint> {
    pub(crate) endpoint: E,
    pub(crate) connections: HashMap<E::ConnectionId, Entity>,
}

impl<E: Endpoint> BevyEndpoint<E> {
    pub fn new(endpoint: E) -> Self {
        BevyEndpoint {
            endpoint,
            connections: HashMap::new(),
        }
    }
}

/// system parameter used for managing [BevyConnection]s on [BevyEndpoint]s
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
    /// calls the connect method on the internal endpoint.
    /// if successful will spawn a [BevyConnection] as a child of the endpoint and return it
    pub fn connect(&mut self, endpoint_entity: Entity, info: E::ConnectInfo) -> Option<Entity> {
        let mut endpoint = self.endpoint_q.get_mut(endpoint_entity).ok()?;

        let (connection_id, _) = endpoint.endpoint.connect(info)?;

        let connection_entity = self
            .commands
            .spawn(BevyConnection::<E>::new(connection_id))
            .set_parent(endpoint_entity)
            .id();

        debug!(
            "Endpoint<{}> {:?} is making a connection",
            std::any::type_name::<E>(),
            endpoint_entity,
        );

        Some(connection_entity)
    }

    /// attempts to disconnect a connection
    ///
    /// will do nothing if the connection does not exist or it's parent isn't an endpoint
    pub fn disconnect(&mut self, connection_entity: Entity) {
        let Ok((connection_parent, connection)) = self.connection_q.get(connection_entity) else {
            return;
        };

        let endpoint_entity = connection_parent.get();

        let Ok(mut endpoint) = self.endpoint_q.get_mut(endpoint_entity) else {
            error!(
                "connection {:?}'s parent {:?} could not be queried as an endpoint. ({})",
                connection_entity,
                endpoint_entity,
                std::any::type_name::<E>()
            );
            return;
        };

        let _ = endpoint.endpoint.disconnect(connection.connection_id);
    }

    /// returns the stats for some connection if it exists
    pub fn get_stats<'c>(&'c self, connection_entity: Entity) -> Option<<<E::Connection<'c> as ConnectionMut<'c>>::NonMut<'c> as ConnectionRef<'c>>::ConnectionStats>{
        let Ok((connection_parent, connection)) = self.connection_q.get(connection_entity) else {
            return None;
        };

        let endpoint_entity = connection_parent.get();

        let Ok(endpoint) = self.endpoint_q.get(endpoint_entity) else {
            error!(
                "connection {:?}'s parent {:?} could not be queried as an endpoint. ({})",
                connection_entity,
                endpoint_entity,
                std::any::type_name::<E>()
            );
            return None;
        };

        Some(
            endpoint
                .endpoint
                .connection(connection.connection_id)?
                .get_stats(),
        )
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
                            commands
                                .spawn(BevyConnection::<E>::new(connection_id))
                                .set_parent(endpoint_entity)
                                .id()
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
