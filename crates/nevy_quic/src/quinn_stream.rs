use std::collections::{HashSet, VecDeque};

use transport_interface::*;

use crate::connection::QuinnConnection;

/// stream id for a quinn stream
#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct QuinnStreamId(pub(crate) quinn_proto::StreamId);

pub struct QuinnSendStreamMut<'s> {
    stream: quinn_proto::SendStream<'s>,
    stream_id: QuinnStreamId,
    open_streams: &'s mut HashSet<QuinnStreamId>,
}

pub struct QuinnRecvStreamMut<'s> {
    events: &'s mut VecDeque<StreamEvent<QuinnStreamId>>,
    stream_id: QuinnStreamId,
    stream: quinn_proto::RecvStream<'s>,
    open_streams: &'s mut HashSet<QuinnStreamId>,
}

#[derive(Debug)]
pub enum QuinnSendError {
    /// the stream is blocked because the peer cannot accept more data
    /// or the stream is congested
    Blocked,
    /// the stream has never been opened, has been finished or was reset
    NoStream,
}

#[derive(Debug)]
pub enum QuinnReadError {
    /// the stream is blocked and there is no more data to be read
    ///
    /// this may be followed by a closed stream event
    Blocked,
    /// the stream has finished and no more data will be available
    Finished,
    /// the stream has never been opened, has been finished or was reset
    NoStream,
}

impl ErrorFatality for QuinnSendError {
    fn is_fatal(&self) -> bool {
        match self {
            QuinnSendError::Blocked => false,
            QuinnSendError::NoStream => true,
        }
    }
}

impl ErrorFatality for QuinnReadError {
    fn is_fatal(&self) -> bool {
        match self {
            QuinnReadError::Blocked => false,
            QuinnReadError::Finished => false,
            QuinnReadError::NoStream => true,
        }
    }
}

impl<'s, 'c> SendStreamMut<'s> for QuinnSendStreamMut<'s> {
    type SendError = QuinnSendError;

    type CloseDescription = Option<quinn_proto::VarInt>;

    fn send(&mut self, data: &[u8]) -> Result<usize, Self::SendError> {
        match self.stream.write(data) {
            Ok(n) => Ok(n),
            Err(quinn_proto::WriteError::Blocked) => Err(QuinnSendError::Blocked),
            Err(quinn_proto::WriteError::ClosedStream)
            | Err(quinn_proto::WriteError::Stopped(_)) => Err(QuinnSendError::NoStream),
        }
    }

    fn close(&mut self, description: Self::CloseDescription) -> Result<(), ()> {
        if match description {
            Some(reset_error_code) => self.stream.reset(reset_error_code).is_ok(),
            None => self.stream.finish().is_ok(),
        } {
            self.open_streams.remove(&self.stream_id);

            Ok(())
        } else {
            Err(())
        }
    }

    fn is_open(&self) -> bool {
        self.open_streams.contains(&self.stream_id)
    }
}

impl<'s, 'c> RecvStreamMut<'s> for QuinnRecvStreamMut<'s> {
    type ReadError = QuinnReadError;

    type CloseDescription = quinn_proto::VarInt;

    fn recv(&mut self, limit: usize) -> Result<Box<[u8]>, Self::ReadError> {
        let mut chunks = match self.stream.read(true) {
            Ok(chunks) => chunks,
            Err(quinn_proto::ReadableError::ClosedStream) => return Err(QuinnReadError::NoStream),
            Err(quinn_proto::ReadableError::IllegalOrderedRead) => {
                unreachable!("will never read unordered")
            }
        };

        let bytes = match chunks.next(limit) {
            Ok(None) => {
                self.open_streams.remove(&self.stream_id);

                self.events.push_back(StreamEvent {
                    stream_id: self.stream_id,
                    peer_generated: true,
                    event_type: StreamEventType::ClosedRecvStream,
                });

                Err(QuinnReadError::Blocked)
            }
            Ok(Some(chunk)) => Ok(chunk.bytes.as_ref().into()),
            Err(quinn_proto::ReadError::Blocked) => Err(QuinnReadError::Blocked),
            Err(quinn_proto::ReadError::Reset(_)) => Err(QuinnReadError::NoStream),
        };

        let _ = chunks.finalize();

        bytes
    }

    fn close(&mut self, description: Self::CloseDescription) -> Result<(), ()> {
        match self.stream.stop(description) {
            Ok(()) => {
                self.open_streams.remove(&self.stream_id);

                Ok(())
            }
            Err(_) => Err(()),
        }
    }

    fn is_open(&self) -> bool {
        self.open_streams.contains(&self.stream_id)
    }
}

impl StreamId for QuinnStreamId {
    type Connection<'c> = &'c mut QuinnConnection;

    type SendMut<'s> = QuinnSendStreamMut<'s>;

    type RecvMut<'s> = QuinnRecvStreamMut<'s>;

    type OpenDescription = quinn_proto::Dir;

    fn open<'c>(
        connection: &mut &'c mut QuinnConnection,
        description: Self::OpenDescription,
    ) -> Option<Self> {
        let stream_id = QuinnStreamId(connection.connection.streams().open(description)?);

        connection.open_send_streams.insert(stream_id);
        if let quinn_proto::Dir::Bi = description {
            connection.open_recv_streams.insert(stream_id);
        }

        Some(stream_id)
    }

    fn get_send<'c, 's>(
        self,
        connection: &'s mut &'c mut QuinnConnection,
    ) -> Option<Self::SendMut<'s>> {
        if !connection.open_send_streams.contains(&self) {
            return None;
        }

        Some(QuinnSendStreamMut {
            stream: connection.connection.send_stream(self.0),
            stream_id: self,
            open_streams: &mut connection.open_send_streams,
        })
    }

    fn get_recv<'c, 's>(
        self,
        connection: &'s mut &'c mut QuinnConnection,
    ) -> Option<Self::RecvMut<'s>> {
        if !connection.open_recv_streams.contains(&self) {
            return None;
        }

        Some(QuinnRecvStreamMut {
            events: &mut connection.stream_events,
            stream_id: self,
            stream: connection.connection.recv_stream(self.0),
            open_streams: &mut connection.open_recv_streams,
        })
    }

    fn poll_events<'c>(connection: &mut &'c mut QuinnConnection) -> Option<StreamEvent<Self>> {
        connection.stream_events.pop_front()
    }
}
