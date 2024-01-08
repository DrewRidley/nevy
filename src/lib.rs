use std::{marker::PhantomData, any::TypeId};
use bevy::{prelude::*, ecs::{archetype::ArchetypeId, component::ComponentId, system::SystemChangeTick}, utils::HashMap, ptr::Ptr};
use bincode::DefaultOptions;
use smallvec::SmallVec;

mod serialize;

/// A marker to indicate that component [T] is networked for this entity.
#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct NetComp<T>(PhantomData<T>);

/// A marker to indicate that the entity is networked.
/// This will likely rarely be used but exists if you need your logic to be aware of the networked status of an entity.
#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct NetEntity;

/// Represents an archetype in a networked system.
///
/// `NetArchetype` is used to manage a collection of components that are networked together.
/// Each component in this archetype is identified by a `ComponentId` and contains data of type `F`.
///
/// # Type Parameter
/// `F`: The type of the underlying serializer/deserializer for this archetype.
/// Usually, you will have two NetArchetypes, one for serialization and one for deserialization
struct NetArchetype<F> {
    // The bevy [ArchetypeId] being tracked by this structure.
    id: ArchetypeId,

    // All of the components and their associated function [F].
    /// 
    /// This structure contains the (ComponentId, # of entities with component, [F]).
    /// Typically F will be a serializer or deserializer, but it is not limited to this usage.
    components: SmallVec::<[(ComponentId, u32, F); 5]>,
}


impl<F> NetArchetype<F> {
    /// Will register the component [T] and its associated function.
    /// 
    /// # Parameters
    /// `world`: The [World] to register this component with.
    /// `func`: The function to use for this component.
    fn register_usage<T: Component>(&mut self, world: &World, func: F) {
        if let Some(comp_id) = world.components().get_id(TypeId::of::<T>()) {
            //This comp should be registered.
            if !self.components.iter().any(|(id, _ , _)| *id == comp_id) {
                //This comp is not registered, so register it.
                self.components.push((comp_id, 0, func));
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
struct NetArchetypes<F> {
    archetypes: Vec<NetArchetype<F>>
}


type BinSerializer<'a, 'b> = &'a mut bincode::Serializer<&'b mut std::io::Cursor<Vec<u8>>, bincode::DefaultOptions>;
type BinSerializeFn = for<'a, 'b, 'c> unsafe fn(Ptr<'a>, BinSerializer<'b, 'c>);


fn net_archetype_updates<'a>(
    serialize_archetype: Res<NetArchetypes<BinSerializeFn>>, 
    world: &World, 
    sys_changeticks: SystemChangeTick) {
    //Intersect our archetypes with the world ones.
    //We need to query the world ones for all their entities.
    let mut buffer : std::io::Cursor<Vec<u8>> = std::io::Cursor::new(Vec::new());

    //Associate each bevy [Archetype] with each local [NetArchetype].

        for (metadata, archetype) in serialize_archetype.archetypes.iter().sort

    for (metadata, archetype) in serialize_archetype.archetypes.iter().zip(world.archetypes().iter()).filter(|(a, b)| a.id == b.id()) {
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
                    let ptr = entity_ref.get_by_id(*comp_id).unwrap();

                    //Lets update the bitmask for this particular bit.
                    let bit = idx % 8;
                    let byte = idx / 8;
                    let bitmask = 1 << bit;
                    let mut byte = buf.get_mut()[(start + byte) as usize];
                    byte |= bitmask;

                    // Safety:
                    // The serializer was associated with this ComponentId when the archetype was registered.
                    // This particular entity is a member of this archetype so it must still contain this component.
                    // As long as this 'ser' is associated with ComponentID, the usage will be correct.
                    unsafe { 
                        ser(ptr, &mut serializer);
                    }
                }
            }
        }
    }
}