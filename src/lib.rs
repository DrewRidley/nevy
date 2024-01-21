//! [![](https://nevy_logo.svg)](https://github.com/DrewRidley/nevy)
//!
//! Nevy is a versatile, open sourced networking library for the [Bevy game engine](https://bevyengine.org).
//! It is designed to be easy to use, flexible, and performant.
//!
//!
//! ## Example
//!
//! Here is an example to illustrate how easy it is to get started writing a multiplayer game:
//!
//! ### Server:
//! ```
//! use bevy::prelude::*;
//! use nevy::prelude::*;
//! 
//! #[derive(Component, NetComp)]
//! struct Health(u8);
//! 
//! #[derive(Component, NetComp)]
//! struct Stamina(u16);
//!
//! fn main() {
//!    App::new()
//!        .add_systems(Startup, startup_server_sys)
//!        .run();
//! }
//! 
//! fn startup_server_sys(mut cmds: Commands) {
//!     // Spawns an entity with Health starting at 8.
//!    cmds.spawn_networked((Health(8), Stamina(100)));
//! 
//!    // Spawn an entity whose owner is ClientId(0). This entity will only exist on the server and client 0.
//!    cmds.spawn_networked((Health(15), Stamina(100), Owner(0))).with_policy::<u16>(ClientPolicy(Owner));
//! 
//!    //Spawns an entity whose owner ClientId(0). All players can view the stamina component,
//!    //but only the owner can view their own health.
//!    cmds.spawn_networked((Health(8), NetSync<Health>(ClientPolicy(Owner), Stamina(100))));
//! }
//!
//! ```
use std::{marker::PhantomData, any::TypeId, hash::Hash, io::Write};
use bevy::{prelude::*, ecs::{archetype::ArchetypeId, component::{ComponentId, self}, system::SystemChangeTick}, utils::{HashMap, HashSet, EntityHash}, ptr::Ptr};
use bincode::DefaultOptions;
use indexmap::IndexMap;
use serde::{Serializer, Serialize};
use smallvec::SmallVec;
mod serialize;

/// A marker trait indicating that a given type can be used as a client identifier.
/// This will be auto-implemented on all types that implement [Hash], [Send] and [Sync].
/// [Send] and [Sync] are required because the underlying network buffer is potentially shared between threads.
pub trait ClientId: Hash + Eq + Send + Sync { }

//Implement ClientId for all hashable types.
impl<T: Hash + Eq + Send + Sync> ClientId for T { }

/// A policy dictating which clients shall receive a particular piece of state.
/// 
/// Particularly useful in games where some information must be hidden from the players.
/// For example, in a PvP game, you might want to hide the health of the enemy team to a player.
/// It is possible to set a single policy for an entire entity with [EntityRef::set_net_policy].
/// 
/// # Type Parameter
/// `I`: The type of the client identifier.
#[derive(Hash)]
pub enum ClientPolicy<I:  Hash> {
    /// Synchronizes this state with all clients.
    All,
    /// Synchronizes this state with all EXCEPT the given clients.
    Exclude(SmallVec<[I; 32]>),
    /// Synchronizes this state with only the given clients.
    // Here, we avoid inlining because in most cases the inclusionary set will be quite large.
    Include(Vec<I>),
    /// Synchronizes this state with only the given client.
    One(I),
    /// Synchronize this unit of state exclusively with the owner.
    /// This will not synchronize if the entity does not have a marked client as the owner.
    Owner,
    /// Disable synchronization of this component (temporarily).
    None
}

/// A marker trait to indicate which client 'owns' this entity.
/// Does not have to be exclusive. A single client can own multiple entities.
/// In this context, 'owns' does not refer to authority, but rather, 
/// which client should receive a particular piece of state.
#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct Owner<I: ClientId>(pub I);

/// A marker trait to indicate that a particular client is Authoritative over the state of this entity.
///
/// A client will dispatch state updates to other clients if it is 'self-authoritative'.
/// Clients will reject state updates from non-authoritative clients (if a p2p transport layer is used).
/// This marker is not used if there is a single authoritative server.
/// A custom consensus mechanism may be required, depending on the game requirements.
#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct Authoritative<I: ClientId>(pub I);

/// Contains a list of each unique pair of (ClientId, Component) on this entity.
/// 
/// This is used to build unique component permutations for each Client's specific message requirements.
/// It optimizes by grouping similar component policies to reduce message fragmentation.
/// Thus, it is recommended where possible to have as few unique policies as your game logic permits.
#[derive(Component)]
#[component(storage = "SparseSet")]
struct EntityPolicyCache<I: Hash> {
    //Contains a list of policies and their corresponding components.
    entries: IndexMap<ClientPolicy<I>, SmallVec<[ComponentId; 6]>>
}

impl<I: Hash> IntoIterator for EntityPolicyCache<I> {
    type Item = (ClientPolicy<I>, SmallVec<[ComponentId; 6]>);
    type IntoIter = indexmap::map::IntoIter<ClientPolicy<I>, SmallVec<[ComponentId; 6]>>;
    fn into_iter(self) -> Self::IntoIter {
        self.entries.into_iter()
    }
}

impl<I: Default + Hash> Default for ClientPolicy<I> {
    fn default() -> Self {
        ClientPolicy::All
    }
}

/// A marker to indicate that a given component is networked for this entity.
/// This marker cannot be removed or inserted at runtime.
/// For dynamic networked components, use [DynNetComp].
/// 
/// # Type Parameter
/// `T`: The type of the component being networked.
#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct NetComp<I: Hash, T>(pub ClientPolicy<I>, PhantomData<T>);

/// A marker to indicate that a given component is networked for this entity.
/// This marker can be removed or inserted at runtime.
/// This marker incurs an additional, fixed, 1 byte overhead per entity update.
/// Additional instances of [DynNetComp] on a given entity do not incur any overhead.
/// For static networked components, use [NetComp].
/// 
/// # Type Parameter
/// `T`: The type of the component being networked.
/// 'I': The type of the identifier.
#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct DynNetComp<I: Hash, T>(pub ClientPolicy<I>, PhantomData<T>);

/// A marker to indicate that the entity is networked.
/// This will likely rarely be used but exists if you need your logic to be aware of the networked status of an entity.
/// This is shorthand for checking each of the many components for an adjacent [NetComp]/[DynNetComp].
/// Due to its sparse nature, its only recommended to use if you require absolute proof of networked state.
#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct NetEntity;


/// Represents an archetype in a networked system.
///
/// `NetArchetype` is used to manage a collection of components that are networked together.
/// Each component in this archetype is identified by a `ComponentId` and contains data of type `F`.
///
/// # Type Parameter
/// `S`: The type of the underlying serializer/deserializer for this archetype.
/// Usually, you will have two NetArchetypes, one for serialization and one for deserialization
struct NetArchetype<S: Serializer> {
    // The bevy [ArchetypeId] being tracked by this structure.
    id: ArchetypeId,

    // All of the components and their associated serializers.
    /// 
    /// This structure contains the (ComponentId, # of entities with component in this archetype, serializer).
    components: SmallVec::<[(ComponentId, u32, fn(EntityRef, S)); 5]>,
}

impl<S: Serializer> NetArchetype<S> where S: Serializer {
    /// Will register the component [T] and its associated function.
    /// 
    /// # Parameters
    /// `world`: The [World] to register this component with.
    //  'ser': The function to use to serialize this component.

    fn register_usage<T: Component + Serialize>(&mut self, world: &World, ser: S) {
        if let Some(comp_id) = world.components().get_id(TypeId::of::<T>()) {
            //This comp should be registered.
            if !self.components.iter().any(|(id, _ , _)| *id == comp_id) {
                //This comp is not registered, so register it.
                self.components.push((comp_id, 0, |ent: EntityRef, ser| {
                    //Unwrap is safe here because access is already gated.
                    ent.get::<T>().unwrap().serialize(ser).expect("Failed to serialize");
                }));

                //Sort the components for consistent client|server ordering.
                self.components.sort();

                return;
            }

            //This comp is already registered, so increment the usage count.
            if let Some((_, count, _)) = self.components.iter_mut().find(|(id, _, _)| *id == comp_id) {
                *count += 1;
            }
        }
    }

    /// Will remove any stale components on this archetype if no entities contain the associated [NetComp] component.
    /// 
    /// This is neccessary because if no entities
    /// 
    /// # Parameters
    /// `id`: The [ComponentId] of the component to cleanup.
    /// 
    /// # Returns
    /// 'true' if this archetype itself is stale and should be deregistered.
    fn cleanup_component(&mut self, id: ComponentId) -> bool {
        if let Some((_, count, _)) = self.components.iter_mut().find(|(comp_id, _ , _)| *comp_id == id) {
            *count -= 1;

            //If none of this archetype's entities contain this component, remove it from being synced.
            if (*count) == 0 {
                self.components.retain(|(comp_id, _, _)| *comp_id != id);
            }
        }

        //If this archetype has no components, it is stale and should be removed.   
        return self.components.is_empty();
    }
}


/// A collection of tracked archetypes and their components.
/// 
/// 'NetArchetypes' contains all of the archetypes that track one or more entities with one or more [NetComp] components.
/// 
/// # Type Parameter
/// `F`: The type of the underlying function for this NetArchetype.
/// Usually, you will have one archetype for serialization.
/// Additional archetypes may be registered for custom behavior of specific components.
/// Deserialization should be done with a SparseSet table of components
#[derive(Resource)]
struct NetArchetypes<S: Serializer> {
    archetypes: Vec<NetArchetype<S>>
}

impl<S: Serializer> IntoIterator for NetArchetypes<S> {
    type Item = NetArchetype<S>;
    type IntoIter = std::vec::IntoIter<NetArchetype<S>>;
    fn into_iter(self) -> Self::IntoIter {
        self.archetypes.into_iter()
    }
}

/// A collection of client IDs and their associated network buffers.
/// Write to the underlying map to send data to clients.
#[derive(Resource, Default)]
struct NetBuffer<I: ClientId> {
    map: IndexMap<I, Vec<u8>>
}

impl<I: ClientId> NetBuffer<I> {
    /// Creates a new NetBuffer. Typically, this will be stored as a resource in the main game world.
    pub fn new() -> Self {
        NetBuffer {
            map: IndexMap::new()
        }
    }

    /// Write a slice to a particular client.
    pub fn write_to_client(&mut self, client: I, data: &[u8]) {
        self.map.entry(client).and_modify(|buf| {
            buf.extend_from_slice(data);
        });
    }

    /// Given a policy, this returns a slice containing the buffers of all relevant clients.
    pub fn relevant_buffers<'a>(&'a mut self, pol: ClientPolicy<I>, own: Option<I>) -> Vec<&mut Vec<u8>> {
        match pol {
            ClientPolicy::All => {
                self.map.values_mut().collect::<Vec<_>>()
            },
            ClientPolicy::Exclude(exclude) => {
                self.map.iter_mut().filter(|(k, v)| {
                    !exclude.contains(k)
                }).map(|(_, v)| v).collect::<Vec<_>>()
            },
            ClientPolicy::Include(include) => {
                self.map.iter_mut().filter(|(k, _)| {
                    include.contains(k)
                }).map(|(_, v)| v).collect::<Vec<_>>()
            },
            ClientPolicy::One(one) => {
                vec![self.map.get_mut(&one).unwrap()]
            },
            ClientPolicy::Owner => {
                if let Some(own) = own {
                    vec![self.map.get_mut(&own).unwrap()]
                } else {
                    warn!("A client policy matched against an owner but no owner was specified!");
                    vec![]
                }
            },
            ClientPolicy::None => {
                vec![]
            }
        }
    }
}

type BincodeSerializer<'a, 'b> = &'a mut bincode::Serializer<&'b mut std::io::Cursor<Vec<u8>>, DefaultOptions>;

fn net_archetype_updates<S: Serializer, I: ClientId>(
    serializer_cache: Res<'static, NetArchetypes<S>>,
    world: &World,
    outbound_buf: ResMut<'static, NetBuffer<I>>
) {
    let change_tick = world.read_change_tick();

    let mut buffer : std::io::Cursor<Vec<u8>> = std::io::Cursor::new(Vec::new());


    // Aligned iterator over all archetypes and their corresponding cache entry.
    for (cache, world_archetype) in serializer_cache.archetypes.iter().map(|ae| (ae, world.archetypes().get(ae.id).unwrap())) { 
        //For each entity in this archetype, we need to check for changes and serialize them accordingly.
        for ent_ref in world_archetype.entities().iter().map(|ae| world.entity(ae.entity())) {
            serialize_entity(ent_ref, &mut buffer);
        }
    }
}

/// Discovers changes on a given entity and serializes a complete changeset message to the underlying buffer.
#[inline]
fn serialize_entity(ent: EntityRef, buf: &mut impl Write) {

}

/* 
fn net_archetype_updates<'a>(
   // serialize_archetype: Res<T>, 
    world: &World, 
    sys_changeticks: SystemChangeTick) {
    //Intersect our archetypes with the world ones.
    //We need to query the world ones for all their entities.
    let mut buffer : std::io::Cursor<Vec<u8>> = std::io::Cursor::new(Vec::new());

    //Associate each bevy [Archetype] with each local [NetArchetype].

    for (metadata, archetype) in serialize_archetype.archetypes.iter().map(|ae| (ae, world.archetypes().get(ae.id).unwrap())) {
        for entity_ref in archetype.entities().iter().map(|ae| world.entity(ae.entity())) {
            let buf = &mut buffer;
            let start = buf.position();
            //The bitmask is start -> start + ((metadata.components.len() + 7) / 8)
            buf.set_position((start as u64) + ((metadata.components.len() + 7) / 8) as u64);
            
            //buffer.set_position(buffer.position() + ((metadata.components.len() + 7) / 8) as u64);

            let mut serializer = bincode::Serializer::new(buf, DefaultOptions::new());
            
            //Move the buffer to accomodate the bitmask.
            //buffer.set_position(((metadata.components.len() + 7) / 8) as u64);

            for (idx, (comp_id, comp_count, ser)) in metadata.components.iter().enumerate() {
                let change_ticks = entity_ref.get_change_ticks_by_id(*comp_id).unwrap();
              
                //This component has changed, lets deref and serialize it.
                if change_ticks.is_changed(sys_changeticks.last_run(), sys_changeticks.this_run()) {
                    ser(entity_ref, &mut serializer);
                }
            }
        }
    }

    // for (metadata, archetype) in serialize_archetype.archetypes.iter().zip(world.archetypes().iter()).filter(|(a, b)| a.id == b.id()) {
    //     for entity_ref in archetype.entities().iter().map(|ae| world.entity(ae.entity())) {
    //         let buf = &mut buffer;
    //         let start = buf.position();
    //         //The bitmask is start -> start + ((metadata.components.len() + 7) / 8)
    //         buf.set_position((start as u64) + ((metadata.components.len() + 7) / 8) as u64);
            
    //         //buffer.set_position(buffer.position() + ((metadata.components.len() + 7) / 8) as u64);

    //         let mut serializer = bincode::Serializer::new(buf, DefaultOptions::new());
            
    //         //Move the buffer to accomodate the bitmask.
    //         //buffer.set_position(((metadata.components.len() + 7) / 8) as u64);

    //         for (idx, (comp_id, comp_count, ser)) in metadata.components.iter().enumerate() {
    //             let change_ticks = entity_ref.get_change_ticks_by_id(*comp_id).unwrap();
              
    //             //This component has changed, lets deref and serialize it.
    //             if change_ticks.is_changed(sys_changeticks.last_run(), sys_changeticks.this_run()) {
    //                 let ptr = entity_ref.get_by_id(*comp_id).unwrap();

    //                 //Lets update the bitmask for this particular bit.
    //                 let bit = idx % 8;
    //                 let byte = idx / 8;
    //                 let bitmask = 1 << bit;
    //                 let mut byte = buf.get_mut()[(start + byte) as usize];
    //                 byte |= bitmask;

    //                 // Safety:
    //                 // The serializer was associated with this ComponentId when the archetype was registered.
    //                 // This particular entity is a member of this archetype so it must still contain this component.
    //                 // As long as this 'ser' is associated with ComponentID, the usage will be correct.
    //                 ser(entity_ref, &mut serializer);
    //             }
    //         }
    //     }
    // }
}

*/