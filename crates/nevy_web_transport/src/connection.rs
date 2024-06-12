use std::collections::{HashMap, VecDeque};

use log::{debug, error, trace, warn};
use nevy_quic::prelude::*;
use quinn_proto::Dir;
use transport_interface::*;
use web_transport_proto::{ConnectRequest, ConnectResponse, Settings};

use crate::{
    endpoint::WebTransportEndpoint,
    streams::{WebTransportRecvStream, WebTransportSendStream, WebTransportStreamId},
};

pub struct WebTransportConnection {
    state: ConnectionState,
    pub(crate) send_streams: HashMap<WebTransportStreamId, WebTransportSendStream>,
    pub(crate) recv_streams: HashMap<WebTransportStreamId, WebTransportRecvStream>,
    pub(crate) stream_events: VecDeque<StreamEvent<WebTransportStreamId>>,
}

#[derive(Debug)]
pub(crate) enum ConnectionState {
    /// State exists but the quic connection is unestablished.
    Unconnected,

    // Client states.
    ClientSendSettings(QuinnStreamId, Vec<u8>),
    ClientWaitSettingsResponse,
    ClientReceiveSettingsResponse(QuinnStreamId, Vec<u8>),
    ClientSendConnect(QuinnStreamId, Vec<u8>),
    ClientReceiveConnectResponse(QuinnStreamId, Vec<u8>),

    // Server states.
    ServerWaitSettingStream,
    ServerReadSettings(QuinnStreamId, Vec<u8>),
    ServerSendSettingsResponse(QuinnStreamId, Vec<u8>),
    ServerWaitConnectStream,
    ServerReadConnectRequest(QuinnStreamId, Vec<u8>),
    ServerSendConnectResponse(QuinnStreamId, Vec<u8>),

    // Server client symmetrical state
    Connected,
    Failed,
}

pub struct WebTransportConnectionMut<'c> {
    pub(crate) quinn: &'c mut QuinnConnection,
    pub(crate) web_transport: &'c mut WebTransportConnection,
    pub(crate) connection_id: QuinnConnectionId,
}

pub struct WebTransportConnectionRef<'c> {
    pub(crate) quinn: &'c QuinnConnection,
}

impl WebTransportConnection {
    pub(crate) fn new() -> Self {
        WebTransportConnection {
            state: ConnectionState::Unconnected,
            send_streams: HashMap::new(),
            recv_streams: HashMap::new(),
            stream_events: VecDeque::new(),
        }
    }

    pub fn is_connected(&self) -> bool {
        match self.state {
            ConnectionState::Connected => true,
            _ => false,
        }
    }
}

impl<'c> WebTransportConnectionMut<'c> {
    /// fired when quinn inidicates a successful quic connection
    pub(crate) fn connected(&mut self) {
        match self.quinn.side() {
            quinn_proto::Side::Client => {
                let mut buffer = Vec::with_capacity(512);

                let stream_id = self.quinn.open_stream(quinn_proto::Dir::Uni).unwrap();

                let mut settings = web_transport_proto::Settings::default();
                settings.enable_webtransport(1);
                settings.encode(&mut buffer);

                self.web_transport.state = ConnectionState::ClientSendSettings(stream_id, buffer)
            }
            quinn_proto::Side::Server => {
                self.web_transport.state = ConnectionState::ServerWaitSettingStream;
            }
        }
    }

    /// transitions to the failed state and disconnects the quic connection
    fn fail(&mut self) {
        trace!("{} | -> Failed", self.get_stats());
        self.web_transport.state = ConnectionState::Failed;
        self.quinn.disconnect();
    }

    /// transitions to the connected state and fires the connected event
    fn success(&mut self, handler: &mut impl EndpointEventHandler<WebTransportEndpoint>) {
        trace!("{} | -> Connected", self.get_stats());
        self.web_transport.state = ConnectionState::Connected;
        handler.connected(self.connection_id);
    }

    pub(crate) fn update(&mut self, handler: &mut impl EndpointEventHandler<WebTransportEndpoint>) {
        while let Some(StreamEvent {
            stream_id,
            peer_generated,
            event_type,
        }) = self.quinn.poll_stream_events()
        {
            match event_type {
                StreamEventType::NewSendStream => {
                    if let (ConnectionState::ServerWaitConnectStream, true) =
                        (&self.web_transport.state, peer_generated)
                    {
                        trace!("{} | -> ServerReadConnectRequest", self.get_stats());
                        self.web_transport.state =
                            ConnectionState::ServerReadConnectRequest(stream_id, Vec::new());
                        continue;
                    }

                    // queue event

                    if peer_generated {
                        let stream_id = WebTransportStreamId(stream_id);
                        self.web_transport
                            .send_streams
                            .insert(stream_id, WebTransportSendStream { header: None });
                        self.web_transport.stream_events.push_back(StreamEvent {
                            stream_id,
                            peer_generated: true,
                            event_type: StreamEventType::NewSendStream,
                        })
                    }
                }
                StreamEventType::NewRecvStream => {
                    if let (ConnectionState::ServerWaitSettingStream, true) =
                        (&self.web_transport.state, peer_generated)
                    {
                        // Client opened uni stream to send Settings.
                        trace!("{} | -> ServerReadSettings", self.get_stats());
                        self.web_transport.state =
                            ConnectionState::ServerReadSettings(stream_id, Vec::new());
                        continue;
                    }

                    if let (ConnectionState::ClientWaitSettingsResponse, true) =
                        (&self.web_transport.state, peer_generated)
                    {
                        // Server opened uni stream to send settings response.
                        trace!(
                            "{} | -> ClientReceiveSettingsResponse",
                            self.as_ref().get_stats()
                        );
                        self.web_transport.state =
                            ConnectionState::ClientReceiveSettingsResponse(stream_id, Vec::new());
                        continue;
                    }

                    // queue event

                    if peer_generated {
                        let stream_id = WebTransportStreamId(stream_id);
                        self.web_transport.recv_streams.insert(
                            stream_id,
                            WebTransportRecvStream {
                                header: Some(Vec::new()),
                            },
                        );

                        self.web_transport.stream_events.push_back(StreamEvent {
                            stream_id,
                            peer_generated: true,
                            event_type: StreamEventType::NewRecvStream,
                        })
                    }
                }
                StreamEventType::ClosedSendStream => {
                    let stream_id = WebTransportStreamId(stream_id);
                    self.web_transport.send_streams.remove(&stream_id);
                    self.web_transport.stream_events.push_back(StreamEvent {
                        stream_id,
                        peer_generated,
                        event_type: StreamEventType::ClosedSendStream,
                    })
                }
                StreamEventType::ClosedRecvStream => {
                    let stream_id = WebTransportStreamId(stream_id);
                    self.web_transport.recv_streams.remove(&stream_id);
                    self.web_transport.stream_events.push_back(StreamEvent {
                        stream_id,
                        peer_generated,
                        event_type: StreamEventType::ClosedRecvStream,
                    })
                }
            }
        }

        match &mut self.web_transport.state {
            ConnectionState::Unconnected => (),
            ConnectionState::ClientSendSettings(stream_id, buffer) => {
                let mut stream = self.quinn.send_stream(*stream_id).unwrap();
                while let Ok(n) = stream.send(buffer) {
                    if n == 0 {
                        break;
                    }
                    buffer.drain(..n);
                }

                if buffer.is_empty() {
                    trace!("{} | -> ClientWaitSettingsResponse", self.get_stats());
                    self.web_transport.state = ConnectionState::ClientWaitSettingsResponse;
                }
            }
            ConnectionState::ClientWaitSettingsResponse => (),
            ConnectionState::ClientReceiveSettingsResponse(stream_id, buffer) => {
                let stream = self.quinn.recv_stream(*stream_id).unwrap();
                match read_settings(stream, buffer) {
                    ReadResult::Wait => (),
                    ReadResult::Fail => self.fail(),
                    ReadResult::Success => {
                        let Some(stream_id) = self.quinn.open_stream(Dir::Bi) else {
                            self.fail();
                            return;
                        };

                        let mut buffer = std::mem::take(buffer);
                        buffer.clear();

                        let connect_req = ConnectRequest {
                            url: "https://nevy.client".parse().unwrap(),
                        };

                        connect_req.encode(&mut buffer);

                        trace!("{} | -> ClientSendConnect", self.get_stats());
                        self.web_transport.state =
                            ConnectionState::ClientSendConnect(stream_id, buffer);
                    }
                }
            }
            ConnectionState::ClientSendConnect(stream_id, buffer) => {
                let mut stream = self.quinn.send_stream(*stream_id).unwrap();
                while let Ok(n) = stream.send(buffer) {
                    if n == 0 {
                        break;
                    }
                    buffer.drain(..n);
                }

                let stream_id = *stream_id;
                let mut buffer = std::mem::take(buffer);
                buffer.clear();

                if buffer.is_empty() {
                    trace!("{} | ->  ClientReceiveConnectResponse", self.get_stats());
                    self.web_transport.state =
                        ConnectionState::ClientReceiveConnectResponse(stream_id, buffer);
                }
            }
            ConnectionState::ClientReceiveConnectResponse(stream_id, buffer) => {
                let stream = self.quinn.recv_stream(*stream_id).unwrap();
                match read_connect_response(stream, buffer) {
                    ReadResult::Wait => (),
                    ReadResult::Fail => self.fail(),
                    ReadResult::Success => self.success(handler),
                }
            }

            // Server states.
            ConnectionState::ServerWaitSettingStream => (),
            ConnectionState::ServerReadSettings(stream_id, buffer) => {
                let stream = self.quinn.recv_stream(*stream_id).unwrap();
                match read_settings(stream, buffer) {
                    ReadResult::Wait => (),
                    ReadResult::Fail => self.fail(),
                    ReadResult::Success => {
                        let Some(stream_id) = self.quinn.open_stream(Dir::Uni) else {
                            self.fail();
                            return;
                        };

                        let mut buffer = std::mem::take(buffer);
                        buffer.clear();

                        let mut settings = web_transport_proto::Settings::default();
                        settings.enable_webtransport(1);
                        settings.encode(&mut buffer);

                        trace!("-> ServerSendSettingsResponse");
                        self.web_transport.state =
                            ConnectionState::ServerSendSettingsResponse(stream_id, buffer)
                    }
                }
            }
            ConnectionState::ServerSendSettingsResponse(stream_id, buffer) => {
                let mut stream = self.quinn.send_stream(*stream_id).unwrap();
                while let Ok(n) = stream.send(buffer) {
                    if n == 0 {
                        break;
                    }
                    buffer.drain(..n);
                }

                if buffer.is_empty() {
                    trace!("-> ServerWaitConnectStream");
                    self.web_transport.state = ConnectionState::ServerWaitConnectStream;
                }
            }
            ConnectionState::ServerWaitConnectStream => (),
            ConnectionState::ServerReadConnectRequest(stream_id, buffer) => {
                let stream = self.quinn.recv_stream(*stream_id).unwrap();
                match read_connect(stream, buffer) {
                    ReadResult::Wait => (),
                    ReadResult::Fail => self.fail(),
                    ReadResult::Success => {
                        let stream_id = *stream_id;
                        let mut buffer = std::mem::take(buffer);
                        buffer.clear();

                        let connect_res = ConnectResponse {
                            status: Default::default(),
                        };
                        connect_res.encode(&mut buffer);

                        trace!("-> ServerSendConnectResponse");
                        self.web_transport.state =
                            ConnectionState::ServerSendConnectResponse(stream_id, buffer);
                    }
                }
            }
            ConnectionState::ServerSendConnectResponse(stream_id, buffer) => {
                let mut stream = self.quinn.send_stream(*stream_id).unwrap();
                while let Ok(n) = stream.send(buffer) {
                    if n == 0 {
                        break;
                    }
                    buffer.drain(..n);
                }

                if buffer.is_empty() {
                    self.success(handler);
                }
            }
            ConnectionState::Connected => (),
            ConnectionState::Failed => (),
        }
    }
}

impl<'c> ConnectionMut<'c> for WebTransportConnectionMut<'c> {
    type NonMut<'b> = WebTransportConnectionRef<'b>
    where
        Self: 'b;

    type StreamType = WebTransportStreamId;

    fn as_ref<'b>(&'b self) -> Self::NonMut<'b> {
        WebTransportConnectionRef { quinn: self.quinn }
    }

    fn disconnect(&mut self) {
        self.quinn.disconnect();
    }
}

impl<'c> ConnectionRef<'c> for WebTransportConnectionRef<'c> {
    type ConnectionStats = std::net::SocketAddr;

    fn get_stats(&self) -> std::net::SocketAddr {
        self.quinn.get_stats()
    }
}

enum ReadResult {
    //We are waiting for more data.
    Wait,
    //The settings read matches our expectations so we can send a response.
    Success,
    //The settings were not parsed correctly.
    Fail,
}

fn read_settings<'s>(mut stream: QuinnRecvStreamMut<'s>, buffer: &mut Vec<u8>) -> ReadResult {
    while let Ok(data) = stream.recv(usize::MAX) {
        buffer.extend(data.as_ref());
    }

    let mut limit = std::io::Cursor::new(&buffer);
    match Settings::decode(&mut limit) {
        Ok(settings) => {
            debug!(
                "Received SETTINGS headers ({:?}) from WebTransport peer.",
                settings
            );
            if settings.supports_webtransport() != 1 {
                warn!("Peer settings indicate that WebTransport is not supported!");
                return ReadResult::Fail;
            }

            return ReadResult::Success;
        }
        Err(web_transport_proto::SettingsError::UnexpectedEnd) => {
            trace!("Partially read SETTINGs request. Buffering...");
            return ReadResult::Wait;
        }
        Err(e) => {
            error!("Error parsing WebTransport SETTINGs header: {}", e);
            return ReadResult::Fail;
        }
    }
}

fn read_connect<'s>(mut stream: QuinnRecvStreamMut<'s>, buffer: &mut Vec<u8>) -> ReadResult {
    while let Ok(data) = stream.recv(usize::MAX) {
        buffer.extend(data.as_ref());
    }

    let mut limit = std::io::Cursor::new(&buffer);
    match ConnectRequest::decode(&mut limit) {
        Ok(req) => {
            debug!(
                "Received CONNECT headers ({:?}) from WebTransport peer.",
                req
            );
            return ReadResult::Success;
        }
        Err(web_transport_proto::ConnectError::UnexpectedEnd) => {
            trace!("Partially read CONNECT request. Buffering...");
            return ReadResult::Wait;
        }
        Err(e) => {
            error!("Error parsing WebTransport CONNECT header: {}", e);
            return ReadResult::Fail;
        }
    }
}

fn read_connect_response<'s>(
    mut stream: QuinnRecvStreamMut<'s>,
    buffer: &mut Vec<u8>,
) -> ReadResult {
    while let Ok(data) = stream.recv(usize::MAX) {
        buffer.extend(data.as_ref());
    }

    let mut limit = std::io::Cursor::new(&buffer);
    match ConnectResponse::decode(&mut limit) {
        Ok(req) => {
            debug!(
                "Received CONNECT headers ({:?}) from WebTransport peer.",
                req
            );
            return ReadResult::Success;
        }
        Err(web_transport_proto::ConnectError::UnexpectedEnd) => {
            trace!("Partially read CONNECT request. Buffering...");
            ReadResult::Wait
        }
        Err(e) => {
            error!("Error parsing WebTransport CONNECT header: {}", e);
            return ReadResult::Fail;
        }
    }
}
