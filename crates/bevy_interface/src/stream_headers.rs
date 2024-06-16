use std::num::NonZeroI8;

use bevy::{ecs::schedule::ScheduleLabel, prelude::*, utils::intern::Interned};
use transport_interface::StreamEventType;

use crate::{
    connections::{BevyConnectionMut, StreamError},
    prelude::{BevyStreamEvent, BevyStreamId, Connections, Description},
    Connected, MismatchedType,
};

type Header = u16;

/// adds events and update loop for
/// [BevyEndpoint] and [BevyConnection]
pub struct StreamHeaderPlugin {
    schedule: Interned<dyn ScheduleLabel>,
}

impl StreamHeaderPlugin {
    /// creates a new [StreamHeaderPlugin] that updates in a certain schedule
    fn new(schedule: impl ScheduleLabel) -> Self {
        StreamHeaderPlugin {
            schedule: schedule.intern(),
        }
    }
}

impl Default for StreamHeaderPlugin {
    fn default() -> Self {
        StreamHeaderPlugin::new(PreUpdate)
    }
}

impl Plugin for StreamHeaderPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<HeaderStreamEvent>();

        app.add_systems(self.schedule, initialize_clients);
    }
}

#[derive(Event)]
pub struct HeaderStreamEvent {
    pub stream_id: BevyStreamId,
    pub peer_generated: bool,
    pub event_type: HeaderStreamEventType,
}

pub enum HeaderStreamEventType {
    NewSendStream,
    ClosedSendStream,
    NewRecvStream(Header),
    ClosedRecvStream,
}

#[derive(Component)]
pub struct EndpointStreamHeaders;

#[derive(Default, Component)]
pub struct ConnectionStreamHeaders {
    uninitialized_streams: Vec<(BevyStreamId, bool, Vec<u8>)>,
}

fn initialize_clients(
    mut commands: Commands,
    mut connected_r: EventReader<Connected>,
    endpoint_q: Query<(), With<EndpointStreamHeaders>>,
) {
    for &Connected {
        endpoint_entity,
        connection_entity,
    } in connected_r.read()
    {
        if endpoint_q.contains(endpoint_entity) {
            commands
                .entity(connection_entity)
                .insert(ConnectionStreamHeaders::default());
        }
    }
}

fn poll_stream_events(
    mut connections: Connections,
    mut connection_q: Query<(Entity, &mut ConnectionStreamHeaders)>,
    mut event_w: EventWriter<HeaderStreamEvent>,
) {
    for (connection_entity, mut headers) in connection_q.iter_mut() {
        let Some(mut endpoint) = connections.connection_endpoint_mut(connection_entity) else {
            error!(
                "Couldn't query connection {:?}'s endpoint",
                connection_entity
            );
            continue;
        };

        let Some(mut connection) = endpoint.connection_mut(connection_entity) else {
            error!(
                "couldn't query connection {:?} from it's endpoint",
                connection_entity
            );
            continue;
        };

        while let Some(BevyStreamEvent {
            stream_id,
            peer_generated,
            event_type,
        }) = connection.poll_stream_events()
        {
            match event_type {
                StreamEventType::NewRecvStream => {
                    headers
                        .uninitialized_streams
                        .push((stream_id, peer_generated, Vec::new()));
                }
                event_type => {
                    event_w.send(HeaderStreamEvent {
                        stream_id,
                        peer_generated,
                        event_type: match event_type {
                            StreamEventType::NewSendStream => HeaderStreamEventType::NewSendStream,
                            StreamEventType::ClosedSendStream => {
                                HeaderStreamEventType::ClosedSendStream
                            }
                            StreamEventType::NewRecvStream => unreachable!(),
                            StreamEventType::ClosedRecvStream => {
                                HeaderStreamEventType::ClosedRecvStream
                            }
                        },
                    });
                }
            }
        }
    }
}

fn read_headers(
    mut event_w: EventWriter<HeaderStreamEvent>,
    mut connections: Connections,
    mut connection_q: Query<(Entity, &mut ConnectionStreamHeaders)>,
) {
    for (connection_entity, mut headers) in connection_q.iter_mut() {
        if headers.uninitialized_streams.len() == 0 {
            continue;
        }

        let Some(mut endpoint) = connections.connection_endpoint_mut(connection_entity) else {
            error!(
                "Couldn't query connection {:?}'s endpoint",
                connection_entity
            );
            continue;
        };

        let Some(mut connection) = endpoint.connection_mut(connection_entity) else {
            error!(
                "couldn't query connection {:?} from it's endpoint",
                connection_entity
            );
            continue;
        };

        headers
            .uninitialized_streams
            .retain_mut(|(stream_id, peer_generated, buffer)| {
                let Some(mut stream) = connection
                    .recv_stream(stream_id.clone())
                    .expect("Shouldn't mismatch stream id")
                else {
                    warn!("stream was closed prematurly before header could be sent");
                    return false;
                };

                loop {
                    let to_receive = (Header::BITS / 8) as usize - buffer.len();

                    if to_receive == 0 {
                        let header = Header::from_be_bytes(buffer.clone().try_into().unwrap());

                        event_w.send(HeaderStreamEvent {
                            stream_id: stream_id.clone(),
                            peer_generated: *peer_generated,
                            event_type: HeaderStreamEventType::NewRecvStream(header),
                        });

                        return false;
                    }

                    match stream.recv(to_receive) {
                        Err(err) => {
                            if err.is_fatal() {
                                error!("fatal error receiving stream header");
                                return false;
                            }

                            break;
                        },
                        Ok(bytes) => {
                            let mut bytes = bytes.as_ref();

                            if let Some(excess_bytes) = to_receive.checked_sub(bytes.len()) {
                                if excess_bytes > 0 {
                                    error!("received more bytes than needed to construct header, discarding {} bytes", excess_bytes);
                                    bytes = &bytes[..(bytes.len() - excess_bytes)];
                                }
                            }

                            buffer.extend(bytes);
                        }
                    }
                }

                true
            });
    }
}

pub struct UninitializedStream {
    stream_id: BevyStreamId,
    header: Vec<u8>,
}

pub enum InitializeStreamError {
    StreamClosedPrematurly,
    MismatchedConnection {
        stream: UninitializedStream,
        connection: MismatchedType,
    },
    FatalSendErr(Box<dyn StreamError>),
}

impl std::fmt::Debug for InitializeStreamError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        todo!()
    }
}

impl UninitializedStream {
    pub fn new(
        connection: &mut BevyConnectionMut,
        description: Description,
        header: Header,
    ) -> Result<Option<Self>, MismatchedType> {
        let stream_id = match connection.open_stream(description) {
            Err(err) => return Err(err),
            Ok(None) => return Ok(None),
            Ok(Some(stream_id)) => stream_id,
        };

        Ok(Some(UninitializedStream {
            stream_id,
            header: header.to_be_bytes().into(),
        }))
    }

    pub fn poll_ready(
        mut self,
        connection: &mut BevyConnectionMut,
    ) -> Result<Result<BevyStreamId, Self>, InitializeStreamError> {
        let mut stream = match connection.send_stream(self.stream_id.clone()) {
            Err(err) => {
                return Err(InitializeStreamError::MismatchedConnection {
                    stream: self,
                    connection: err,
                })
            }
            Ok(None) => return Err(InitializeStreamError::StreamClosedPrematurly),
            Ok(Some(stream)) => stream,
        };

        loop {
            if self.header.len() == 0 {
                return Ok(Ok(self.stream_id));
            }

            match stream.send(&self.header) {
                Err(err) => {
                    if err.is_fatal() {
                        return Err(InitializeStreamError::FatalSendErr(err));
                    }

                    break Ok(Err(self));
                }
                Ok(bytes) => {
                    if bytes == 0 {
                        break Ok(Err(self));
                    }

                    self.header.drain(..bytes);
                }
            }
        }
    }
}
