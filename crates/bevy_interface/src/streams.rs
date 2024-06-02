use std::marker::PhantomData;

use bevy::{ecs::schedule::ScheduleLabel, prelude::*, utils::intern::Interned};
use transport_interface::*;

use crate::prelude::{BevyConnection, BevyEndpoint};

pub struct StreamPlugin<E, S> {
    _p: PhantomData<(E, S)>,
    schedule: Interned<dyn ScheduleLabel>,
}

impl<E, S> StreamPlugin<E, S> {
    fn new(schedule: impl ScheduleLabel) -> Self {
        StreamPlugin {
            _p: PhantomData,
            schedule: schedule.intern(),
        }
    }
}

impl<E, S> Default for StreamPlugin<E, S> {
    fn default() -> Self {
        StreamPlugin::new(PreUpdate)
    }
}

impl<E, S> Plugin for StreamPlugin<E, S>
where
    E: Endpoint + Send + Sync + 'static,
    E::ConnectionId: Send + Sync,
    S: for<'c> StreamId<Connection<'c> = E::Connection<'c>> + Send + Sync + 'static,
{
    fn build(&self, app: &mut App) {
        app.add_event::<NewSendStream<S>>();
        app.add_event::<NewRecvStream<S>>();
        app.add_event::<ClosedSendStream<S>>();
        app.add_event::<ClosedRecvStream<S>>();

        app.add_systems(self.schedule, update_streams::<E, S>);
    }
}

#[derive(Event)]
pub struct NewSendStream<S> {
    pub endpoint_entity: Entity,
    pub connection_entity: Entity,
    pub stream_id: S,
    pub peer_generated: bool,
}

#[derive(Event)]
pub struct NewRecvStream<S> {
    pub endpoint_entity: Entity,
    pub connection_entity: Entity,
    pub stream_id: S,
    pub peer_generated: bool,
}

#[derive(Event)]
pub struct ClosedSendStream<S> {
    pub endpoint_entity: Entity,
    pub connection_entity: Entity,
    pub stream_id: S,
    pub peer_generated: bool,
}

#[derive(Event)]
pub struct ClosedRecvStream<S> {
    pub endpoint_entity: Entity,
    pub connection_entity: Entity,
    pub stream_id: S,
    pub peer_generated: bool,
}

fn update_streams<E, S>(
    mut endpoint_q: Query<&mut BevyEndpoint<E>>,
    connection_q: Query<(Entity, &Parent, &BevyConnection<E>)>,
    mut new_send_stream_w: EventWriter<NewSendStream<S>>,
    mut new_recv_stream_w: EventWriter<NewRecvStream<S>>,
    mut closed_send_stream_w: EventWriter<ClosedSendStream<S>>,
    mut closed_recv_stream_w: EventWriter<ClosedRecvStream<S>>,
) where
    E: Endpoint + Send + Sync + 'static,
    E::ConnectionId: Send + Sync,
    S: for<'c> StreamId<Connection<'c> = E::Connection<'c>> + Send + Sync + 'static,
{
    for (connection_entity, connection_parent, connection) in connection_q.iter() {
        let endpoint_entity = connection_parent.get();

        let Ok(mut endpoint) = endpoint_q.get_mut(endpoint_entity) else {
            error!(
                "connection {:?}'s parent {:?} could not be queried as an endpoint. ({})",
                connection_entity,
                endpoint_entity,
                std::any::type_name::<E>()
            );
            continue;
        };

        let connection_id = connection.connection_id;

        let Some(mut connection) = endpoint.endpoint.connection_mut(connection_id) else {
            continue;
        };

        while let Some(StreamEvent {
            stream_id,
            peer_generated,
            event_type,
        }) = connection.poll_stream_events::<S>()
        {
            match event_type {
                StreamEventType::NewSendStream => {
                    new_send_stream_w.send(NewSendStream {
                        endpoint_entity,
                        connection_entity,
                        stream_id,
                        peer_generated,
                    });
                }
                StreamEventType::ClosedSendStream => {
                    closed_send_stream_w.send(ClosedSendStream {
                        endpoint_entity,
                        connection_entity,
                        stream_id,
                        peer_generated,
                    });
                }
                StreamEventType::NewRecvStream => {
                    new_recv_stream_w.send(NewRecvStream {
                        endpoint_entity,
                        connection_entity,
                        stream_id,
                        peer_generated,
                    });
                }
                StreamEventType::ClosedRecvStream => {
                    closed_recv_stream_w.send(ClosedRecvStream {
                        endpoint_entity,
                        connection_entity,
                        stream_id,
                        peer_generated,
                    });
                }
            }
        }
    }
}

#[derive(bevy::ecs::system::SystemParam)]
pub struct Streams<'w, 's, E>
where
    E: Endpoint + Send + Sync + 'static,
    E::ConnectionId: Send + Sync,
{
    endpoint_q: Query<'w, 's, &'static mut BevyEndpoint<E>>,
    connection_q: Query<'w, 's, &'static BevyConnection<E>>,
}

impl<'w, 's, E> Streams<'w, 's, E>
where
    E: Endpoint + Send + Sync + 'static,
    E::ConnectionId: Send + Sync,
{
}
