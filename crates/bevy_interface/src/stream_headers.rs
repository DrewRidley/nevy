use bevy::{ecs::schedule::ScheduleLabel, prelude::*, utils::intern::Interned};
use transport_interface::StreamEventType;

use crate::{
    connections::{BevyConnectionMut, StreamError},
    prelude::{BevyStreamEvent, BevyStreamId, Connections, Description},
    Connected, MismatchedType,
};

type Header = u16;

/// On any endpoint with [EndpointStreamHeaders],
/// will poll stream events and re-emit [HeaderStreamEvent]s
///
/// will read the first few bytes and determine a header for each new recv stream before
/// firing an event with it's id
///
/// use [HeaderStreamId] to properly send those headers on all streams destined for these endpoints
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

        app.add_systems(
            self.schedule,
            (initialize_clients, poll_stream_events, read_headers),
        );
    }
}

#[derive(Event)]
pub struct HeaderStreamEvent {
    pub endpoint_entity: Entity,
    pub connection_entity: Entity,
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

/// insert on all endpoints to enable stream header functionality
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
    mut connection_q: Query<(Entity, &mut ConnectionStreamHeaders, &Parent)>,
    mut event_w: EventWriter<HeaderStreamEvent>,
) {
    for (connection_entity, mut headers, connection_parent) in connection_q.iter_mut() {
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
                        endpoint_entity: connection_parent.get(),
                        connection_entity,
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
    mut connection_q: Query<(Entity, &mut ConnectionStreamHeaders, &Parent)>,
) {
    for (connection_entity, mut headers, connection_parent) in connection_q.iter_mut() {
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
                            endpoint_entity: connection_parent.get(),
                            connection_entity,
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

/// wraps a [BevyStreamId] and will not return
/// that stream id until a header is successfully sent
///
/// this ensures that the application can't use that stream id
/// unless the header has been sent first
pub struct HeaderStreamId {
    stream_id: BevyStreamId,
    header: Vec<u8>,
}

#[derive(Debug)]
pub enum InitializeHeaderStreamError {
    StreamClosedPrematurly,
    MismatchedConnection { connection: MismatchedType },
    FatalSendErr(Box<dyn StreamError>),
}

impl HeaderStreamId {
    /// tries to create a new stream on a connection
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

        Ok(Some(HeaderStreamId {
            stream_id,
            header: header.to_be_bytes().into(),
        }))
    }

    /// Attempts to write the header, if successful will return the wrapped [BevyStreamId].
    ///
    /// This method can be called repeatedly to get the stream id even after completion,
    /// but after the stream id has been successfuly returned once the [HeaderStreamId] can be dropped and the
    /// [BevyStreamId] can be used normally from that point on.
    pub fn poll_ready(
        &mut self,
        connection: &mut BevyConnectionMut,
    ) -> Result<Option<BevyStreamId>, InitializeHeaderStreamError> {
        let mut stream = match connection.send_stream(self.stream_id.clone()) {
            Err(err) => {
                return Err(InitializeHeaderStreamError::MismatchedConnection { connection: err })
            }
            Ok(None) => return Err(InitializeHeaderStreamError::StreamClosedPrematurly),
            Ok(Some(stream)) => stream,
        };

        loop {
            if self.header.len() == 0 {
                return Ok(Some(self.stream_id.clone()));
            }

            match stream.send(&self.header) {
                Err(err) => {
                    if err.is_fatal() {
                        return Err(InitializeHeaderStreamError::FatalSendErr(err));
                    }

                    break Ok(None);
                }
                Ok(bytes) => {
                    if bytes == 0 {
                        break Ok(None);
                    }

                    self.header.drain(..bytes);
                }
            }
        }
    }
}
