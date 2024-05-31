use std::{cell::Cell, net::SocketAddr, str::FromStr, sync::Arc};

use bevy::{prelude::*, utils::{HashMap, HashSet}};
use bytes::Bytes;
use quinn_proto::{Chunks, ConnectionEvent, Dir, RecvStream, StreamId};
use quinn_udp::{UdpSockRef, UdpSocketState};
use web_transport_proto::{ConnectRequest, ConnectResponse};

use crate::{endpoint::udp_transmit, EndpointBuffers, EndpointEventHandler, NewStreamHandler};



#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct ConnectionId(pub(crate) quinn_proto::ConnectionHandle);


/// A connection to a peer through a [NativeEndpoint] goverened through [quinn_proto].
///
/// Contains all of the necessary state to facilitate a connection.
pub struct ConnectionState {
    /// The underlying state behind the connection.
    connection: quinn_proto::Connection,
    /// The connection handle.
    connection_id: ConnectionId,
    /// Streams that are currently available for reading.
    /// Excludes streams that do not have any data available.
    readable_streams: HashSet<StreamId>,
    /// Streams that are currently available for writes.
    /// Excludes streams that are currently blocked or otherwise congested.
    read_responses: HashMap<StreamId, StreamReaderResponse>,
    pub(crate) new_stream_handler: Option<Arc<dyn NewStreamHandler>>,
}

impl ConnectionState {
    pub(crate) fn new(conn: quinn_proto::Connection, connection_id: ConnectionId, new_stream_handler: Option<Arc<dyn NewStreamHandler>>) -> Self {
        Self {
            connection: conn,
            connection_id,
            readable_streams: HashSet::new(),
            read_responses: HashMap::new(),
            new_stream_handler,
        }
    }

    pub fn set_new_stream_handler(&mut self, handler: Option<Arc<dyn NewStreamHandler>>) {
        self.new_stream_handler = handler;
    }

    pub fn connection_id(&self) -> ConnectionId {
        self.connection_id
    }

    pub fn remote_address(&self) -> SocketAddr {
        self.connection.remote_address()
    }

    pub fn side(&self) -> quinn_proto::Side {
        self.connection.side()
    }

    pub(crate) fn handle(&mut self, event: ConnectionEvent) {
        self.connection.handle_event(event)
    }

    pub fn disconnect(&mut self, error_code: quinn_proto::VarInt, reason: Box<[u8]>) {
        self.connection.close(std::time::Instant::now(), error_code, reason.into());
    }

    pub fn reader(&mut self, stream_id: StreamId) -> StreamReader {
        StreamReader {
            ready: if self.readable_streams.contains(&stream_id) {
                Some((
                    self.read_responses.entry(stream_id).or_insert_with(|| StreamReaderResponse::IncompleteRead),
                    self.connection.recv_stream(stream_id),
                ))
            } else {
                None
            },
        }
    }

    pub fn write(&mut self, stream_id: StreamId, data: &[u8]) -> Result<usize, WriteError> {
        match self.connection.send_stream(stream_id).write(data) {
            Err(quinn_proto::WriteError::Blocked) => Err(WriteError::Blocked),
            Err(quinn_proto::WriteError::ClosedStream) | Err(quinn_proto::WriteError::Stopped(_)) => Err(WriteError::StreamDoesntExist),
            Ok(bytes) => Ok(bytes)
        }
    }

    /// attempts to open a unidirectional stream
    ///
    /// fails if there are too many streams
    pub fn open_uni(&mut self) -> Option<StreamId> {
        self.open(quinn_proto::Dir::Uni)
    }

    pub fn open_bi(&mut self) -> Option<StreamId> {
        self.open(quinn_proto::Dir::Bi)
    }

    pub fn open(&mut self, dir: quinn_proto::Dir)-> Option<StreamId> {
        self.connection.streams().open(dir).and_then(
            |stream_id| {
                if let Some(handler) = self.new_stream_handler.take() {

                    let stream_id = if handler.new_stream(self, stream_id, dir) {
                        debug!("keeping the new stream");
                        Some(stream_id)
                    } else {
                        debug!("cancelling the new stream");
                        self.finish(stream_id);
                        None
                    };

                    self.new_stream_handler = Some(handler);
                    stream_id
                } else {
                    Some(stream_id)
                }
            }
        )
    }

    /// finishes a send stream
    pub fn finish(&mut self, stream_id: StreamId) {
        let _ = self.connection.send_stream(stream_id).finish();
    }

    pub(crate) fn poll_connection(
        &mut self,
        endpoint: &mut quinn_proto::Endpoint,
        socket: UdpSockRef,
        socket_state: &mut UdpSocketState,
        buffers: &mut EndpointBuffers,
        event_handler: &mut impl EndpointEventHandler,
    ) {
        for (stream_id, response) in std::mem::take(&mut self.read_responses) {
            match response {
                StreamReaderResponse::IncompleteRead => continue,
                StreamReaderResponse::Blocked => (),
                StreamReaderResponse::Finished => {
                    event_handler.receive_stream_closed(self, stream_id, None);
                },
                StreamReaderResponse::Reset(error_code) => {
                    event_handler.receive_stream_closed(self, stream_id, Some(error_code));
                },
            }

            self.readable_streams.remove(&stream_id);
        }

        let max_datagrams = socket_state.max_gso_segments();

        if let Some(transmit) = self.connection.poll_transmit(std::time::Instant::now(), max_datagrams, &mut buffers.send_buffer) {
            match socket_state.send(socket, &udp_transmit(&transmit, &buffers.send_buffer)) {
                Err(err) => {
                    error!("Transmition error: {}", err);
                    return;
                },
                Ok(()) => (),
            };
        }

        if let Some(deadline) = self.connection.poll_timeout() {
            let now = std::time::Instant::now();
            if deadline >= now {
                self.connection.handle_timeout(now);
            }
        }

        while let Some(endpoint_event) = self.connection.poll_endpoint_events() {
            if let Some(conn_event) = endpoint.handle_event(self.connection_id.0, endpoint_event) {
                // The endpoint gave us an event back that has to be processed.
                // This may potentially add new events to the outer loop, but it's safe to do so here.
                self.handle(conn_event);
            }
        }

        while let Some(app_event) = self.connection.poll() {
            let peer_addr = self.connection.remote_address();

            match app_event {
                quinn_proto::Event::HandshakeDataReady => {
                    trace!("Handshake data is ready for peer: {}", peer_addr);
                },
                quinn_proto::Event::Connected => {
                    debug!("Successfully connected to {}", peer_addr);
                    event_handler.new_connection(self);

                    // //WebTransport enabled and we are a 'client'
                    // if ep.web_transport && ep.cfg.1.is_none() {
                    //     let Some(send_settings) = conn.connection.streams().open(Dir::Uni) else {
                    //         warn!("Unable to open unidirectional stream to send WebTransport SETTINGs frame!");
                    //         continue;
                    //     };

                    //     send_settings_client(&mut conn.connection, send_settings);
                    // }
                },
                quinn_proto::Event::ConnectionLost { reason } => {
                    info!("Connection lost with peer: {} with reason: {}", peer_addr, reason);
                    event_handler.disconnected(self);
                },
                quinn_proto::Event::Stream(stream_event) => {
                    match stream_event {
                        quinn_proto::StreamEvent::Opened { .. } => { },
                        quinn_proto::StreamEvent::Readable { id } => {
                            self.readable_streams.insert(id);
                            debug!("Stream {} for peer {} is readable..", id, peer_addr);
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

        while let Some(stream_id) = self.connection.streams().accept(quinn_proto::Dir::Bi) {
            // if self.connection.streams().remote_open_streams(quinn_proto::Dir::Bi) == 1 && ep.web_transport {
            //     exchange_connect_server(&mut conn.connection, id);
            //     continue;
            // }

            let peer_addr = self.connection.remote_address();
            debug!("Peer: {} opened new bidrectional stream with ID: {}", peer_addr, stream_id.index());

            self.readable_streams.insert(stream_id);
            event_handler.new_stream(self, stream_id, true);
        }

        //Poll any new unidirectional streams.
        while let Some(stream_id) = self.connection.streams().accept(quinn_proto::Dir::Uni) {
            // let open_stream_count = conn.connection.streams().remote_open_streams(quinn_proto::Dir::Uni) ;

            // // We are a server with a new open uni stream.
            // if open_stream_count == 1 && ep.web_transport && ep.cfg.1.is_some() {
            //     exchange_settings_server(&mut conn.connection, stream_id);
            //     continue;
            // }

            // // We are a client and the server opened a stream with a SETTINGs response.
            // if open_stream_count == 1 && ep.web_transport && ep.cfg.1.is_none() {
            //     receive_settings_client(&mut conn.connection, stream_id);
            // }

            let peer_addr = self.connection.remote_address();
            debug!("Peer: {} opened new unidirectional stream with ID: {}", peer_addr, stream_id.index());

            self.readable_streams.insert(stream_id);
            event_handler.new_stream(self, stream_id, false);
        }
    }
}


pub struct StreamReader<'a> {
    ready: Option<(
        &'a mut StreamReaderResponse,
        RecvStream<'a>,
    )>,
}

impl<'a> StreamReader<'a> {
    pub fn read(&mut self) -> ChunksIter {
        self.read_up_to(usize::MAX)
    }

    pub fn read_up_to(&mut self, max_bytes: usize) -> ChunksIter {
        ChunksIter {
            ready: match self.ready.as_mut() {
                None => None,
                Some((response, recv)) => {
                    match recv.read(true) {
                        Ok(chunks) => Some((response, chunks, max_bytes)),

                        Err(quinn_proto::ReadableError::ClosedStream) => {
                            None
                        },
                        Err(quinn_proto::ReadableError::IllegalOrderedRead) => {
                            None
                        },
                    }
                }
            },
        }
    }
}

pub struct ChunksIter<'a> {
    ready: Option<(
        &'a mut StreamReaderResponse,
        Chunks<'a>,
        usize,
    )>,
}

impl<'a> Drop for ChunksIter<'a> {
    fn drop(&mut self) {
        if let Some((_, chunks, _)) = self.ready.take() {
            let _ = chunks.finalize();
        }
    }
}

impl<'a> Iterator for ChunksIter<'a> {
    type Item = Bytes;

    fn next(&mut self) -> Option<Self::Item> {

        match self.ready.as_mut() {
            None => None,
            Some((response, chunks, max_bytes)) => {
                match chunks.next(*max_bytes) {
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

                    Ok(Some(chunk)) => {
                        *max_bytes -= chunk.bytes.len();
                        Some(chunk.bytes)
                    },
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



#[derive(Debug)]
pub enum WriteError {
    Blocked,
    StreamDoesntExist,
}
