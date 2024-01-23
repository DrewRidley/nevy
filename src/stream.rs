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
use bevy::{ecs::{archetype::{Archetype, ArchetypeId}, component::ComponentId}, prelude::*, utils::HashMap};
use indexmap::IndexMap;
use num_traits::Num;
use rkyv::Deserialize;
use smallvec::SmallVec;
use crate::{policy::EntityRingPolicy, ClientId};

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

/// A cache that contains a list of archetypes that need to be visited by the network dispatcher.
/// 
/// A networked archetype is an archetype with one or more [NetComponent].
#[derive(Resource)]
pub struct NetArchetypes {
    entries: SmallVec<[ArchetypeId; 16]>
}


/// Receives a valid archetype, and writes relevant changes to the underlying buffer.
/// This assumes the archetype has already been confirmed to align to the policy requirements.
fn buffer_archetype_changes(
    world: &World,
    archetype: &Archetype,
) {
    for ent in archetype.entities().iter() {
        
    }
}

/// Streams all networked archetypes that contain policy 'P'.
/// 
/// Assumes that this system has already been filtered by the run criteria imposed by 'P'.
pub fn net_stream_sys<N: 'static + Num + Send + Sync, P: 'static + Send + Sync>(
    world: &World,
    net_archetypes: Res<NetArchetypes>,
) {
    for relevant_archetype in net_archetypes.entries.iter() {
        match world.archetypes().get(*relevant_archetype) {
            Some(ecs_archetype) => { 
                //The archetypes must be filtered to only include ones with the matching policy.
                //Non-policy driven entities must be handled by a separate system.
                let policy = world.component_id::<EntityRingPolicy<N, P>>();
                if let Some(_) = policy {
                    buffer_archetype_changes(world, ecs_archetype);
                }
            }
            None => {
                panic!("Archetype {:?} was included in the list of streamed archetypes, but does not exist!.", relevant_archetype);
            }
        }
    }
}