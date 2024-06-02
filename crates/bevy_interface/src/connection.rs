use bevy::prelude::*;
use transport_interface::*;

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
