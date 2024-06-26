use std::marker::PhantomData;

use bevy::prelude::*;
use bevy_interface::{
    connections::StreamError, prelude::*, stream_headers::InitializeHeaderStreamError,
};
use serde::Serialize;

/// Adds message serialization functionality
pub struct MessageSerializationPlugin<C> {
    _p: PhantomData<C>,
    messages: Vec<Box<dyn MessageIdBuilder<C>>>,
}

trait MessageIdBuilder<C>: Send + Sync + 'static {
    fn build(&self, message_id: u16, app: &mut App);
}

struct MessageIdBuilderType<T> {
    _p: PhantomData<T>,
}

impl<C> MessageSerializationPlugin<C> {
    pub fn new() -> Self {
        MessageSerializationPlugin {
            _p: PhantomData,
            messages: Vec::new(),
        }
    }
}

impl<C: Component> MessageSerializationPlugin<C> {
    /// adds a message type to the plugin, assigning it the next message id
    pub fn add_message<T: Serialize + Send + Sync + 'static>(&mut self) -> &mut Self {
        self.messages
            .push(Box::new(MessageIdBuilderType::<T> { _p: PhantomData }));

        self
    }
}

impl<C: Component> Plugin for MessageSerializationPlugin<C> {
    fn build(&self, app: &mut App) {
        for (message_id, builder) in self.messages.iter().enumerate() {
            builder.build(message_id as u16, app);
        }
    }
}

impl<T: Serialize + Send + Sync + 'static, C: Component> MessageIdBuilder<C>
    for MessageIdBuilderType<T>
{
    fn build(&self, message_id: u16, app: &mut App) {
        app.insert_resource(MessageId::<C, T> {
            _p: PhantomData,
            message_id,
        });
    }
}

/// the message id for a message `T`,
/// assigned by [MessageSerializationPlugin] and stored as a resource
#[derive(Copy, Resource)]
pub struct MessageId<C, T> {
    _p: PhantomData<(C, T)>,
    message_id: u16,
}

impl<C, T> Clone for MessageId<C, T> {
    fn clone(&self) -> Self {
        MessageId {
            _p: PhantomData,
            message_id: self.message_id,
        }
    }
}

/// wraps a stream id and ensures that the message protocol isn't broken
pub struct MessageStreamState<C> {
    _p: PhantomData<C>,
    stream_id: HeaderStreamId,
    buffer: Vec<u8>,
}

#[derive(Debug)]
pub enum MessageStreamSendError {
    StreamClosed,
    MismatchedConnection(MismatchedType),
    FatalSendErr(Box<dyn StreamError>),
}

impl<C: Send + Sync + 'static> MessageStreamState<C> {
    /// creates a new stream on a connection and sets up state for sending messages on that stream
    ///
    /// the same connection should be used for all further operations
    pub fn new(
        connection: &mut BevyConnectionMut,
        description: Description,
        header: u16,
    ) -> Result<Option<Self>, MismatchedType> {
        let Some(stream_id) = HeaderStreamId::new(connection, description, header)? else {
            return Ok(None);
        };

        Ok(Some(MessageStreamState {
            _p: PhantomData,
            stream_id,
            buffer: Vec::new(),
        }))
    }

    /// writes as much of the internal buffer as possible
    ///
    /// returns `true` if all data was successfully writen and another message will be accepted
    pub fn flush(
        &mut self,
        connection: &mut BevyConnectionMut,
    ) -> Result<bool, MessageStreamSendError> {
        // get the stream id
        let Some(stream_id) = self
            .stream_id
            .poll_ready(connection)
            .map_err(|err| match err {
                InitializeHeaderStreamError::StreamClosedPrematurly => {
                    MessageStreamSendError::StreamClosed
                }
                InitializeHeaderStreamError::MismatchedConnection(err) => {
                    MessageStreamSendError::MismatchedConnection(err)
                }
                InitializeHeaderStreamError::FatalSendErr(err) => {
                    MessageStreamSendError::FatalSendErr(err)
                }
            })?
        else {
            // header hasn't been sent yet
            return Ok(false);
        };

        // header has been written, get the stream
        let Some(mut stream) = connection
            .send_stream(stream_id)
            .map_err(|err| MessageStreamSendError::MismatchedConnection(err))?
        else {
            return Err(MessageStreamSendError::StreamClosed);
        };

        // write as much data as possible
        loop {
            if self.buffer.is_empty() {
                break Ok(true);
            }

            match stream.send(&self.buffer) {
                Err(err) => {
                    if err.is_fatal() {
                        return Err(MessageStreamSendError::FatalSendErr(err));
                    }

                    // writing is blocked
                    break Ok(false);
                }
                Ok(bytes) => {
                    self.buffer.drain(..bytes);
                }
            }
        }
    }

    /// returns `true` if a new message will be accepted
    pub fn ready(&self) -> bool {
        self.buffer.is_empty()
    }

    /// attempts to send a message
    ///
    /// will return `true` if the message was accepted,
    /// but it may not have been written completely.
    /// use [ready](MessageStreamState::ready) to check if
    /// another message will be accepted
    pub fn send<T: Serialize + Send + Sync + 'static>(
        &mut self,
        connection: &mut BevyConnectionMut,
        message_id: MessageId<C, T>,
        message: &T,
    ) -> Result<bool, MessageStreamSendError> {
        if !self.ready() {
            if !self.flush(connection)? {
                return Ok(false);
            }
        }

        let message_id = message_id.message_id.to_be_bytes();
        let bytes = bincode::serialize(message).expect("Failed to serialize message");
        let message_length = (bytes.len() as u16).to_be_bytes();

        self.buffer.extend(message_id);
        self.buffer.extend(message_length);
        self.buffer.extend(bytes);

        self.flush(connection)?;

        Ok(true)
    }

    /// cancels message writing and returns the stream id
    ///
    /// cancels message header writing if it hasn't been completed
    ///
    /// typically would only be used for closing the stream
    pub fn end(self) -> BevyStreamId {
        self.stream_id.end()
    }
}
