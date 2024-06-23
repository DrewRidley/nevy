use std::{collections::VecDeque, marker::PhantomData};

use bevy::{ecs::schedule::ScheduleLabel, prelude::*, utils::intern::Interned};
use bevy_interface::prelude::*;
use serde::de::DeserializeOwned;

/// Adds message deserialization functionality
pub struct MessageDeserializationPlugin<C> {
    _p: PhantomData<C>,
    schedule: Interned<dyn ScheduleLabel>,
    messages: Vec<Box<dyn MessageIdBuilder<C>>>,
}

trait MessageIdBuilder<C>: Send + Sync + 'static {
    fn build(&self, schedule: Interned<dyn ScheduleLabel>, message_id: u16, app: &mut App);
}

struct MessageIdBuilderType<T> {
    _p: PhantomData<T>,
}

impl<C> MessageDeserializationPlugin<C> {
    pub fn new(schedule: impl ScheduleLabel) -> Self {
        MessageDeserializationPlugin {
            _p: PhantomData,
            schedule: schedule.intern(),
            messages: Vec::new(),
        }
    }
}

impl<C: Component> MessageDeserializationPlugin<C> {
    /// adds a message type to the plugin, assigning it the next message id
    pub fn add_message<T: DeserializeOwned + Send + Sync + 'static>(&mut self) -> &mut Self {
        self.messages
            .push(Box::new(MessageIdBuilderType::<T> { _p: PhantomData }));

        self
    }
}

impl<C: Component> Plugin for MessageDeserializationPlugin<C> {
    fn build(&self, app: &mut App) {
        app.add_systems(
            self.schedule,
            (
                insert_connection_components::<C>,
                receive_message_streams::<C>,
                read_message_streams::<C>,
            ),
        );

        for (message_id, builder) in self.messages.iter().enumerate() {
            builder.build(self.schedule, message_id as u16, app);
        }
    }
}

impl<T: DeserializeOwned + Send + Sync + 'static, C: Component> MessageIdBuilder<C>
    for MessageIdBuilderType<T>
{
    fn build(&self, schedule: Interned<dyn ScheduleLabel>, message_id: u16, app: &mut App) {
        app.insert_resource(MessageId::<C, T> {
            _p: PhantomData,
            message_id,
        });

        app.add_systems(
            schedule,
            (
                deserialize_messages::<C, T>,
                insert_connection_message_type_components::<C, T>,
            ),
        );
    }
}

/// Insert onto an endpoint to specify which stream header to use for message streams
#[derive(Component)]
pub struct EndpointMessagingHeader {
    pub header: u16,
}

/// Contains the message id of a type
#[derive(Resource)]
struct MessageId<C, T> {
    _p: PhantomData<(C, T)>,
    message_id: u16,
}

/// Contains open receive streams that are sending messages
#[derive(Component, Default)]
struct ConnectionMessageStreams {
    /// Contains the stream, and the partially read message
    streams: Vec<(BevyStreamId, ReadMessageState)>,
}

enum ReadMessageState {
    ReadingHeader(Vec<u8>),
    ReadingMessage {
        message_id: u16,
        message_length: u16,
        buffer: Vec<u8>,
    },
}

/// contains received serialized messages for a connection
#[derive(Component, Default)]
struct ReceivedSerializedMessages {
    buffers: Vec<VecDeque<Box<[u8]>>>,
}

/// contains received and deserialized messages for a connection
///
/// query and poll this component to receive messages
#[derive(Component)]
pub struct ReceivedMessages<T> {
    messages: VecDeque<T>,
}

impl ReadMessageState {
    fn new() -> Self {
        ReadMessageState::ReadingHeader(Vec::new())
    }

    fn read(&mut self, stream: &mut BevyRecvStream) -> Option<(u16, Box<[u8]>)> {
        loop {
            match self {
                ReadMessageState::ReadingHeader(buffer) => {
                    let to_read = 4 - buffer.len();

                    if to_read == 0 {
                        let message_id = u16::from_be_bytes(buffer[0..2].try_into().unwrap());
                        let message_length = u16::from_be_bytes(buffer[2..4].try_into().unwrap());

                        *self = ReadMessageState::ReadingMessage {
                            message_id,
                            message_length,
                            buffer: Vec::new(),
                        };

                        continue;
                    }

                    match stream.recv(to_read) {
                        Ok(bytes) => buffer.extend(bytes.as_ref()),
                        Err(err) => {
                            if err.is_fatal() {
                                panic!("fatal error reading message header");
                            }

                            break None;
                        }
                    }
                }
                ReadMessageState::ReadingMessage {
                    message_id,
                    message_length,
                    buffer,
                } => {
                    let to_read = *message_length as usize - buffer.len();

                    if to_read == 0 {
                        let message_id = *message_id;
                        let buffer = std::mem::replace(buffer, Vec::default()).into_boxed_slice();

                        *self = ReadMessageState::new();

                        break Some((message_id, buffer));
                    }

                    match stream.recv(to_read) {
                        Ok(data) => buffer.extend(data.as_ref()),
                        Err(err) => {
                            if err.is_fatal() {
                                panic!("fatal error reading message body");
                            }

                            break None;
                        }
                    }
                }
            }
        }
    }
}

impl ReceivedSerializedMessages {
    fn push_message(&mut self, message_id: u16, message: Box<[u8]>) {
        let buffer = loop {
            if let Some(buffer) = self.buffers.get_mut(message_id as usize) {
                break buffer;
            } else {
                self.buffers.push(VecDeque::new());
            }
        };

        buffer.push_back(message);
    }

    fn poll_message_received(&mut self, message_id: u16) -> Option<Box<[u8]>> {
        let buffer = self.buffers.get_mut(message_id as usize)?;
        buffer.pop_front()
    }
}

impl<T> ReceivedMessages<T> {
    pub fn new() -> Self {
        ReceivedMessages {
            messages: VecDeque::new(),
        }
    }

    /// drains the internal queue
    pub fn drain(&mut self) -> impl Iterator<Item = T> + '_ {
        self.messages.drain(..)
    }

    /// pops one message of the front of the queue
    pub fn pop(&mut self) -> Option<T> {
        self.messages.pop_front()
    }
}

fn insert_connection_components<C: Component>(
    mut commands: Commands,
    mut connected_r: EventReader<Connected>,
    endpoint_q: Query<(), With<C>>,
) {
    for &Connected {
        endpoint_entity,
        connection_entity,
    } in connected_r.read()
    {
        if !endpoint_q.contains(endpoint_entity) {
            continue;
        }

        commands.entity(connection_entity).insert((
            ConnectionMessageStreams::default(),
            ReceivedSerializedMessages::default(),
        ));
    }
}

fn insert_connection_message_type_components<C: Component, T: Send + Sync + 'static>(
    mut commands: Commands,
    mut connected_r: EventReader<Connected>,
    endpoint_q: Query<(), With<C>>,
) {
    for &Connected {
        endpoint_entity,
        connection_entity,
    } in connected_r.read()
    {
        if !endpoint_q.contains(endpoint_entity) {
            continue;
        }

        commands
            .entity(connection_entity)
            .insert(ReceivedMessages::<T>::new());
    }
}

fn deserialize_messages<C: Send + Sync + 'static, T: DeserializeOwned + Send + Sync + 'static>(
    message_id: Res<MessageId<C, T>>,
    mut connection_q: Query<(&mut ReceivedSerializedMessages, &mut ReceivedMessages<T>)>,
) {
    for (mut serialized_messages, mut deserialized_messages) in connection_q.iter_mut() {
        while let Some(bytes) = serialized_messages.poll_message_received(message_id.message_id) {
            let Ok(deserialized) = bincode::deserialize(bytes.as_ref()) else {
                warn!(
                    "failed to deserialize a \"{}\" message",
                    std::any::type_name::<T>()
                );
                continue;
            };

            deserialized_messages.messages.push_back(deserialized);
        }
    }
}

fn receive_message_streams<C: Component>(
    mut stream_event_r: EventReader<HeaderStreamEvent>,
    mut connection_q: Query<&mut ConnectionMessageStreams>,
    endpoint_q: Query<&EndpointMessagingHeader, With<C>>,
) {
    for HeaderStreamEvent {
        endpoint_entity,
        connection_entity,
        stream_id,
        event_type,
        ..
    } in stream_event_r.read()
    {
        let &HeaderStreamEventType::NewRecvStream(incoming_header) = event_type else {
            continue;
        };

        let Ok(&EndpointMessagingHeader { header }) = endpoint_q.get(*endpoint_entity) else {
            continue;
        };

        if incoming_header != header {
            continue;
        }

        let Ok(mut streams) = connection_q.get_mut(*connection_entity) else {
            continue;
        };

        streams
            .streams
            .push((stream_id.clone(), ReadMessageState::new()));
    }
}

fn read_message_streams<C: Component>(
    mut connections: Connections,
    mut connection_q: Query<(
        Entity,
        &mut ConnectionMessageStreams,
        &mut ReceivedSerializedMessages,
        &Parent,
    )>,
    endpoint_q: Query<(), With<C>>,
) {
    for (connection_entity, mut streams, mut serialized_messages, connection_parent) in
        connection_q.iter_mut()
    {
        if !endpoint_q.contains(connection_parent.get()) {
            continue;
        }

        let Some(mut endpoint) = connections.connection_endpoint_mut(connection_entity) else {
            error!(
                "failed to get endpoint for connection {:?} when reading messages",
                connection_entity
            );
            continue;
        };

        let Some(mut connection) = endpoint.connection_mut(connection_entity) else {
            error!(
                "failed to get connection {:?} from it's endpoint",
                connection_entity
            );
            continue;
        };

        streams.streams.retain_mut(|(stream_id, read_state)| {
            let Some(mut stream) = connection
                .recv_stream(stream_id.clone())
                .expect("shouldn't mismatch stream id")
            else {
                return false;
            };

            while let Some((message_id, message)) = read_state.read(&mut stream) {
                serialized_messages.push_message(message_id, message);
            }

            true
        });
    }
}
