use std::any::TypeId;
use std::io::Write;
use std::marker::PhantomData;

use bevy::ecs::system::SystemChangeTick;
use indexmap::IndexMap;
use serde::{Serializer, Serialize};
use smallvec::SmallVec;

use bevy::{prelude::*, ecs::archetype::ArchetypeId};
use bevy::ecs::component::ComponentId;
use stack_vec::{stack, StackVec64};

use crate::Owner;
use crate::policy::{ClientId, ClientPolicy, PolicyIter};


/// Contains a list of each unique pair of (ClientId, Component) on this entity.
/// 
/// This is used to build unique component permutations for each Client's specific message requirements.
/// It optimizes by grouping similar component policies to reduce message fragmentation.
/// Thus, it is recommended where possible to have as few unique policies as your game logic permits.
/// 
/// # Type Parameter
/// `I`: The type of the client identifier.
#[derive(Component)]
#[component(storage = "SparseSet")]
struct EntityPolicyCache<I: ClientId> {
    //Contains a list of policies and their corresponding components.
    entries: IndexMap<ClientPolicy<I>, SmallVec<[ComponentId; 6]>>
}

impl<I: ClientId> IntoIterator for EntityPolicyCache<I> {
    type Item = (ClientPolicy<I>, SmallVec<[ComponentId; 6]>);
    type IntoIter = indexmap::map::IntoIter<ClientPolicy<I>, SmallVec<[ComponentId; 6]>>;
    fn into_iter(self) -> Self::IntoIter {
        self.entries.into_iter()
    }
}

impl<I: Default + ClientId> Default for ClientPolicy<I> {
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
/// 'I': The type of the identifier used in this context.
#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct NetComp<I: ClientId, T>(pub ClientPolicy<I>, PhantomData<T>);

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
pub struct DynNetComp<I: ClientId, T>(pub ClientPolicy<I>, PhantomData<T>);


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
/// This is used internally to manage networked state. See [NetComp] for more information.
/// If you wish to send a generic message, please look at [MessageSender].
#[derive(Resource, Default)]
struct NetBuffer<I: ClientId> {
    map: IndexMap<I, Vec<u8>>
}

impl<I: ClientId + Copy> NetBuffer<I> {
    /// Creates a new NetBuffer. Typically, this will be stored as a resource in the main game world.
    pub fn new() -> Self {
        NetBuffer {
            map: IndexMap::new()
        }
    }

    /// Write a piece of data  to a specific client.
    pub fn write_to_client(&mut self, client: I, data: &[u8]) {
        self.map.entry(client).and_modify(|buf| {
            buf.extend_from_slice(data);
        });
    }

    /// Given a policy, this returns a slice containing the buffers of all relevant clients.
    pub fn relevant_buffers(
        &mut self,
        pol: &ClientPolicy<I>,
        own: Option<I>,
    ) -> PolicyIter<
        &mut Vec<u8>,
        impl Iterator<Item = &mut Vec<u8>> + DoubleEndedIterator + core::iter::FusedIterator,
        impl Iterator<Item = &mut Vec<u8>> + DoubleEndedIterator + core::iter::FusedIterator,
        impl Iterator<Item = &mut Vec<u8>> + DoubleEndedIterator + core::iter::FusedIterator,
        impl Iterator<Item = &mut Vec<u8>> + DoubleEndedIterator + core::iter::FusedIterator,
        impl Iterator<Item = &mut Vec<u8>> + DoubleEndedIterator + core::iter::FusedIterator,
    > {

        match pol {
            ClientPolicy::All => PolicyIter::All(self.map.values_mut()),
            ClientPolicy::Exclude(exclude) => PolicyIter::Exclude(
                self.map
                    .iter_mut()
                    .filter(move |(k, _)| !exclude.contains(k))
                    .map(|(_, v)| v),
            ),
            ClientPolicy::Include(include) => PolicyIter::Include(
                self.map
                    .iter_mut()
                    .filter(move |(k, _)| include.contains(k))
                    .map(|(_, v)| v),
            ),
            ClientPolicy::One(one) => PolicyIter::One(core::iter::once(
                self.map.get_mut(one).expect("The given key was not found"),
            )),
            ClientPolicy::Owner => PolicyIter::Owner(if let Some(own) = own {
                Some(self.map.get_mut(&own).expect("Owner was not found")).into_iter()
            } else {
                warn!("A client policy matched against an owner but no owner was specified!");
                None.into_iter()
            }),
            ClientPolicy::None => PolicyIter::None,
        }
    }
}



fn net_archetype_updates<S: Serializer, I: ClientId>(
    serializer_cache: Res<'static, NetArchetypes<S>>,
    world: &World,
    buffer: ResMut<'static, NetBuffer<I>>,
    sys_changeticks: SystemChangeTick,
) {
    let change_tick = world.read_change_tick();


    // Aligned iterator over all archetypes and their corresponding cache entry.
    for (cache, world_archetype) in serializer_cache.archetypes.iter().map(|ae| (ae, world.archetypes().get(ae.id).unwrap())) { 
        //For each entity in this archetype, we need to check for changes and serialize them accordingly.
        for ent_ref in world_archetype.entities().iter().map(|ae| world.entity(ae.entity())) {
            let policy_cache = ent_ref.get::<EntityPolicyCache<I>>().unwrap();
            let owner = ent_ref.get::<Owner<I>>().unwrap();

            let changes = cache.components.iter().map(|(comp_id, _, _)| *comp_id)
                .filter(|id| ent_ref.get_change_ticks_by_id(*id).unwrap().is_changed(sys_changeticks.last_run(), sys_changeticks.this_run())).collect::<StackVec64<_>>();

            //For each unique policy...
            //This code filters out any components that have not changed.
            for (policy, comps) in policy_cache.into_iter().filter(|(_, comp)| changes.iter().any(|c| comp.contains(c))) {
                let serialized_comps = stack![u8; 100];
                
                for buffer in buffer.relevant_buffers(policy, owner) {

                }
            }
        }
    }
}

/// Serializes the components in the set on the given entity into a message.
#[inline]
fn serialize_component_set(ent: &EntityRef, components: &[ComponentId], ) -> &[u8] {
    &[0u8]
}