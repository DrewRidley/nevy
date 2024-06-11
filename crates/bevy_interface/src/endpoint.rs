use std::any::Any;

use bevy::{prelude::*, utils::HashMap};
use transport_interface::*;

use crate::{
    connection::{BevyConnection, BevyConnectionState},
    Connected, Disconnected,
};

/// the component that holds state and represents a networking endpoint
///
/// use the [Connections] system parameter to manage connections
#[derive(Component)]
pub struct BevyEndpoint {
    state: Box<dyn Any + Send + Sync + 'static>,
}

impl BevyEndpoint {
    fn get<E: 'static>(&self) -> Option<&E> {
        self.state.downcast_ref()
    }

    fn get_mut<E: 'static>(&mut self) -> Option<&mut E> {
        self.state.downcast_mut()
    }
}

pub struct BevyEndpointState<E: Endpoint> {
    pub(crate) endpoint: E,
    pub(crate) connections: HashMap<E::ConnectionId, Entity>,
}

impl<E: Endpoint> BevyEndpointState<E> {
    pub fn new(endpoint: E) -> Self {
        BevyEndpointState {
            endpoint,
            connections: HashMap::new(),
        }
    }
}

#[derive(bevy::ecs::system::SystemParam)]
pub(crate) struct ConnectionQuery<'w, 's, E: Endpoint + Send + Sync + 'static>
where
    E::ConnectionId: Send + Sync + 'static,
{
    pub endpoint_q: Query<'w, 's, &'static mut BevyEndpoint>,
    pub connection_q: Query<'w, 's, (&'static Parent, &'static BevyConnectionState<E>)>,
}

impl<'w, 's, E: Endpoint + Send + Sync + 'static> ConnectionQuery<'w, 's, E>
where
    E::ConnectionId: Send + Sync,
{
    pub fn endpoint_of_connection<'a>(
        &'a self,
        connection_entity: Entity,
    ) -> Option<(&'a BevyEndpointState<E>, E::ConnectionId)> {
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

        Some((endpoint, connection.connection_id))
    }

    pub fn endpoint_of_connection_mut<'a>(
        &'a mut self,
        connection_entity: Entity,
    ) -> Option<(Mut<'a, BevyEndpointState<E>>, E::ConnectionId)> {
        let Ok((connection_parent, connection)) = self.connection_q.get(connection_entity) else {
            return None;
        };

        let endpoint_entity = connection_parent.get();

        let Ok(endpoint) = self.endpoint_q.get_mut(endpoint_entity) else {
            error!(
                "connection {:?}'s parent {:?} could not be queried as an endpoint. ({})",
                connection_entity,
                endpoint_entity,
                std::any::type_name::<E>()
            );
            return None;
        };

        Some((endpoint, connection.connection_id))
    }
}

/// system parameter used for managing [BevyConnection]s on [BevyEndpoint]s
#[derive(bevy::ecs::system::SystemParam)]
pub struct Connections<'w, 's, E: Endpoint + Send + Sync + 'static>
where
    E::ConnectionId: Send + Sync + 'static,
{
    commands: Commands<'w, 's>,
    query: ConnectionQuery<'w, 's, E>,
}

impl<'w, 's, E: Endpoint + Send + Sync + 'static> Connections<'w, 's, E>
where
    E::ConnectionId: Send + Sync,
{
    /// calls the connect method on the internal endpoint.
    /// if successful will spawn a [BevyConnection] as a child of the endpoint and return it
    pub fn connect<'i>(
        &mut self,
        endpoint_entity: Entity,
        info: E::ConnectInfo<'i>,
    ) -> Option<Entity> {
        let mut endpoint = self.query.endpoint_q.get_mut(endpoint_entity).ok()?;

        let (connection_id, _) = endpoint.endpoint.connect(info)?;

        let connection_entity = self
            .commands
            .spawn(BevyConnectionState::<E>::new(connection_id))
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
        let Some((mut endpoint, connection_id)) =
            self.query.endpoint_of_connection_mut(connection_entity)
        else {
            return;
        };

        let _ = endpoint.endpoint.disconnect(connection_id);
    }

    /// returns the stats for some connection if it exists
    pub fn get_stats<'c>(&'c self, connection_entity: Entity) -> Option<<<E::Connection<'c> as ConnectionMut<'c>>::NonMut<'c> as ConnectionRef<'c>>::ConnectionStats>{
        let (endpoint, connection_id) = self.query.endpoint_of_connection(connection_entity)?;

        Some(endpoint.endpoint.connection(connection_id)?.get_stats())
    }
}

pub(crate) fn insert_missing_bevy_endpoints<E>(
    mut commands: Commands,
    endpoint_q: Query<Entity, (With<BevyEndpointState<E>>, Without<BevyEndpoint>)>,
) where
    E: Endpoint,
    BevyEndpointState<E>: Component,
{
    for entity in endpoint_q.iter() {
        commands.entity(entity).insert(BevyEndpoint);
    }
}

#[derive(bevy::ecs::system::SystemParam)]
pub(crate) struct HandlerParams<'w, 's> {
    commands: Commands<'w, 's>,
    connected_w: EventWriter<'w, Connected>,
    disconnected_w: EventWriter<'w, Disconnected>,
}

/// the event handler for updating endpoints in bevy
struct Handler<'a, 'w, 's, E: Endpoint> {
    params: &'a mut HandlerParams<'w, 's>,
    accept_inoming: bool,
    endpoint_entity: Entity,
    connections: &'a mut HashMap<E::ConnectionId, Entity>,
}

impl<'a, 'w, 's, E: Endpoint> EndpointEventHandler<E> for Handler<'a, 'w, 's, E>
where
    E: 'static,
    E::ConnectionId: Send + Sync,
{
    fn connection_request<'i>(
        &mut self,
        _request: <E as Endpoint>::IncomingConnectionInfo<'i>,
    ) -> bool {
        self.accept_inoming
    }

    fn connected(&mut self, connection_id: <E as Endpoint>::ConnectionId) {
        let &mut connection_entity = self
            .connections
            .entry(connection_id.clone())
            .or_insert_with(|| {
                self.params
                    .commands
                    .spawn((BevyConnectionState::<E>::new(connection_id), BevyConnection))
                    .set_parent(self.endpoint_entity)
                    .id()
            });

        self.params.connected_w.send(Connected {
            endpoint_entity: self.endpoint_entity,
            connection_entity,
        });
    }

    fn disconnected(&mut self, connection_id: <E as Endpoint>::ConnectionId) {
        if let Some(connection_entity) = self.connections.remove(&connection_id) {
            self.params.disconnected_w.send(Disconnected {
                endpoint_entity: self.endpoint_entity,
                connection_entity,
            });
        }
    }
}

pub(crate) fn update_endpoints<E: Endpoint + Send + Sync + 'static>(
    mut params: HandlerParams,
    mut endpoint_q: Query<(Entity, &mut BevyEndpointState<E>)>,
) where
    E::ConnectionId: Send + Sync,
{
    for (endpoint_entity, mut endpoint) in endpoint_q.iter_mut() {
        let endpoint = endpoint.as_mut();
        endpoint.endpoint.update(&mut Handler {
            params: &mut params,
            accept_inoming: true, // TODO: add api
            endpoint_entity,
            connections: &mut endpoint.connections,
        });
    }
}
