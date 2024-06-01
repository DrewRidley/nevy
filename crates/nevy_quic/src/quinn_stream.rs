use std::collections::VecDeque;

use transport_interface::*;

use crate::connection::QuinnConnection;

/// stream id for a quinn stream
#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct QuinnStreamId(pub(crate) quinn_proto::StreamId);

pub struct QuinnSendStreamMut<'s> {
    stream: quinn_proto::SendStream<'s>,
}

pub struct QuinnRecvStreamMut<'s> {
    events: &'s mut VecDeque<StreamEvent<QuinnStreamId>>,
    stream_id: QuinnStreamId,
    stream: quinn_proto::RecvStream<'s>,
}

pub struct QuinnRecvStreamRef;

pub struct QuinnSendStreamRef;

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
    /// the stream has never been opened, has been finished or was reset
    NoStream,
}

impl<'s, 'c> SendStreamMut<'s> for QuinnSendStreamMut<'s> {
    type NonMut = QuinnSendStreamRef;

    type SendError = QuinnSendError;

    type CloseDescription = Option<quinn_proto::VarInt>;

    fn as_ref(&self) -> Self::NonMut {
        QuinnSendStreamRef
    }

    fn send(&mut self, data: &[u8]) -> Result<usize, Self::SendError> {
        match self.stream.write(data) {
            Ok(n) => Ok(n),
            Err(quinn_proto::WriteError::Blocked) => Err(QuinnSendError::Blocked),
            Err(quinn_proto::WriteError::ClosedStream)
            | Err(quinn_proto::WriteError::Stopped(_)) => Err(QuinnSendError::NoStream),
        }
    }

    fn close(&mut self, description: Self::CloseDescription) -> Result<(), ()> {
        match description {
            Some(reset_error_code) => self.stream.reset(reset_error_code).map_err(|_| ()),
            None => self.stream.finish().map_err(|_| ()),
        }
    }
}

impl<'s, 'c> SendStreamRef<'s> for QuinnSendStreamRef {}

impl<'s, 'c> RecvStreamMut<'s> for QuinnRecvStreamMut<'s> {
    type NonMut = QuinnRecvStreamRef;

    type ReadError = QuinnReadError;

    type CloseDescription = quinn_proto::VarInt;

    fn as_ref(&self) -> Self::NonMut {
        QuinnRecvStreamRef
    }

    fn recv(&mut self, limit: usize) -> Result<Box<[u8]>, Self::ReadError> {
        let mut chunks = match self.stream.read(true) {
            Ok(chunks) => chunks,
            Err(quinn_proto::ReadableError::ClosedStream) => return Err(QuinnReadError::NoStream),
            Err(quinn_proto::ReadableError::IllegalOrderedRead) => {
                unreachable!("will never read unorered")
            }
        };

        let bytes = match chunks.next(limit) {
            Ok(None) => {
                self.events
                    .push_back(StreamEvent::ClosedRecvStream(self.stream_id));

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
        self.stream.stop(description).map_err(|_| ())
    }
}

impl<'s, 'c> RecvStreamRef<'s> for QuinnRecvStreamRef {}

impl<'s, 'c: 's> StreamId<'s, 'c, &'c mut QuinnConnection> for QuinnStreamId {
    type SendMut = QuinnSendStreamMut<'s>;

    type RecvMut = QuinnRecvStreamMut<'s>;

    type OpenDescription = quinn_proto::Dir;

    fn open(
        connection: &mut &'c mut QuinnConnection,
        description: Self::OpenDescription,
    ) -> Option<Self> {
        Some(QuinnStreamId(
            connection.connection.streams().open(description)?,
        ))
    }

    fn get_send_mut(self, connection: &'s mut &'c mut QuinnConnection) -> Option<Self::SendMut>
    where
        'c: 's,
    {
        Some(QuinnSendStreamMut {
            stream: connection.connection.send_stream(self.0),
        })
    }

    fn get_recv_mut(self, connection: &'s mut &'c mut QuinnConnection) -> Option<Self::RecvMut> {
        Some(QuinnRecvStreamMut {
            events: &mut connection.stream_events,
            stream_id: self,
            stream: connection.connection.recv_stream(self.0),
        })
    }

    fn get_send(
        self,
        _connection: &'s &'c QuinnConnection,
    ) -> Option<<Self::SendMut as SendStreamMut<'s>>::NonMut> {
        Some(QuinnSendStreamRef)
    }

    fn get_recv(
        self,
        _connection: &'s &'c QuinnConnection,
    ) -> Option<<Self::RecvMut as RecvStreamMut<'s>>::NonMut> {
        Some(QuinnRecvStreamRef)
    }

    fn poll_events(connection: &mut &'c mut QuinnConnection) -> Option<StreamEvent<Self>> {
        connection.stream_events.pop_front()
    }
}
