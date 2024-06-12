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

    type IncomingConnectionInfo<'i>;

    /// Polls this endpoint and progresses its internal state.
    /// This may have a different effect on different endpoints, but generally it will also consume data from the underlying mechanism (socket).
    fn update(&mut self, handler: &mut impl EndpointEventHandler<Self>);

    /// Retrieves a connection reference from the endpoint given its unique id.
    fn connection<'c>(
        &'c self,
        id: Self::ConnectionId,
    ) -> Option<<Self::Connection<'c> as ConnectionMut>::NonMut<'c>>;

    /// Retrieves a connection reference mutably from the endpoint given its unique id.
    fn connection_mut<'a>(&'a mut self, id: Self::ConnectionId) -> Option<Self::Connection<'a>>;

    /// creates a connection described by `info`
    fn connect<'c>(
        &'c mut self,
        info: Self::ConnectInfo,
    ) -> Option<(Self::ConnectionId, Self::Connection<'c>)>;

    /// Closes a connection matching the provided id.
    fn disconnect(&mut self, id: Self::ConnectionId) -> Result<(), ()> {
        if let Some(mut connection) = self.connection_mut(id) {
            connection.disconnect();
            Ok(())
        } else {
            Err(())
        }
    }
}

/// implement this trait on a type to handle events when updating an [Endpoint]
///
/// make sure to override the [connection_request](EndpointEventHandler::connection_request)
/// default implementation or incoming connections will always be rejected by default
pub trait EndpointEventHandler<E: Endpoint>
where
    E: ?Sized,
{
    /// callback to query if an incoming connection should be accepted
    ///
    /// return `true` to accept
    ///
    /// this method could be called multiple for the same connection,
    /// say if multiple verification stages are needed,
    /// depending on the specific [Endpoint] implementation
    #[allow(unused_variables)]
    fn connection_request<'i>(&mut self, request: E::IncomingConnectionInfo<'i>) -> bool {
        false
    }

    fn connected(&mut self, connection_id: E::ConnectionId);

    fn disconnected(&mut self, connection_id: E::ConnectionId);
}

/// contains all the operations that can be made with a mutable reference to connection state with a lifetime of `'c`
pub trait ConnectionMut<'c> {
    /// the non mutable reference equivalent with a borrowed lifetime shorter that `'c`
    type NonMut<'b>: ConnectionRef<'b>
    where
        Self: 'b;

    type StreamType: StreamId<Connection<'c> = Self> + 'c;

    /// get a non mutable reference with the same lifetime
    fn as_ref<'b>(&'b self) -> Self::NonMut<'b>;

    /// disconnect the client
    fn disconnect(&mut self);

    fn get_stats<'b>(&'b self) -> <Self::NonMut<'b> as ConnectionRef<'b>>::ConnectionStats {
        self.as_ref().get_stats()
    }

    /// opens a stream for stream id type `S`
    fn open_stream<S>(&mut self, description: S::OpenDescription) -> Option<S>
    where
        S: StreamId<Connection<'c> = Self>,
        S: 'c,
    {
        S::open(self, description)
    }

    /// get a send stream mutably with type `S`
    fn send_stream<'s>(
        &'s mut self,
        stream_id: Self::StreamType,
    ) -> Option<<Self::StreamType as StreamId>::SendMut<'s>> {
        stream_id.get_send(self)
    }

    /// get a recv stream mutably with type `S`
    fn recv_stream<'s>(
        &'s mut self,
        stream_id: Self::StreamType,
    ) -> Option<<Self::StreamType as StreamId>::RecvMut<'s>> {
        stream_id.get_recv(self)
    }

    /// polls stream events for stream id type `S`
    fn poll_stream_events(&mut self) -> Option<StreamEvent<Self::StreamType>> {
        Self::StreamType::poll_events(self)
    }
}

/// contains all the operations that can be made with a reference to connection state with a lifetime of `'c`
pub trait ConnectionRef<'c> {
    type ConnectionStats: std::fmt::Debug;

    fn get_stats(&self) -> Self::ConnectionStats;
}

/// contains methods to operate on a stream type
pub trait StreamId {
    /// the mutable connection reference of this stream id
    type Connection<'c>: ConnectionMut<'c>
    where
        Self: 'c;

    /// the mutable send stream reference of this stream id
    type SendMut<'s>: SendStreamMut<'s>
    where
        Self: 's;

    /// the mutable recv stream reference of this stream id
    type RecvMut<'s>: RecvStreamMut<'s>
    where
        Self: 's;

    /// description of a stream when opening
    type OpenDescription;

    /// opens a stream
    ///
    /// should fire an event consistent with the stream id returned
    fn open<'c>(
        connection: &mut Self::Connection<'c>,
        description: Self::OpenDescription,
    ) -> Option<Self>
    where
        Self: Sized;

    /// get a `'s` mutable reference to a send stream with from a `'c` mutable reference to a connection
    fn get_send<'c, 's>(
        self,
        connection: &'s mut Self::Connection<'c>,
    ) -> Option<Self::SendMut<'s>>;

    /// get a `'s` mutable reference to a recv stream with from a `'c` mutable reference to a connection
    fn get_recv<'c, 's>(
        self,
        connection: &'s mut Self::Connection<'c>,
    ) -> Option<Self::RecvMut<'s>>;

    /// poll the events for this stream from a mutable reference to a connection
    fn poll_events<'c>(connection: &mut Self::Connection<'c>) -> Option<StreamEvent<Self>>
    where
        Self: Sized;
}

/// contains operations for a mutable reference to a send stream with lifetime `'s`
pub trait SendStreamMut<'s> {
    type SendError;

    type CloseDescription;

    fn send(&mut self, data: &[u8]) -> Result<usize, Self::SendError>;

    fn close(&mut self, description: Self::CloseDescription) -> Result<(), ()>;
}

/// contains operations for a mutable reference to a recv stream with lifetime `'s`
pub trait RecvStreamMut<'s> {
    type ReadError;

    type CloseDescription;

    fn recv(&mut self, limit: usize) -> Result<Box<[u8]>, Self::ReadError>;

    fn close(&mut self, description: Self::CloseDescription) -> Result<(), ()>;
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
