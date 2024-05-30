use std::str::FromStr;

use bevy::{ecs::system::SystemParam, prelude::*, utils::{HashMap, HashSet}};
use bytes::Bytes;
use quinn_proto::{ConnectionEvent, Dir, RecvStream, StreamId};
use quinn_udp::UdpSockRef;
use web_transport_proto::{ConnectRequest, ConnectResponse};

use crate::endpoint::{udp_transmit, NativeEndpoint};



pub enum StreamReader<'a> {
    Empty,
    Ready {
        response: &'a mut StreamReaderResponse,
        recv: RecvStream<'a>,
    }
}

impl<'a> Iterator for StreamReader<'a> {
    type Item = Bytes;

    fn next(&mut self) -> Option<Self::Item> {

        match self {
            Self::Empty => None,
            Self::Ready { response, recv } => {
                let mut chunks = match recv.read(true) {
                    Ok(chunks) => chunks,
                    Err(quinn_proto::ReadableError::ClosedStream) => {
                        return None;
                    },
                    Err(quinn_proto::ReadableError::IllegalOrderedRead) => {
                        return None;
                    },
                };

                match chunks.next(usize::MAX) {
                    Ok(None) => {
                        // no more data available ever, stream finished
                        **response = StreamReaderResponse::Finished;
                        None
                    },
                    Err(quinn_proto::ReadError::Reset(error_code)) => {
                        // no more data ever, peer reset stream
                        **response = StreamReaderResponse::Reset(error_code);
                        None
                    },
                    Err(quinn_proto::ReadError::Blocked) => {
                        // no more data yet
                        **response = StreamReaderResponse::Blocked;
                        None
                    },

                    Ok(Some(chunk)) => Some(chunk.bytes),
                }
            }
        }

    }
}

enum StreamReaderResponse {
    IncompleteRead,
    Blocked,
    Finished,
    Reset(quinn_proto::VarInt),
}

/// A connection to a peer through a [NativeEndpoint] goverened through [quinn_proto].
///
/// Contains all of the necessary state to facilitate a connection.
pub struct NativeConnection {
    /// The underlying state behind the connection.
    pub(crate) conn: quinn_proto::Connection,
    /// The connection handle.
    pub(crate) handle: quinn_proto::ConnectionHandle,
    /// Streams that are currently available for reading.
    /// Excludes streams that do not have any data available.
    pub(crate) read_streams: HashSet<StreamId>,
    /// Streams that are currently available for writes.
    /// Excludes streams that are currently blocked or otherwise congested.
    pub(crate) read_responses: HashMap<StreamId, StreamReaderResponse>,
}

impl NativeConnection {
    pub(crate) fn new(conn: quinn_proto::Connection, handle: quinn_proto::ConnectionHandle) -> Self {
        Self {
            conn,
            handle,
            read_streams: HashSet::new(),
            read_responses: HashMap::new(),
        }
    }

    pub(crate) fn handle(&mut self, event: ConnectionEvent) {
        self.conn.handle_event(event)
    }

    pub fn read(&mut self, stream_id: StreamId) -> StreamReader {
        if self.read_streams.contains(&stream_id) {
            StreamReader::Ready {
                response: self.read_responses.entry(stream_id).or_insert_with(|| StreamReaderResponse::IncompleteRead),
                recv: self.conn.recv_stream(stream_id),
            }
        } else {
            StreamReader::Empty
        }
    }

    pub fn write(&mut self, stream_id: StreamId, data: &[u8]) -> Result<usize, WriteError> {
        match self.conn.send_stream(stream_id).write(data) {
            Err(quinn_proto::WriteError::Blocked) => Err(WriteError::Blocked),
            Err(quinn_proto::WriteError::ClosedStream) | Err(quinn_proto::WriteError::Stopped(_)) => Err(WriteError::StreamDoesntExist),
            Ok(bytes) => Ok(bytes)
        }
    }
}

#[derive(Debug)]
pub enum WriteError {
    Blocked,
    StreamDoesntExist,
}


pub struct ConnectionId(pub(crate) quinn_proto::ConnectionHandle);

/// A new stream has been established for the following (endpoint, connection, stream) that can be written to.
#[derive(Event)]
pub struct NewWriteStream(pub Entity, pub ConnectionId, pub StreamId);

/// A new stream has been established for the following (endpoint, connection, stream) that can be read from.
#[derive(Event)]
pub struct NewReadStream(pub Entity, pub ConnectionId, pub StreamId);

/// a receive stream at (endpoint, connection, stream) has been closed.
#[derive(Event)]
pub struct ClosedStream(pub Entity, pub ConnectionId, pub StreamId, pub Option<quinn_proto::VarInt>);


#[derive(Event)]
pub struct Connected(pub Entity, pub ConnectionId);

#[derive(Event)]
pub struct Disconnected(pub Entity, pub ConnectionId);


#[derive(SystemParam)]
pub struct ConnectionEventWriters<'w> {
    new_read_stream_w: EventWriter<'w, NewReadStream>,
    new_write_stream_w: EventWriter<'w, NewWriteStream>,
    closed_stream_w: EventWriter<'w, ClosedStream>,

    client_connect_w: EventWriter<'w, Connected>,
    client_disconnect_w: EventWriter<'w, Disconnected>
}


fn poll_endpoint_connections(ep_ent: Entity, ep: &mut NativeEndpoint, writers: &mut ConnectionEventWriters) {
    for (handle, conn) in ep.connections.iter_mut() {

        for (stream_id, response) in conn.read_responses.drain() {
            match response {
                StreamReaderResponse::IncompleteRead => continue,
                StreamReaderResponse::Blocked => (),
                StreamReaderResponse::Finished => {
                    writers.closed_stream_w.send(ClosedStream(ep_ent, ConnectionId(conn.handle), stream_id, None));
                },
                StreamReaderResponse::Reset(error_code) => {
                    writers.closed_stream_w.send(ClosedStream(ep_ent, ConnectionId(conn.handle), stream_id, Some(error_code)));
                },
            }

            conn.read_streams.remove(&stream_id);
        }

        let max_datagrams = ep.sock.1.max_gso_segments();
        let mut send_buffer = Vec::with_capacity(conn.conn.current_mtu() as usize);

        if let Some(tx) = conn.conn.poll_transmit(std::time::Instant::now(), max_datagrams, &mut send_buffer) {
            match ep.sock.1.send(UdpSockRef::from(&ep.sock.0), &udp_transmit(&tx, &send_buffer)) {
                Err(err) => {
                    error!("A transmission error occured while sending a connection response: {}", err);
                    continue;
                },
                Ok(_) => {
                    trace!("Sent connection reponse to peer.");
                }
            };
        }

        if let Some(deadline) = conn.conn.poll_timeout() {
            if deadline >= std::time::Instant::now() {
                conn.conn.handle_timeout(std::time::Instant::now());
            }
        }

        while let Some(ep_event) = conn.conn.poll_endpoint_events() {
            if let Some(conn_event) = ep.endpoint.handle_event(*handle, ep_event) {
                //The endpoint gave us an event back that has to be processed.
                //This may potentially add new events to the outer loop, but it's safe to do so here.
                conn.handle(conn_event);
            }
        }

        while let Some(app_event) = conn.conn.poll() {
            let peer_addr = conn.conn.remote_address();

            match app_event {
                quinn_proto::Event::HandshakeDataReady => {
                    trace!("Handshake data is ready for peer: {}", peer_addr);
                },
                quinn_proto::Event::Connected => {
                    debug!("Peer: {} has successfully connected.", peer_addr);
                    writers.client_connect_w.send(Connected(ep_ent, ConnectionId(conn.handle)));

                    //WebTransport enabled and we are a 'client'
                    if ep.web_transport && ep.cfg.1.is_none() {
                        let Some(send_settings) = conn.conn.streams().open(Dir::Uni) else {
                            warn!("Unable to open unidirectional stream to send WebTransport SETTINGs frame!");
                            continue;
                        };

                        send_settings_client(&mut conn.conn, send_settings);
                    }
                },
                quinn_proto::Event::ConnectionLost { reason } => {
                    info!("Connection lost with peer: {} with reason: {}", peer_addr, reason);
                    writers.client_disconnect_w.send(Disconnected(ep_ent, ConnectionId(conn.handle)));
                },
                quinn_proto::Event::Stream(stream_event) => {
                    match stream_event {
                        quinn_proto::StreamEvent::Opened { .. } => { },
                        quinn_proto::StreamEvent::Readable { id } => {
                            conn.read_streams.insert(id);
                            trace!("Stream {} for peer {} is readable..", id, peer_addr);
                        },
                        quinn_proto::StreamEvent::Writable { id } => {
                            trace!("Stream {} for peer {} is writable.", id, peer_addr);
                        },
                        quinn_proto::StreamEvent::Finished { id } => {
                            // this endpoint has finished transmitting all data on some send stream
                            // ack that all data was received after stream was initially requested to be 'finished'.
                            trace!("finished transmitting on stream {} for peer {}.", id, peer_addr);
                        },
                        quinn_proto::StreamEvent::Stopped { id, error_code } => {
                            // the peer has reset a send stream
                            trace!("Stream {} for peer {} has been stopped with error: {}", id, peer_addr, error_code);
                        },
                        quinn_proto::StreamEvent::Available { .. } => {
                            warn!("A new stream is available that has not been responded to!");
                        },
                    }
                },
                quinn_proto::Event::DatagramReceived => {
                    trace!("Received a datagram for peer: {}", peer_addr);
                },
                quinn_proto::Event::DatagramsUnblocked => {
                    trace!("Datagrams unblocked for peer: {}", peer_addr);
                },
            }
        }

        while let Some(id) = conn.conn.streams().accept(quinn_proto::Dir::Bi) {
            if conn.conn.streams().remote_open_streams(quinn_proto::Dir::Bi) == 1 && ep.web_transport {
                exchange_connect_server(&mut conn.conn, id);
                continue;
            }

            let peer_addr = conn.conn.remote_address();
            debug!("Peer: {} opened new bidrectional stream with ID: {}", peer_addr, id.index());

            writers.new_read_stream_w.send(NewReadStream(ep_ent, ConnectionId(conn.handle), id));
            writers.new_write_stream_w.send(NewWriteStream(ep_ent, ConnectionId(conn.handle), id));
        }

        //Poll any new unidirectional streams.
        while let Some(id) = conn.conn.streams().accept(quinn_proto::Dir::Uni) {
            let open_stream_count = conn.conn.streams().remote_open_streams(quinn_proto::Dir::Uni) ;

            //We are a server with a new open uni stream.
            if open_stream_count == 1 && ep.web_transport && ep.cfg.1.is_some() {
                exchange_settings_server(&mut conn.conn, id);
                continue;
            }

            //We are a client and the server opened a stream with a SETTINGs response.
            if open_stream_count == 1 && ep.web_transport && ep.cfg.1.is_none() {
                receive_settings_client(&mut conn.conn, id);
            }

            let peer_addr = conn.conn.remote_address();
            debug!("Peer: {} opened new unidirectional stream with ID: {}", peer_addr, id.index());
            writers.new_read_stream_w.send(NewReadStream(ep_ent, ConnectionId(conn.handle), id));
        }
    }
}


// Sends the 'SETTINGs' frame through the specified outbound unidirectional stream.
fn send_settings_client(conn: &mut quinn_proto::Connection, uni: StreamId) {
    let mut settings = web_transport_proto::Settings::default();
    settings.enable_webtransport(1);

    debug!("Sending WebTransport SETTINGs frame");

    let mut buf = Vec::new();
    settings.encode(&mut buf);

    if let Err(e) =  conn.send_stream(uni).write(&buf) {
        warn!("Received an error while sending WebTransport SETTINGs frame: {}", e);
    }
}

// Processes the SETTINGs response received in the inbound unidirectional stream, 'uni'.
fn receive_settings_client(conn: &mut quinn_proto::Connection, uni: StreamId) {
    let mut buf = Vec::new();

    // First, read the entire stream
    if let Ok(mut reader) = conn.recv_stream(uni).read(true) {
        loop {
            if let Some(chunk) = reader.next(usize::MAX).ok() {
                // Unwrap the Option<Chunk> to get the Chunk
                let chunk = chunk.unwrap();
                buf.extend_from_slice(&chunk.bytes);
                let mut limit = std::io::Cursor::new(&buf);

                match web_transport_proto::Settings::decode(&mut limit) {
                    Ok(settings) => {
                        trace!("Received SETTINGS frame: {:?}", settings);
                        if settings.supports_webtransport() == 0 {
                            info!("Server does not support WebTransport!");
                        } else {
                            trace!("Server supports WebTransport.");
                        }
                        let _ = reader.finalize();
                        break;
                    }
                    Err(web_transport_proto::SettingsError::UnexpectedEnd) => continue,
                    Err(e) => {
                        warn!("Received an error while decoding WebTransport settings response: {}", e);
                        let _ = reader.finalize();
                        return;
                    }
                }
            } else {
                warn!("Error reading from stream");
                let _ = reader.finalize();
                return;
            }
        }
    } else {
        debug!("Unable to read first sent stream. It may not be a WebTransport stream");
        return;
    }

    trace!("WebTransport response was valid. Sending CONNECT header.");

    buf.clear();

    let Some(bidir) = conn.streams().open(Dir::Bi) else {
        warn!("Unable to open bidirectional stream to send CONNECT header to server");
        return;
    };

    //We do not have to have a real url in this packet, as long as the server recognizes the request url sent.
    let connect_req = ConnectRequest { url: url::Url::from_str("https://nevy.client").unwrap() };
    connect_req.encode(&mut buf);

    let Err(e) = conn.send_stream(bidir).write(&buf) else {
        trace!("Successfully sent CONNECT header to the server.");
        return;
    };

    warn!("Received and error while writing the CONNECT request to the server: {}", e);
}


//Provided a endpoint and a unidirectional stream with SETTINGs, will try to negotiate and respond to this request.
fn exchange_settings_server(conn: &mut quinn_proto::Connection, id: StreamId) {
    let mut buf = Vec::new();

    // First, read the entire stream
    if let Ok(mut reader) = conn.recv_stream(id).read(true) {
        loop {
            if let Some(chunk) = reader.next(usize::MAX).ok() {
                // Unwrap the Option<Chunk> to get the Chunk
                let chunk = chunk.unwrap();
                buf.extend_from_slice(&chunk.bytes);
                let mut limit = std::io::Cursor::new(&buf);

                match web_transport_proto::Settings::decode(&mut limit) {
                    Ok(settings) => {
                        trace!("Received SETTINGS frame: {:?}", settings);
                        if settings.supports_webtransport() == 0 {
                            info!("Peer does not support WebTransport!");
                        } else {
                            trace!("Peer supports WebTransport.");
                        }
                        let _ = reader.finalize();
                        break;
                    }
                    Err(web_transport_proto::SettingsError::UnexpectedEnd) => continue,
                    Err(e) => {
                        warn!("Received an error while decoding WebTransport settings header: {}", e);
                        let _ = reader.finalize();
                        return;
                    }
                }
            } else {
                warn!("Error reading from stream");
                let _ = reader.finalize();
                return;
            }
        }
    } else {
        debug!("Unable to read first sent stream. It may not be a WebTransport stream");
        return;
    }

    // Now send the response
    let mut setting_resp = web_transport_proto::Settings::default();
    setting_resp.enable_webtransport(1);
    debug!("Sending SETTINGS frame response: {:?}", setting_resp);

    buf.clear();
    setting_resp.encode(&mut buf);

    if let Some(resp_stream) = conn.streams().open(quinn_proto::Dir::Uni) {
        if let Err(e) = conn.send_stream(resp_stream).write(&buf) {
            warn!("Failed to send SETTINGS response to peer: {}", e);
        }
    } else {
        warn!("Failed to open stream for WebTransport SETTINGS reply.");
    }
}

fn exchange_connect_server(conn: &mut quinn_proto::Connection, id: StreamId) {
    trace!("Client accepted our SETTINGs header, negotiating final CONNECT headers.");

    let mut buf = Vec::new();

    // First, read the entire CONNECT request
    if let Ok(mut reader) = conn.recv_stream(id).read(true) {
        loop {
            if let Some(chunk) = reader.next(usize::MAX).ok() {
                let chunk = chunk.unwrap();
                buf.extend_from_slice(&chunk.bytes);
                let mut limit = std::io::Cursor::new(&buf);

                match ConnectRequest::decode(&mut limit) {
                    Ok(request) => {
                        debug!("Received CONNECT request: {:?}", request);
                        let _ = reader.finalize();
                        break;
                    }
                    Err(web_transport_proto::ConnectError::UnexpectedEnd) => {
                        trace!("Buffering CONNECT request");
                        continue;
                    }
                    Err(e) => {
                        error!("Error parsing CONNECT request header: {}", e);
                        let _ = reader.finalize();
                        return;
                    }
                }
            } else {
                warn!("Error reading CONNECT request from stream");
                let _ = reader.finalize();
                return;
            }
        }
    } else {
        warn!("Unable to read CONNECT request from stream");
        return;
    }

    // Now send the CONNECT response
    let resp = ConnectResponse { status: default() };
    debug!("Sending CONNECT response: {:?}", resp);

    buf.clear();
    resp.encode(&mut buf);

    let mut write_stream = conn.send_stream(id);

    if let Err(e) = write_stream.write(&buf) {
        warn!("Failed to write a response to the CONNECT request: {}", e);
    }

}



pub(crate) fn connection_poll_sys(
    mut ep_q: Query<(Entity, &mut NativeEndpoint)>,
    mut writers: ConnectionEventWriters
) {
    for (ep_ent, mut endpoint) in ep_q.iter_mut() {
        //TODO: poll only connections that are relevant based on some buffer, etc.
        poll_endpoint_connections(ep_ent, &mut endpoint, &mut writers);
    }
}