use log::debug;
use nevy_quic::prelude::*;
use transport_interface::*;
use web_transport_proto::Frame;

use crate::connection::{WebTransportConnectionMut, WebTransportConnectionRef};

#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct WebTransportStreamId(pub(crate) QuinnStreamId);

pub(crate) struct WebTransportSendStream {
    /// the header data if it has not been sent
    pub(crate) header: Option<Vec<u8>>,
}

pub(crate) struct WebTransportRecvStream {
    /// the header data if it has not been fully received yet.
    pub(crate) header: Option<Vec<u8>>,
}

pub struct WebTransportSendStreamMut<'s> {
    state: &'s mut WebTransportSendStream,
    stream: QuinnSendStreamMut<'s>,
}

pub struct WebTransportSendStreamRef;

pub struct WebTransportRecvStreamMut<'s> {
    state: &'s mut WebTransportRecvStream,
    stream: QuinnRecvStreamMut<'s>,
}

pub struct WebTransportRecvStreamRef;

impl<'s> SendStreamMut<'s> for WebTransportSendStreamMut<'s> {
    type NonMut<'b> = WebTransportSendStreamRef
    where
        Self: 'b;

    type SendError = QuinnSendError;

    type CloseDescription = Option<quinn_proto::VarInt>;

    fn as_ref<'b>(&'b self) -> Self::NonMut<'b> {
        todo!()
    }

    fn send(&mut self, data: &[u8]) -> Result<usize, Self::SendError> {
        if let Some(header) = self.state.header.as_mut() {
            loop {
                let n = self.stream.send(header)?;
                if n == 0 {
                    break;
                }
                header.drain(..n);
            }
        }

        self.stream.send(data)
    }

    fn close(&mut self, description: Self::CloseDescription) -> Result<(), ()> {
        self.stream.close(description)
    }
}

impl<'s> SendStreamRef<'s> for WebTransportSendStreamRef {}

impl<'s> RecvStreamMut<'s> for WebTransportRecvStreamMut<'s> {
    type NonMut<'b> = WebTransportRecvStreamRef
    where
        Self: 'b;

    type ReadError = QuinnReadError;

    type CloseDescription = quinn_proto::VarInt;

    fn as_ref<'b>(&'b self) -> Self::NonMut<'b> {
        todo!()
    }

    fn recv(&mut self, limit: usize) -> Result<Box<[u8]>, Self::ReadError> {
        if let Some(header) = self.state.header.as_mut() {
            loop {
                let bytes = self.stream.recv(1)?;
                header.extend(bytes.as_ref());

                match Frame::decode(&mut header.as_ref()) {
                    Ok(_) => {
                        self.state.header = None;
                        break;
                    }
                    Err(web_transport_proto::VarIntUnexpectedEnd) => continue,
                }
            }
        }

        self.stream.recv(limit)
    }

    fn close(&mut self, description: Self::CloseDescription) -> Result<(), ()> {
        self.stream.close(description)
    }
}

impl<'s> RecvStreamRef<'s> for WebTransportRecvStreamRef {}

impl<'c> StreamId<'c, WebTransportConnectionMut<'c>> for WebTransportStreamId {
    type SendMut<'s> = WebTransportSendStreamMut<'s>
    where
        Self: 's;

    type RecvMut<'s> = WebTransportRecvStreamMut<'s>
    where
        Self: 's;

    type OpenDescription = quinn_proto::Dir;

    fn open(
        connection: &mut WebTransportConnectionMut<'c>,
        description: Self::OpenDescription,
    ) -> Option<Self> {
        if !connection.web_transport.is_connected() {
            return None;
        }

        let stream_id = WebTransportStreamId(connection.quinn.open_stream(description)?);

        let mut buf = vec![];
        Frame::WEBTRANSPORT.encode(&mut buf);

        connection
            .web_transport
            .send_streams
            .insert(stream_id, WebTransportSendStream { header: Some(buf) });
        connection
            .web_transport
            .stream_events
            .push_back(StreamEvent {
                stream_id,
                peer_generated: false,
                event_type: StreamEventType::NewSendStream,
            });
        debug!("local opened send stream");

        if let quinn_proto::Dir::Bi = description {
            connection.web_transport.recv_streams.insert(
                stream_id,
                WebTransportRecvStream {
                    header: Some(Vec::new()),
                },
            );
            connection
                .web_transport
                .stream_events
                .push_back(StreamEvent {
                    stream_id,
                    peer_generated: false,
                    event_type: StreamEventType::NewRecvStream,
                });
            debug!("local opened recv stream");
        }

        Some(stream_id)
    }

    fn get_send_mut<'s>(
        self,
        connection: &'s mut WebTransportConnectionMut<'c>,
    ) -> Option<Self::SendMut<'s>> {
        Some(WebTransportSendStreamMut {
            state: connection.web_transport.send_streams.get_mut(&self)?,
            stream: connection.quinn.send_stream_mut(self.0)?,
        })
    }

    fn get_recv_mut<'s>(
        self,
        connection: &'s mut WebTransportConnectionMut<'c>,
    ) -> Option<Self::RecvMut<'s>> {
        Some(WebTransportRecvStreamMut {
            state: connection.web_transport.recv_streams.get_mut(&self)?,
            stream: connection.quinn.recv_stream_mut(self.0)?,
        })
    }

    fn get_send<'s>(
        self,
        _connection: &'s WebTransportConnectionRef<'c>,
    ) -> Option<<Self::SendMut<'s> as SendStreamMut<'s>>::NonMut<'s>> {
        Some(WebTransportSendStreamRef)
    }

    fn get_recv<'s>(
        self,
        _connection: &'s WebTransportConnectionRef<'c>,
    ) -> Option<<Self::RecvMut<'s> as RecvStreamMut<'s>>::NonMut<'s>> {
        Some(WebTransportRecvStreamRef)
    }

    fn poll_events(connection: &mut WebTransportConnectionMut<'c>) -> Option<StreamEvent<Self>> {
        connection.web_transport.stream_events.pop_front()
    }
}
