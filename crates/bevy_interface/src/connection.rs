use bevy::prelude::*;
use transport_interface::*;

/// marker component for connections
///
/// will exist on all [BevyConnectionState]s,
/// but has no generic so it can be queried without that type info
#[derive(Component)]
pub struct BevyConnection;

/// component representing a connection on it's parent
/// [BevyEndpoint](crate::endpoint::BevyEndpoint)
///
/// use the [Connections](crate::endpoint::Connections)
/// system parameter to manage connections
#[derive(Component)]
pub struct BevyConnectionState<E: Endpoint>
where
    E::ConnectionId: Send + Sync,
{
    pub(crate) connection_id: E::ConnectionId,
}

impl<E: Endpoint> BevyConnectionState<E>
where
    E::ConnectionId: Send + Sync,
{
    pub(crate) fn new(connection_id: E::ConnectionId) -> Self {
        BevyConnectionState { connection_id }
    }
}
