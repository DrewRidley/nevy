/// An abstract interface to interact with one or more kinds of network interface.
///
/// All network interfaces *must* provide some form of stateful connection, and a way to retrieve streams associated with those connections.
/// In the case of a simple unreliable UDP transport, a single unreliable stream can be used.
/// Endpoints must be polled via the [poll] method to progress their internal state.
pub trait Endpoint {
    /// The type of connection state this endpoint orchestrates.
    type Connection<'c>: ConnectionMut<'c>
    where
        Self: 'c;

    /// the connection id type
    ///
    /// this is required to also be hashable and cheap to copy
    /// so that it can be used reasonably in generic contexts
    type ConnectionId: std::hash::Hash + Eq + Copy;

    type ConnectInfo;

    /// Polls this endpoint and progresses its internal state.
    /// This may have a different effect on different endpoints, but generally it will also consume data from the underlying mechanism (socket).
    fn update(&mut self);

    /// Retrieves a connection reference from the endpoint given its unique id.
    fn connection<'a>(
        &'a self,
        id: Self::ConnectionId,
    ) -> Option<<Self::Connection<'a> as ConnectionMut>::NonMut>;

    /// Retrieves a connection reference mutably from the endpoint given its unique id.
    fn connection_mut<'a>(&'a mut self, id: Self::ConnectionId) -> Option<Self::Connection<'a>>;

    fn connect(&mut self, info: Self::ConnectInfo) -> Option<Self::ConnectionId>;

    /// Closes a connection matching the provided id.
    fn disconnect(&mut self, id: Self::ConnectionId) -> Result<(), ()> {
        if let Some(mut connection) = self.connection_mut(id) {
            connection.disconnect();
            Ok(())
        } else {
            Err(())
        }
    }

    fn poll_event(&mut self) -> Option<EndpointEvent<Self>>
    where
        Self: Sized;
}

/// contains all the operations that can be made with a mutable reference to connection state with a lifetime of `'c`
pub trait ConnectionMut<'c> {
    /// the non mutable reference equivalent with the same lifetime
    type NonMut: ConnectionRef<'c, Mut = Self>;

    /// get a non mutable reference with the same lifetime
    fn as_ref(&'c self) -> Self::NonMut;

    /// disconnect the client
    fn disconnect(&mut self);

    fn peer_addr(&'c self) -> std::net::SocketAddr {
        self.as_ref().peer_addr()
    }

    /// opens a stream for stream id type `S`
    fn open_stream<'s, S>(&mut self, description: S::OpenDescription) -> Option<S>
    where
        S: StreamId<'s, 'c, Self>,
        Self: Sized,
    {
        S::open(self, description)
    }

    /// get a send stream mutably with type `S`
    fn send_stream_mut<'s, S>(&'s mut self, stream_id: S) -> Option<S::SendMut>
    where
        S: StreamId<'s, 'c, Self>,
        Self: Sized,
    {
        stream_id.get_send_mut(self)
    }

    /// get a recv stream mutably with type `S`
    fn recv_stream_mut<'s, S>(&'s mut self, stream_id: S) -> Option<S::RecvMut>
    where
        S: StreamId<'s, 'c, Self>,
        Self: Sized,
    {
        stream_id.get_recv_mut(self)
    }

    /// polls stream events for stream id type `S`
    fn poll_stream_events<'s, S>(&mut self) -> Option<StreamEvent<S>>
    where
        S: StreamId<'s, 'c, Self>,
        'c: 's,
        Self: Sized,
    {
        S::poll_events(self)
    }
}

/// contains all the operations that can be made with a reference to connection state with a lifetime of `'c`
pub trait ConnectionRef<'c> {
    type Mut: ConnectionMut<'c, NonMut = Self>;

    fn peer_addr(&self) -> std::net::SocketAddr;

    /// get a send stream with type `S`
    fn send_stream<'s, S>(
        &'s self,
        stream_id: S,
    ) -> Option<<S::SendMut as SendStreamMut<'s>>::NonMut>
    where
        S: StreamId<'s, 'c, Self::Mut>,
    {
        stream_id.get_send(self)
    }

    /// get a send recv with type `S`
    fn recv_stream<'s, S>(
        &'s self,
        stream_id: S,
    ) -> Option<<S::RecvMut as RecvStreamMut<'s>>::NonMut>
    where
        S: StreamId<'s, 'c, Self::Mut>,
    {
        stream_id.get_recv(self)
    }
}

/// contains methods to operate on a stream type for mutable and immutable connection references with a lifetime of `'c`
/// by borrowing references to send and recv streams with a lifetime of `'s`
pub trait StreamId<'s, 'c: 's, C: ConnectionMut<'c>>: Sized {
    /// the type of mutable send stream reference with lifetime `'s` for `C` for this stream id
    type SendMut: SendStreamMut<'s>;
    /// the type of mutable recv stream reference with lifetime `'s` for `C` for this stream id
    type RecvMut: RecvStreamMut<'s>;

    /// description of a stream when opening
    type OpenDescription;

    /// opens a stream
    ///
    /// should fire an event consistent with the stream id returned
    fn open(connection: &mut C, description: Self::OpenDescription) -> Option<Self>;

    /// get a mutable reference to send stream with `'s` from a mutable reference to a connection with `'c`
    fn get_send_mut(self, connection: &'s mut C) -> Option<Self::SendMut>
    where
        'c: 's;

    /// get a mutable reference to recv stream with `'s` from a mutable reference to a connection with `'c`
    fn get_recv_mut(self, connection: &'s mut C) -> Option<Self::RecvMut>
    where
        'c: 's;

    /// get a reference to send stream with `'s` from a reference to a connection with `'c`
    fn get_send(
        self,
        connection: &'s C::NonMut,
    ) -> Option<<Self::SendMut as SendStreamMut<'s>>::NonMut>
    where
        'c: 's;

    /// get a reference to recv stream with `'s` from a reference to a connection with `'c`
    fn get_recv(
        self,
        connection: &'s C::NonMut,
    ) -> Option<<Self::RecvMut as RecvStreamMut<'s>>::NonMut>
    where
        'c: 's;

    /// poll the events for this stream from a mutable reference to a connection
    fn poll_events(connection: &mut C) -> Option<StreamEvent<Self>>;
}

/// contains operations for a mutable reference to a send stream with lifetime `'s`
pub trait SendStreamMut<'s> {
    type NonMut: SendStreamRef<'s>;

    type SendError;

    type CloseDescription;

    fn as_ref(&self) -> Self::NonMut;

    fn send(&mut self, data: &[u8]) -> Result<usize, Self::SendError>;

    fn close(&mut self, description: Self::CloseDescription) -> Result<(), ()>;
}

/// contains operations for a reference to a send stream with lifetime `'s`
pub trait SendStreamRef<'s> {}

/// contains operations for a mutable reference to a recv stream with lifetime `'s`
pub trait RecvStreamMut<'s> {
    type NonMut: RecvStreamRef<'s>;

    type ReadError;

    type CloseDescription;

    fn as_ref(&self) -> Self::NonMut;

    fn recv(&mut self, limit: usize) -> Result<Box<[u8]>, Self::ReadError>;

    fn close(&mut self, description: Self::CloseDescription) -> Result<(), ()>;
}

/// contains operations for a reference to a recv stream with lifetime `'s`
pub trait RecvStreamRef<'s> {}

/// events fired by endpoints
///
/// all events are with reference to connections
pub struct EndpointEvent<E: Endpoint> {
    pub connection_id: E::ConnectionId,
    pub event: ConnectionEvent,
}

/// the type of [EndpointEvent]
pub enum ConnectionEvent {
    Connected,
    Disconnected,
}

/// events fired by streams
pub struct StreamEvent<S> {
    pub stream_id: S,
    /// - `false` if the local endpoint generated the event
    /// - `true` if the peer triggered the event
    pub peer_generated: bool,
    pub event_type: StreamEventType,
}

pub enum StreamEventType {
    NewSendStream,
    ClosedSendStream,
    NewRecvStream,
    ClosedRecvStream,
}
