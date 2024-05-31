
use std::{hint::black_box, str::FromStr};

use bevy::{ecs::schedule::ScheduleLabel, prelude::*, utils::{intern::Interned, HashMap}};
use quinn_proto::VarInt;
use url::Url;
use web_transport_proto::{ConnectRequest, ConnectResponse, Frame, Settings};

use crate::{bevy::*, prelude::*, EndpointEventHandler, StreamReader};


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
    // Connections that have not completed WebTransport negotiations.
    uninitialized_connections: std::collections::HashMap<ConnectionId, UninitializedConnection>,
}

/// the state for a web transport client that hasn't been fully initialized
enum  UninitializedConnection {
    Client {
        /// The current state of this initialization.
        state: HandshakeSendStream,
        // The buffer containing incomplete handshake data for the current phase.
        buffer: Vec<u8>
    },
    Server {
        /// The current state of this initialization.
        state: HandshakeReceiveStream,
        // The buffer containing incomplete handshake data for the current phase.
        buffer: Vec<u8>
    },
    Failed,
}

enum HandshakeReceiveStream {
    /// waiting for the peer to open a uni >> us to send settings.
    SettingsWait,
    /// waiting for the peer to send the settings on the stream.
    ReceivingSettings(StreamId),
    /// opened a uni >> peer to send a setting respond.
    SendSettingsResponse(StreamId),
    /// Waiting for the bidirectional stream with 'CONNECT' request.
    ConnectWait,
    ReceivingConnect(StreamId),
    SendConnectResponse(StreamId)
}

enum HandshakeSendStream {
    // We are populating the buffer with the settings to be sent.
    GenerateSettings,
    /// We are sending our settings to the peer (server).
    SendSettings(StreamId),
    // We are waiting for the peer to open uni >> us with the response to our settings.
    WaitSettingStream,
    /// we have the unidirectional stream expected and are reading the data.
    ReceiveSettings(StreamId),
    // We opened a bi stream and sent our connect request.
    SendConnect(StreamId),
    // We are now waiting to receive a connect response in the bidirectional channel from the server.
    ReceivingConnectResponse(StreamId),
}


enum ReadResult {
    //We are waiting for more data.
    Wait,
    //The settings read matches our expectations so we can send a response.
    Success,
    //The settings were not parsed correctly.
    Fail
}

fn read_settings(mut reader: StreamReader, buffer: &mut Vec<u8>) -> ReadResult {
    for chunk in reader.read() {
        buffer.extend_from_slice(&chunk);
        let mut limit = std::io::Cursor::new(&buffer);
        match Settings::decode(&mut limit) {
            Ok(req) => {
                debug!("Received SETTINGS headers ({:?}) from WebTransport peer.", req);
                if req.supports_webtransport() != 1 {
                    warn!("Peer settings indicate that WebTransport is not supported!");
                    return ReadResult::Fail;
                }

                return ReadResult::Success;
            },
            Err(web_transport_proto::SettingsError::UnexpectedEnd) => {
                trace!("Partially read SETTINGs request. Buffering...");
                continue;
            },
            Err(e) => {
                error!("Error parsing WebTransport SETTINGs header: {}", e);
                return ReadResult::Fail;
            }
        }
    }

    ReadResult::Fail
}

fn read_connect(mut reader: StreamReader, buffer: &mut Vec<u8>) -> ReadResult {
    for chunk in reader.read() {
        buffer.extend_from_slice(&chunk);
        let mut limit = std::io::Cursor::new(&buffer);
        match ConnectRequest::decode(&mut limit) {
            Ok(req) => {
                debug!("Received CONNECT headers ({:?}) from WebTransport peer.", req);
                return ReadResult::Success;
            },
            Err(web_transport_proto::ConnectError::UnexpectedEnd) => {
                trace!("Partially read CONNECT request. Buffering...");
                continue;
            },
            Err(e) => {
                error!("Error parsing WebTransport CONNECT header: {}", e);
                return ReadResult::Fail;
            }
        }
    }

    ReadResult::Fail
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

        web_transport.uninitialized_connections.retain(|&connection_id, uninitialized_connection| {
            let connection = endpoint.get_connection_mut(connection_id).expect("events have been processed, connection should exist");

            match uninitialized_connection {
                UninitializedConnection::Client { state, buffer } => {
                    match state {
                        HandshakeSendStream::GenerateSettings => {
                            let mut settings = Settings::default();
                            settings.enable_webtransport(1);
                            settings.encode(buffer);

                            let Some(stream) = connection.open_uni() else {
                                warn!("Unable to open unidirectional stream to negotiate settings...");
                                *uninitialized_connection = UninitializedConnection::Failed;
                                return true;
                            };

                            *state = HandshakeSendStream::SendSettings(stream);
                        }
                        HandshakeSendStream::SendSettings(stream) => {
                            match connection.write(*stream, buffer) {
                                Ok(written) => {
                                    trace!("Wrote {written} of {} bytes", buffer.len());
                                    // If we finished writing, we now can wait for the response.
                                    if written == buffer.len() {
                                        trace!("Sent full settings. Waiting for reply...");
                                        buffer.clear();
                                        *state = HandshakeSendStream::WaitSettingStream;
                                    } else {
                                        // Otherwise, keep the remaining bytes in the buffer
                                        trace!("Partial write of settings to server. Draining bytes to complete sending next tick.");
                                        buffer.drain(..written);
                                    }
                                },
                                Err(e) => {
                                    // Log the error
                                    warn!("Error writing settings response: {:?}", e);
                                    *uninitialized_connection = UninitializedConnection::Failed;
                                },
                            }
                        },
                        HandshakeSendStream::WaitSettingStream => (),
                        HandshakeSendStream::ReceiveSettings(stream) => {
                            let reader = connection.reader(*stream);
                            match read_settings(reader, buffer) {
                                ReadResult::Wait => {
                                    trace!("Blocking to receive settings from server.");
                                    return true;
                                },
                                ReadResult::Success => {
                                    let Some(stream) = connection.open_bi() else {
                                        warn!("Received acceptable settings from server but was unable to open bidirectional stream for CONNECT");
                                        *uninitialized_connection = UninitializedConnection::Failed;
                                        return true;
                                    };

                                    trace!("Opened stream after valid settings response. Sending CONNECT request.");
                                    buffer.clear();
                                    let connect_req = ConnectRequest { url: Url::from_str("Hello").unwrap() };
                                    connect_req.encode(buffer);
                                    *state = HandshakeSendStream::SendConnect(stream);
                                },
                                ReadResult::Fail => {
                                    info!("Failed to read WebTransport settings from server.");
                                    *uninitialized_connection = UninitializedConnection::Failed;
                                    return true;
                                }
                            }
                        },
                        HandshakeSendStream::SendConnect(stream) => {
                            match connection.write(*stream, buffer) {
                                Ok(written) => {
                                    trace!("Wrote {written} of {} bytes", buffer.len());
                                    // If we finished writing, we now can wait for the response.
                                    if written == buffer.len() {
                                        trace!("Sent full settings. Waiting for reply...");
                                        buffer.clear();
                                        *state = HandshakeSendStream::ReceivingConnectResponse(*stream);
                                    } else {
                                        // Otherwise, keep the remaining bytes in the buffer
                                        trace!("Partial write of settings to server. Draining bytes to complete sending next tick.");
                                        buffer.drain(..written);
                                    }
                                },
                                Err(e) => {
                                    // Log the error
                                    warn!("Error writing settings response: {:?}", e);
                                    *uninitialized_connection = UninitializedConnection::Failed;
                                },
                            }
                        },
                        HandshakeSendStream::ReceivingConnectResponse(stream) => {
                            let reader = connection.reader(*stream);
                            match read_settings(reader, buffer) {
                                ReadResult::Wait => {
                                    trace!("Blocking to receive connect from server.");
                                    return true;
                                },
                                ReadResult::Success => {
                                    info!("Fully established WebTransport connection with the server.");
                                    return false;
                                },
                                ReadResult::Fail => {
                                    info!("Failed to read WebTransport settings from server.");
                                    *uninitialized_connection = UninitializedConnection::Failed;
                                    return true;
                                }
                            }
                        },
                    }
                },
                UninitializedConnection::Server { state, buffer } => {
                    match state {
                        //Waiting for client to open uni stream.
                        HandshakeReceiveStream::SettingsWait => (),
                        HandshakeReceiveStream::ReceivingSettings(stream_id) => {
                            let reader = connection.reader(*stream_id);
                            match read_settings(reader, buffer) {
                                ReadResult::Wait => {
                                    trace!("Receiving settings from the opened stream!");
                                    return true;
                                }
                                ReadResult::Success => {
                                    let Some(stream) = connection.open_uni() else {
                                        warn!("Received WebTransport settings but was unable to open unidirectional stream for response.");
                                        *uninitialized_connection = UninitializedConnection::Failed;
                                        return true;
                                    };

                                    trace!("Read connect request, queueing response to be sent.");

                                    //Create the settings that will then be sent over multiple potential polls.
                                    let mut settings_resp = Settings::default();
                                    settings_resp.enable_webtransport(1);
                                    buffer.clear();
                                    trace!("Buffer len before writing settings: {}", buffer.len());
                                    settings_resp.encode(buffer);
                                    trace!("Buffer len after writing settings: {}", buffer.len());
                                    *state = HandshakeReceiveStream::SendSettingsResponse(stream);
                                    return true;
                                },
                                ReadResult::Fail => {
                                    info!("Failed to read WebTransport settings from peer.");
                                    *uninitialized_connection = UninitializedConnection::Failed;
                                    return true;
                                }
                            }

                        },
                        HandshakeReceiveStream::SendSettingsResponse(stream_id) => {
                            match connection.write(*stream_id, buffer) {
                                Ok(written) => {
                                    trace!("Wrote {written} of {} bytes", buffer.len());
                                    // If we finished writing, we need to wait for the bidirectional CONNECT stream.
                                    if written == buffer.len() {
                                        debug!("Sent full settings response. Waiting for bidir stream from client.");
                                        buffer.clear();
                                        *state = HandshakeReceiveStream::ConnectWait;
                                    } else {
                                        // Otherwise, keep the remaining bytes in the buffer
                                        trace!("Partial write. Draining bytes to complete sending next tick.");
                                        buffer.drain(..written);
                                    }
                                },
                                Err(e) => {
                                    // Log the error
                                    warn!("Error writing settings response: {:?}", e);
                                    *uninitialized_connection = UninitializedConnection::Failed;
                                },
                            }
                        },
                        //Waiting for client to open bidirectional stream.
                        HandshakeReceiveStream::ConnectWait => (),
                        HandshakeReceiveStream::ReceivingConnect(stream_id) => {
                            let reader = connection.reader(*stream_id);
                            match read_connect(reader, buffer) {
                                ReadResult::Wait => return true,
                                ReadResult::Success => {
                                    trace!("WebTransport connect was valid. Queueing final response.");

                                    buffer.clear();
                                    //Create the response for it to be sent on the next tick.
                                    let connect_resp = ConnectResponse { status: default() };
                                    connect_resp.encode(buffer);
                                    *state = HandshakeReceiveStream::SendConnectResponse(*stream_id);
                                },
                                ReadResult::Fail => {
                                    info!("Failed to read WebTransport settings from peer.");
                                    *uninitialized_connection = UninitializedConnection::Failed;
                                    return true;
                                }
                            }
                        },
                        HandshakeReceiveStream::SendConnectResponse(stream) => {
                            match connection.write(*stream, buffer) {
                                Ok(written) => {
                                    // If we finished writing, we need to wait for the bidirectional CONNECT stream.
                                    if written == buffer.len() {
                                        trace!("Wrote {written} bytes to send the connect response");
                                        info!("Successfully negotiated WebTransport connection with peer.");
                                        return false;
                                    } else {
                                        // Otherwise, keep the remaining bytes in the buffer
                                        buffer.drain(..written);
                                    }
                                },
                                Err(e) => {
                                    // Log the error
                                    warn!("Error writing settings response: {:?}", e);
                                    *uninitialized_connection = UninitializedConnection::Failed;
                                },
                            }
                        },
                    }
                },
                UninitializedConnection::Failed => todo!(),
            }

            true
        });
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
                debug!("Initializing WebTransport client state. Sending settings.");
                UninitializedConnection::Client {
                    state: HandshakeSendStream::GenerateSettings,
                    buffer: Vec::with_capacity(u16::MAX as usize)
                }
            },
            quinn_proto::Side::Server => {
                trace!("Initializing WebTransport server state. Waiting for stream...");
                UninitializedConnection::Server {
                    state: HandshakeReceiveStream::SettingsWait,
                    buffer: Vec::with_capacity(u16::MAX as usize)
                }
            },
        };

        self.web_transport.uninitialized_connections.insert(connection.connection_id(), uninitialized_connection);
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
        debug!("WebTransport handler received a new stream");
        if let Some(uninitialized_connection) = self.web_transport.uninitialized_connections.get_mut(&connection.connection_id()) {
            trace!("Stream was associated with a pending connection.");
            match uninitialized_connection {
                UninitializedConnection::Client { state, buffer: _ } => {
                    match state {
                        HandshakeSendStream::WaitSettingStream => {
                            if bi_directional {
                                info!("WebTransport peer opened a bidirectional stream when a unidirectional one was expected!");
                                *uninitialized_connection = UninitializedConnection::Failed;
                                self.bevy.new_stream(connection, stream_id, bi_directional);
                                return;
                            }
                            *state = HandshakeSendStream::ReceiveSettings(stream_id)
                        },
                        _ => ()
                    }
                },
                UninitializedConnection::Server { state, buffer: _ } => {
                    match state {
                        HandshakeReceiveStream::SettingsWait => {
                            if bi_directional {
                                info!("WebTransport peer opened a bidirectional stream when a unidirectional one was expected!");
                                *uninitialized_connection = UninitializedConnection::Failed;
                                self.bevy.new_stream(connection, stream_id, bi_directional);
                                return;
                            }

                            *state = HandshakeReceiveStream::ReceivingSettings(stream_id);
                        },
                        HandshakeReceiveStream::ConnectWait => {
                            if !bi_directional {
                                info!("WebTransport peer opened a unidirectional stream when a bidirectional one was expected!");
                                *uninitialized_connection = UninitializedConnection::Failed;
                                self.bevy.new_stream(connection, stream_id, bi_directional);
                                return;
                            }

                            *state = HandshakeReceiveStream::ReceivingConnect(stream_id);
                        },
                        //If a stream is opened during other states it can be passed through to the application.
                        //This allows streams to be opportunistically opened during WebTransport negotiation.
                        _ => ()
                    }
                },
                UninitializedConnection::Failed => {
                    connection.disconnect(quinn_proto::VarInt::from_u32(55), "WebTransport is enabled but peer did not behave as expected".as_bytes().into());
                    info!("Peer did not follow expected WebTransport protocol and was forcibly disconnected!");
                    self.web_transport.uninitialized_connections.remove(&connection.connection_id());
                },
            }

            return;
        }

        let mut reader = connection.reader(stream_id);
        if let Some(data) = reader.read().next() {
            let header = Frame::decode(&mut data.as_ref());
            let Ok(frame) = header else {
                warn!("WebTransport is enabled but stream did not begin with a valid frame!");
                return;
            };

            if frame.0 != web_transport_proto::VarInt::from_u32(0x41) {
                warn!("Stream was expected to begin with 0x41 (WebTransport) but began with {:?} instead.", frame);
            }
        }

        self.bevy.new_stream(connection, stream_id, bi_directional);
    }

    fn receive_stream_closed(&mut self, connection: &mut crate::ConnectionState, stream_id: quinn_proto::StreamId, reset_error: Option<quinn_proto::VarInt>) {
        // dont fire closed stream events for the web transport streams

        // if let Some(uninitialized_connection) = self.web_transport.uninitialized_connections.get_mut(&connection.connection_id()) {
        //     match uninitialized_connection {
        //         UninitializedConnection::Client { .. } => (),

        //         UninitializedConnection::Server { state, receive_buffer } => {
        //             if let HandshakeReceiveStream::Receiving { stream_id: handshake_stream_id, handshake_data } = &handshake_stream {
        //                 if *handshake_stream_id == stream_id {

        //                     if let Some(reset_error) = reset_error {
        //                         // fail the web transport connection
        //                         connection.disconnect(0u32.into(), [].into());
        //                         *uninitialized_connection = UninitializedConnection::Failed;
        //                         return;
        //                     }

        //                     debug!("finished receiving web transport handshake data {:?}", handshake_data);

        //                     *handshake_stream = HandshakeReceiveStream::Received;
        //                 }
        //             }
        //         },

        //         UninitializedConnection::Failed => (),
        //     }

        //     return;
        // }

        self.bevy.receive_stream_closed(connection, stream_id, reset_error);
    }
}
