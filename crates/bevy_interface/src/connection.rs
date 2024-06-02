use bevy::prelude::*;
use transport_interface::*;

/// component representing a connection on it's parent
/// [BevyEndpoint](crate::endpoint::BevyEndpoint)
///
/// use the [Connections](crate::endpoint::Connections)
/// system parameter to manage connections
#[derive(Component)]
pub struct BevyConnection<E: Endpoint>
where
    E::ConnectionId: Send + Sync,
{
    pub(crate) connection_id: E::ConnectionId,
}

impl<E: Endpoint> BevyConnection<E>
where
    E::ConnectionId: Send + Sync,
{
    pub(crate) fn new(connection_id: E::ConnectionId) -> Self {
        BevyConnection { connection_id }
    }
}
