use std::collections::VecDeque;

use transport_interface::*;

use crate::{endpoint::QuinnEndpoint, quinn_stream::QuinnStreamId};

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct QuinnConnectionId(pub(crate) quinn_proto::ConnectionHandle);

pub struct QuinnConnection {
    pub(crate) connection: quinn_proto::Connection,
    pub(crate) connection_id: QuinnConnectionId,
    pub(crate) stream_events: VecDeque<StreamEvent<QuinnStreamId>>,
}

impl QuinnConnection {
    pub(crate) fn new(
        connection: quinn_proto::Connection,
        connection_id: QuinnConnectionId,
    ) -> Self {
        QuinnConnection {
            connection,
            connection_id,
            stream_events: VecDeque::new(),
        }
    }

    pub(crate) fn process_event(&mut self, event: quinn_proto::ConnectionEvent) {
        self.connection.handle_event(event);
    }

    pub(crate) fn poll_timeouts(&mut self) {
        let now = std::time::Instant::now();
        while let Some(deadline) = self.connection.poll_timeout() {
            if deadline <= now {
                self.connection.handle_timeout(now);
            } else {
                break;
            }
        }
    }

    pub(crate) fn poll_events(&mut self, events: &mut VecDeque<EndpointEvent<QuinnEndpoint>>) {
        while let Some(app_event) = self.connection.poll() {
            match app_event {
                quinn_proto::Event::HandshakeDataReady => (),
                quinn_proto::Event::Connected => {
                    events.push_back(EndpointEvent {
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

    pub(crate) fn accept_streams(&mut self) {
        while let Some(stream_id) = self.connection.streams().accept(quinn_proto::Dir::Uni) {
            let stream_id = QuinnStreamId(stream_id);

            self.stream_events
                .push_back(StreamEvent::NewRecvStream(stream_id));
        }

        while let Some(stream_id) = self.connection.streams().accept(quinn_proto::Dir::Bi) {
            let stream_id = QuinnStreamId(stream_id);

            self.stream_events
                .push_back(StreamEvent::NewRecvStream(stream_id));
            self.stream_events
                .push_back(StreamEvent::NewSendStream(stream_id));
        }
    }
}

impl<'c> ConnectionMut<'c> for &'c mut QuinnConnection {
    type NonMut = &'c QuinnConnection;

    fn as_ref(&'c self) -> Self::NonMut {
        self
    }

    fn disconnect(&mut self) {
        todo!("disconnection")
    }
}

impl<'c> ConnectionRef<'c> for &'c QuinnConnection {
    type Mut = &'c mut QuinnConnection;
}
