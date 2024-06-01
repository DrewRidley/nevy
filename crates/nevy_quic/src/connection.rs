use transport_interface::*;

use crate::QuinnContext;

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct QuinnConnectionId(pub(crate) quinn_proto::ConnectionHandle);

pub struct QuinnConnection {
    pub(crate) connection: quinn_proto::Connection,
    connection_id: QuinnConnectionId,
}

impl QuinnConnection {
    pub(crate) fn new(
        connection: quinn_proto::Connection,
        connection_id: QuinnConnectionId,
    ) -> Self {
        QuinnConnection {
            connection,
            connection_id,
        }
    }

    pub(crate) fn process_event(&mut self, event: quinn_proto::ConnectionEvent) {
        self.connection.handle_event(event);
    }

    pub(crate) fn poll_timeouts(&mut self, context: &mut QuinnContext) {
        let _ = context;
        let now = std::time::Instant::now();
        while let Some(deadline) = self.connection.poll_timeout() {
            if deadline <= now {
                self.connection.handle_timeout(now);
            } else {
                break;
            }
        }
    }

    pub(crate) fn poll_events(&mut self, context: &mut QuinnContext) {
        while let Some(app_event) = self.connection.poll() {
            match app_event {
                quinn_proto::Event::HandshakeDataReady => (),
                quinn_proto::Event::Connected => {
                    context.events.push_back(EndpointEvent {
                        connection_id: self.connection_id,
                        event: ConnectionEvent::Connected,
                    });
                }
                quinn_proto::Event::ConnectionLost { reason: _ } => {}
                quinn_proto::Event::Stream(_s) => {}
                quinn_proto::Event::DatagramReceived => {}
                quinn_proto::Event::DatagramsUnblocked => {}
            }
        }
    }
}

impl Connection for QuinnConnection {
    type Context<'c> = &'c mut QuinnContext;

    type Id = QuinnConnectionId;

    fn disconnect<'c>(&mut self, context: &mut Self::Context<'c>) {
        todo!()
    }

    fn send_stream<S>(&self, stream_id: S) -> Option<&S::Stream>
    where
        S: StreamId,
        S::Stream: SendStream,
    {
        todo!()
    }

    fn send_stream_mut<'c, S>(
        &mut self,
        stream_id: S,
        context: &mut Self::Context<'c>,
    ) -> Option<&mut S::Stream>
    where
        S: StreamId,
        S::Stream: SendStream,
    {
        todo!()
    }

    fn recv_stream<S>(&self, stream_id: S) -> Option<&S::Stream>
    where
        S: StreamId,
        S::Stream: RecvStream,
    {
        todo!()
    }

    fn recv_stream_mut<'c, S>(
        &mut self,
        stream_id: S,
        context: &mut Self::Context<'c>,
    ) -> Option<&mut S::Stream>
    where
        S: StreamId,
        S::Stream: RecvStream,
    {
        todo!()
    }

    fn poll_stream_event<'c, S>(
        &mut self,
        context: &mut Self::Context<'c>,
    ) -> Option<StreamEvent<S>>
    where
        S: StreamId,
    {
        todo!()
    }
}

struct QuinnStreamId(quinn_proto::StreamId);

struct QuinnSendStream {}

struct QuinnRecvStream {}

impl StreamId for QuinnStreamId {
    type Stream = QuinnSendStream;
}

impl StreamId for QuinnStreamId {
    type Stream = QuinnRecvStream;
}

struct QuinnDatagram;

impl StreamId for QuinnDatagram {
    type Stream = QuinnDatagram;
}
