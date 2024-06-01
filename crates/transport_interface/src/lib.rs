/// An abstract interface to interact with one or more kinds of network interface.
///
/// All network interfaces *must* provide some form of stateful connection, and a way to retrieve streams associated with those connections.
/// In the case of a simple unreliable UDP transport, a single unreliable stream can be used.
/// Endpoints must be polled via the [poll] method to progress their internal state.
pub trait Endpoint {
    /// The context required to perform operations on this endpoint.
    ///
    /// This will be used by Endpoint implementations to fire events or pass data back to the pollee.
    type Context;

    /// The type of connection state this endpoint orchestrates.
    type Connection: Connection<Context = Self::Context>;

    type ConnectInfo;

    /// Polls this endpoint and progresses its internal state.
    /// This may have a different effect on different endpoints, but generally it will also consume data from the underlying mechanism (socket).
    fn update(&mut self, context: &mut Self::Context);

    /// Retrieves a connection from the endpoint given its unique id.
    fn connection(
        &self,
        id: ConnectionId<Self>,
        context: &Self::Context,
    ) -> Option<&Self::Connection>;

    /// Retrieves a mutably reference to a connection given its unique id.
    fn connection_mut(
        &mut self,
        id: ConnectionId<Self>,
        context: &mut Self::Context,
    ) -> Option<&mut Self::Connection>;

    fn connect(
        &mut self,
        context: &mut Self::Context,
        info: Self::ConnectInfo,
    ) -> Option<ConnectionId<Self>>;

    /// Closes a connection matching the provided id.
    fn disconnect(
        &mut self,
        id: ConnectionId<Self>,
        context: &mut Self::Context,
    ) -> Result<(), ()> {
        if let Some(connection) = self.connection_mut(id, context) {
            connection.disconnect(context);
            Ok(())
        } else {
            Err(())
        }
    }

    fn poll_event(&mut self, context: &mut Self::Context) -> Option<EndpointEvent<Self>>
    where
        Self: Sized;
}

/// shorthand for the [Connection] id for some [Endpoint]
pub type ConnectionId<E> = <<E as Endpoint>::Connection as Connection>::Id;

/// An abstract interface for any 'connection' which has the ability to send and/or receive data via a
/// [SendStream], [BufferedSendStream] or [ReceiveStream]. See [StreamId] for more information on how to differentiate streams.
pub trait Connection {
    type Context;

    type Id;

    fn disconnect(&mut self, context: &mut Self::Context);

    fn send_stream<S>(&self, stream_id: S, context: &Self::Context) -> Option<&S::Stream>
    where
        S: StreamId,
        S::Stream: SendStream;

    fn send_stream_mut<S>(
        &mut self,
        stream_id: S,
        context: &mut Self::Context,
    ) -> Option<&mut S::Stream>
    where
        S: StreamId,
        S::Stream: SendStream;

    fn recv_stream<S>(&self, stream_id: S, context: &Self::Context) -> Option<&S::Stream>
    where
        S: StreamId,
        S::Stream: RecvStream;

    fn recv_stream_mut<S>(
        &mut self,
        stream_id: S,
        context: &mut Self::Context,
    ) -> Option<&mut S::Stream>
    where
        S: StreamId,
        S::Stream: RecvStream;

    fn poll_stream_event<S>(&mut self, context: &mut Self::Context) -> Option<StreamEvent<S>>
    where
        S: StreamId;
}

/// A stream capable of internally buffering data before transmitting.
///
/// This is an extension of [SendStream]
pub trait BufferedSendStream {
    fn buffer_len(&self) -> usize;
}

/// A stream to immediately send data to a given peer associated with a [Connection].
///
/// Specific endpoints *may* associate one [Connection] or [SendStream] with multiple peers, depending on the architecture.
/// If timeliness is not important, consider using [BufferedSendStream].
pub trait SendStream {
    type SendError;

    fn send(&mut self, data: &[u8]) -> Result<usize, Self::SendError>;
}

/// A stream to receive a potential chunk of data from a [Connection].
///
/// Returns [None] if there is no data available to read.
pub trait RecvStream {
    fn recv(&mut self, limit: usize) -> Option<&[u8]>;
}

/// A type used to identify streams.
/// The associated type [StreamId::Stream] is the type of stream this is an identifier for.
///
/// Example:
///
///```rust
/// struct Stream(usize);
///
/// impl StreamId for Stream {
///     type Stream =  SendStream;
/// }
///
/// struct DatagramStream;
/// impl StreamId for DatagramStream {
///     type Stream = BufferedSendStream;
/// }
/// ```
pub trait StreamId {
    type Stream;
}

pub struct EndpointEvent<E: Endpoint> {
    pub connection_id: ConnectionId<E>,
    pub event: ConnectionEvent,
}

pub enum ConnectionEvent {
    Connected,
    Disconnected,
}

pub enum StreamEvent<S: StreamId> {
    NewStream(S),
    ClosedStream(S),
}

/// A convienience wrapper over an [Endpoint] that abstracts away the complexity of passing [EndpointRef::context] to all calls.
pub struct EndpointRef<'a, E: Endpoint> {
    pub context: &'a E::Context,
    pub endpoint: &'a E,
}

/// A convienience wrapper over an [Endpoint] that abstracts away the complexity of passing [EndpointRefMut::context] to all calls.
///
/// Provides mutable aliasing to make changes to the endpoint.
pub struct EndpointRefMut<'a, E: Endpoint> {
    pub context: &'a mut E::Context,
    pub endpoint: &'a mut E,
}

/// A convienience wrapper over an [Connection] that abstracts away the complexity of passing [ConnectionRef::context] to all calls.
pub struct ConnectionRef<'a, C: Connection> {
    pub context: &'a C::Context,
    pub connection: &'a C,
}

/// A convienience wrapper over an [Connection] that abstracts away the complexity of passing [ConnectionRefMut::context] to all calls.
///
/// Provides mutable aliasing to make changes to the connection.
pub struct ConnectionRefMut<'a, C: Connection> {
    pub context: &'a mut C::Context,
    pub connection: &'a mut C,
}

impl<'a, E: Endpoint> EndpointRef<'a, E> {
    /// Retrieves a [ConnectionRef] associated with the given connection.
    ///
    /// Returns [None] if there is no connection with the specified Id.
    pub fn connection(&self, id: ConnectionId<E>) -> Option<ConnectionRef<E::Connection>> {
        self.endpoint
            .connection(id, self.context)
            .map(|connection| ConnectionRef {
                connection,
                context: self.context,
            })
    }
}

impl<'a, E: Endpoint> EndpointRefMut<'a, E> {
    pub fn update(&mut self) {
        self.endpoint.update(self.context);
    }

    /// Retrieves a [ConnectionRef] associated with the given connection.
    ///
    /// Returns [None] if there is no connection with the specified Id.
    pub fn connection(&self, id: ConnectionId<E>) -> Option<ConnectionRef<E::Connection>> {
        self.endpoint
            .connection(id, self.context)
            .map(|connection| ConnectionRef {
                connection,
                context: self.context,
            })
    }

    /// Retrieves a [ConnectionRefMut] associated with the given connection.
    ///
    /// Returns [None] if there is no connection with the specified Id.
    pub fn connection_mut(
        &mut self,
        id: ConnectionId<E>,
    ) -> Option<ConnectionRefMut<E::Connection>> {
        self.endpoint
            .connection_mut(id, self.context)
            .map(|connection| ConnectionRefMut {
                connection,
                context: self.context,
            })
    }

    pub fn poll_event(&mut self) -> Option<EndpointEvent<E>> {
        self.endpoint.poll_event(self.context)
    }
}

impl<'a, E: Endpoint> From<EndpointRefMut<'a, E>> for EndpointRef<'a, E> {
    fn from(ref_mut: EndpointRefMut<'a, E>) -> EndpointRef<'a, E> {
        EndpointRef {
            context: ref_mut.context,
            endpoint: ref_mut.endpoint,
        }
    }
}

impl<'a, C: Connection> ConnectionRef<'a, C> {
    /// Retrieves a [SendStream] associated with the provided [StreamId].
    ///
    /// Returns [None] if the stream does not exist.
    pub fn send_stream<S>(&self, stream_id: S) -> Option<&S::Stream>
    where
        S: StreamId,
        S::Stream: SendStream,
    {
        self.connection.send_stream(stream_id, self.context)
    }

    pub fn recv_stream<S>(&self, stream_id: S) -> Option<&S::Stream>
    where
        S: StreamId,
        S::Stream: RecvStream,
    {
        self.connection.recv_stream(stream_id, self.context)
    }
}

impl<'a, C: Connection> ConnectionRefMut<'a, C> {
    /// Retrieves a [SendStream] reference associated with the provided [StreamId].
    ///
    /// Returns [None] if the stream does not exist.
    pub fn send_stream<S>(&self, stream_id: S) -> Option<&S::Stream>
    where
        S: StreamId,
        S::Stream: SendStream,
    {
        self.connection.send_stream(stream_id, self.context)
    }

    /// Retrieves a &mut [SendStream] associated with the provided [StreamId].
    ///
    /// Returns [None] if the stream does not exist.
    pub fn send_stream_mut<S>(&mut self, stream_id: S) -> Option<&mut S::Stream>
    where
        S: StreamId,
        S::Stream: SendStream,
    {
        self.connection.send_stream_mut(stream_id, self.context)
    }

    pub fn recv_stream<S>(&self, stream_id: S) -> Option<&S::Stream>
    where
        S: StreamId,
        S::Stream: RecvStream,
    {
        self.connection.recv_stream(stream_id, self.context)
    }

    pub fn recv_stream_mut<S>(&mut self, stream_id: S) -> Option<&mut S::Stream>
    where
        S: StreamId,
        S::Stream: RecvStream,
    {
        self.connection.recv_stream_mut(stream_id, self.context)
    }
}

impl<'a, C: Connection> From<ConnectionRefMut<'a, C>> for ConnectionRef<'a, C> {
    fn from(ref_mut: ConnectionRefMut<'a, C>) -> ConnectionRef<'a, C> {
        ConnectionRef {
            context: ref_mut.context,
            connection: ref_mut.connection,
        }
    }
}
