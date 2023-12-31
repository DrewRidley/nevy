use std::marker::PhantomData;
use bevy::{prelude::*, utils::HashSet};

///Contains the latest state received from the server.\
///May be out of date depending on reliability strategy selected in the corresponding [NetPolicy] component.
pub struct ServerState<T>(T);

///Holds the differential state to reduce bandwidth usage.
///If the corresponding [NetPolicy] component allows, duplicate state will be held and compared when changes are detected.
pub struct NetDiff<T>(T);


/// A reliability guarantee that a particular component may require.
pub enum SyncReliability {
    /// There are zero guarantees about the deliver or order of updates applied.
    Unreliable,
    /// There are no guarantees about delivery of updates, but they will be applied in order. 
    /// If update 'n + 1' is received, update 'n' will be discarded, even if it arrives later.
    UnreliableSequenced,
    /// Updates are guaranteed to be delivered, but the order they are applied is not guaranteed.
    Reliable,
    /// Updates are guaranteed to be delivered and applied in order.
    ReliableOrdered,
}

/// The specific networking behavior contracted for a particular component's synchronization.
pub struct SyncStrategy {
    /// The reliabiltiy guarantees this component requires.
    pub reliability: SyncReliability,

    /// Whether this component should have diffed updates.
    /// If true, only the delta between the previous and current state will be sent over the network.
    /// If false, the entire state will be sent over the network when a change occurs.
    /// Diffing does have a slight performance and memory cost, but can significantly reduce bandwidth usage for large components.
    pub diff: bool
}

#[derive(PartialEq, Eq, Hash)]
pub struct ClientId(u32);

/// The default [PeerPolicy].
/// Allows peers to be whitelisted or blacklisted.
pub enum PeerList {
    /// All peers will receive component updates.
    All,
    AllExcept(HashSet<ClientId>),
    /// Only these specific clients will receive updates about this component.
    Specific(HashSet<ClientId>),
    /// This component will not be synchronized.
    None,
}

pub struct AllPeers;
pub struct PeersExcept(HashSet<ClientId>);
pub struct SpecificPeers(HashSet<ClientId>);
pub struct NoPeers;

impl PeerPolicy for AllPeers {
    fn should_receive(&self, _client_id: ClientId) -> bool {
        true
    }
}

impl PeerPolicy for PeersExcept {
    fn should_receive(&self, client_id: ClientId) -> bool {
        !self.0.contains(&client_id)
    }
}

impl PeerPolicy for SpecificPeers {
    fn should_receive(&self, client_id: ClientId) -> bool {
        self.0.contains(&client_id)
    }
}

impl PeerPolicy for NoPeers {
    fn should_receive(&self, _client_id: ClientId) -> bool {
        false
    }
}


/// A trait to be implemented by any type that wishes to make determinations about which peers should receive a given component update.
pub trait PeerPolicy {
    fn should_receive(&self, client_id: ClientId) -> bool;
}


/// A component marking which components on an entity should be networked, and how.
/// This component contains a [SyncStrategy] which determines how this component will be synchronized.
/// This component also contains a [PeerPolicy] which determines which peers will receive updates about this component.
/// The peer policy defaults to [AllPeers] if not specified.
#[derive(Component)]
pub struct NetSyncPolicy<T: Component, P: PeerPolicy = AllPeers> {
    ///The strategy used to synchronize this particular component.
    pub strategy: SyncStrategy,

    /// Which peers should receive updates about this particular component.
    /// Primarily useful with your own logic that dynamically changes the peers according to relevance.
    pub peer_policy: P,

    ///The component to be synchronized.
    phantom: PhantomData<T>,
}

pub struct NetMessagePolicy {

}

pub trait NetMessage {

}



/// A trait to be implemented on any bundles which will be sent over the network.
/// This can be used to reduce boilerplate to spawn entities on the server.
pub trait NetBundle {
    /// Responsible for registering all of the client systems for the components present in the bundle.
    type ClientPlugin: Plugin + Default;

    /// Responsible for registering all of the server systems for the components present in the bundle.
    type ServerPlugin: Plugin + Default;

    /// Contains ONLY the components that exist on the server. This is used to spawn the initial bundle on the server.
    type ServerBundle: Bundle;
}

pub mod client;
pub mod server;

pub mod prelude {
    pub use crate::client;
    pub use crate::server;
}
