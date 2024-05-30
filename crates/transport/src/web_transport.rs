
use bevy::{ecs::schedule::ScheduleLabel, prelude::*, utils::{intern::Interned, HashMap}};

use crate::{prelude::*, bevy::*, EndpointEventHandler};


/// adds web transport functionality for the [WebTransportEndpoint] component
///
/// depends on [BevyEndpointPlugin]
pub struct WebTransportEndpointPlugin {
    schedule: Interned<dyn ScheduleLabel>,
}

impl WebTransportEndpointPlugin {
    pub fn new(schedule: impl ScheduleLabel) -> Self {
        WebTransportEndpointPlugin {
            schedule: schedule.intern(),
        }
    }
}

impl Default for WebTransportEndpointPlugin {
    fn default() -> Self {
        WebTransportEndpointPlugin::new(PreUpdate)
    }
}

impl Plugin for WebTransportEndpointPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(self.schedule, update_endpoints);
    }
}



/// when on the same entity as an [EndpointState] it will operate as a web transport endpoint
#[derive(Component, Default)]
pub struct WebTransportEndpoint {
    uninitialized_connections: HashMap<ConnectionId, UninitializedConnection>,
}

/// the state for a web transport client that hasn't been fully initialized
enum  UninitializedConnection {
    Client {
        /// the send queue for the handshake stream
        handshake_buffer: Option<(StreamId, Vec<u8>)>,
    },
    Server {
        handshake_stream: HandshakeReceiveStream,
    },
    Failed,
}

enum HandshakeReceiveStream {
    /// waiting for the peer to open a stream
    NotOpened,
    /// waiting for the peer to finish sending data on the stream
    Receiving {
        stream_id: StreamId,
        handshake_data: Vec<u8>,
    },
    /// finished receiving handshake data
    Received,
}



fn update_endpoints(
    mut events: BevyEndpointEvents,
    mut endpoint_q: Query<(Entity, &mut EndpointState, &mut WebTransportEndpoint)>,
    mut buffers: Local<EndpointBuffers>,
) {
    for (endpoint_entity, mut endpoint, mut web_transport) in endpoint_q.iter_mut() {
        endpoint.update(&mut buffers, &mut WebTransportEventHandler {
            bevy: BevyEndpointEventHandler {
                events: &mut events,
                endpoint_entity,
            },
            web_transport: &mut web_transport,
        });

        for (&connection_id, uninitialized_connection) in web_transport.uninitialized_connections.iter_mut() {
            let connection = endpoint.get_connection_mut(connection_id).expect("events have been processed, connection should exist");

            match uninitialized_connection {

                UninitializedConnection::Client { handshake_buffer } => {
                    if let Some((stream_id, buffer)) = handshake_buffer {
                        loop {
                            match connection.write(*stream_id, &buffer) {
                                Ok(0) | Err(WriteError::Blocked) => break,

                                Ok(bytes_written) => {
                                    buffer.drain(..bytes_written);

                                    if buffer.len() == 0 {
                                        connection.finish(*stream_id);
                                        *handshake_buffer = None;
                                        break;
                                    }
                                },

                                Err(WriteError::StreamDoesntExist) => {
                                    // peer must have reset the stream, disconnect
                                    connection.disconnect(0u32.into(), [].into());
                                    *uninitialized_connection = UninitializedConnection::Failed;
                                    break;
                                },
                            }
                        }
                    }
                },

                UninitializedConnection::Server { handshake_stream } => {
                    if let HandshakeReceiveStream::Receiving { stream_id, handshake_data } = handshake_stream {
                        for data in connection.reader(*stream_id).read() {
                            handshake_data.extend(data.as_ref());
                        }
                    }
                },

                UninitializedConnection::Failed => (),
            }
        }
    }
}


struct WebTransportEventHandler<'a, 'w> {
    bevy: BevyEndpointEventHandler<'a, 'w>,
    web_transport: &'a mut WebTransportEndpoint,
}

impl<'a, 'w> EndpointEventHandler for WebTransportEventHandler<'a, 'w> {
    fn new_connection(&mut self, connection: &mut crate::ConnectionState) {
        // dont fire connected event until web transport is initialized

        let uninitialized_connection = match connection.side() {
            quinn_proto::Side::Client => {
                debug!("Sending WebTransport SETTINGs frame");

                let mut settings = web_transport_proto::Settings::default();
                settings.enable_webtransport(1);

                let mut buffer = Vec::new();
                settings.encode(&mut buffer);

                let handshake_stream = connection.open_uni().unwrap();

                UninitializedConnection::Client {
                    handshake_buffer: Some((handshake_stream, buffer)),
                }
            },
            quinn_proto::Side::Server => {
                UninitializedConnection::Server {
                    handshake_stream: HandshakeReceiveStream::NotOpened,
                }
            },
        };

        self.web_transport.uninitialized_connections.insert(connection.connection_id(), uninitialized_connection);

        todo!()
    }

    fn disconnected(&mut self, connection: &mut crate::ConnectionState) {
        // only fire disconnect event if the client had finished establishing a web transport connection

        if self.web_transport.uninitialized_connections.remove(&connection.connection_id()).is_some() {
            return;
        }

        self.bevy.disconnected(connection);
    }

    fn new_stream(&mut self, connection: &mut crate::ConnectionState, stream_id: quinn_proto::StreamId, bi_directional: bool) {
        // catch the streams needed to initialize web transport, otherwise fire new stream events

        if let Some(uninitialized_connection) = self.web_transport.uninitialized_connections.get_mut(&connection.connection_id()) {
            match uninitialized_connection {
                UninitializedConnection::Client { .. } => (),

                UninitializedConnection::Server { handshake_stream } => {
                    if let HandshakeReceiveStream::NotOpened = handshake_stream {

                        *handshake_stream = HandshakeReceiveStream::Receiving {
                            stream_id,
                            handshake_data: Vec::new(),
                        };
                    }
                },

                UninitializedConnection::Failed => (),
            }

            return;
        }

        self.bevy.new_stream(connection, stream_id, bi_directional);
    }

    fn receive_stream_closed(&mut self, connection: &mut crate::ConnectionState, stream_id: quinn_proto::StreamId, reset_error: Option<quinn_proto::VarInt>) {
        // dont fire closed stream events for the web transport streams

        if let Some(uninitialized_connection) = self.web_transport.uninitialized_connections.get_mut(&connection.connection_id()) {
            match uninitialized_connection {
                UninitializedConnection::Client { .. } => (),

                UninitializedConnection::Server { handshake_stream } => {
                    if let HandshakeReceiveStream::Receiving { stream_id: handshake_stream_id, handshake_data } = &handshake_stream {
                        if *handshake_stream_id == stream_id {

                            if let Some(reset_error) = reset_error {
                                // fail the web transport connection
                                connection.disconnect(0u32.into(), [].into());
                                *uninitialized_connection = UninitializedConnection::Failed;
                                return;
                            }

                            debug!("finished receiving web transport handshake data {:?}", handshake_data);

                            *handshake_stream = HandshakeReceiveStream::Received;
                        }
                    }
                },

                UninitializedConnection::Failed => (),
            }

            return;
        }

        self.bevy.receive_stream_closed(connection, stream_id, reset_error);
    }
}
