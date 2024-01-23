//! Contains the neccessary types and systems to synchronize networked state.
//! 
//! Primarily, this module exists as an interface to the component streaming protocols defined in
//! [crate::policy].
//! 
//! In a vast majority of use cases, adding [NetStreamPlugin] will enable all of the basic functionality.
//! 
//! For more complex use cases, investigate building your own systems using the [NetStreamBuilder].
//! This builder will facilitate the construction of custom streaming protocols and behavior.
use std::marker::PhantomData;
use bevy::{ecs::schedule::ScheduleLabel, prelude::*};
use crate::ClientId;

#[cfg(feature = "rkyv")]
use rkyv::{Archive, Serialize, Deserialize};


#[cfg(feature = "serde")]
fn register_component_serializer<C: Component + serde::Serialize>() {

} 

#[cfg(feature = "rkyv")]
fn register_component_serializer<C: Component + rkyv::Serialize>() {

}


/// A plugin containing all of the neccessary systems to synchronize networked state.
/// This plugin will monitor and dispatch appropiate component changes to the connected peers.
pub struct NetStreamPlugin {}
impl Plugin for NetStreamPlugin {
    fn build(&self, app: &mut App) {
       
    }
}

/// This type is incomplete, but will eventually be used to facilitate the development of custom
/// streaming protocols.
pub struct NetStreamBuilder;

/// A marker trait to indicate which client 'owns' this entity.
/// Does not have to be exclusive. A single client can own multiple entities.
/// In this context, 'owns' does not refer to authority, but rather, 
/// which client should receive a particular piece of state.
/// 
/// # Type Parameter
/// `I`: The type of the client identifier.
#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct Owner<I: ClientId>(pub I);

/// A marker trait to indicate that a particular client is Authoritative over the state of this entity.
///
/// A client will dispatch state updates to other clients if it is 'self-authoritative'.
/// Clients will reject state updates from non-authoritative clients (if a p2p transport layer is used).
/// This marker is not used if there is a single authoritative server.
/// A custom consensus mechanism may be required, depending on the game requirements.
/// 
/// # Type Parameter
/// `I`: The type of the client identifier.
#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct Authoritative<I: ClientId>(pub I);

/// A marker component used to indicate that a particular component shall be synchronized.
/// All entities that are synchronized should contain at least one component associated with this marker.
/// 
/// It is possible to have [NetComponent] without [crate::policy::ComponentPolicy].
/// It is invalid to add an adjacent [crate::policy::ComponentPolicy] afterwards.
/// If you wish to dynamically control the insertion and removal of a component, you must 
/// Have a policy present at the time it is spawned.
#[derive(Component)]
pub struct NetComponent<
// If using serde, the component must implement [serde::Serialize] and [serde::Deserialize].
#[cfg(feature = "serde")] C: Component + serde::Serialize,
// If using rkyv as the serialization mechanism, it must implement the associated methods.
#[cfg(feature = "rkyv")] C: Component + rkyv::Archive + rkyv::Serialize + rkyv::Deserialize,
// Fallback to standard trait bounds. TODO: Require a custom trait impl in this case.
#[cfg(all(not(feature = "serde"), not(feature = "rkyv")))] C: Component,
>(PhantomData<C>);

/// A marker to indicate that the entity is networked.
/// This will likely rarely be used but exists if you need your logic to be aware of the networked status of an entity.
/// This is shorthand for checking each of the many components for an adjacent [NetComponent].
/// Due to its sparse nature, its only recommended to use if you require absolute proof of networked state.
#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct NetEntity;