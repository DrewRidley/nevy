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
//!    // Spawn an entity whose owner is 0. This entity will only exist on the server and client 0.
//!    cmds.spawn_networked((Health(15), Stamina(100), Owner(0))).with_policy::<u16>(ClientPolicy(Owner));
//! 
//!    //Spawns an entity whose owner ClientId(0). All players can view the stamina component,
//!    //but only the owner can view their own health.
//!    cmds.spawn_networked((Health(8), NetSync<Health>(ClientPolicy(Owner), Stamina(100))));
//! }
//!
//! ```
use bevy::prelude::*;
use crate::policy::ClientId;

mod serialize;
mod policy;
mod stream;


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

/// A marker to indicate that the entity is networked.
/// This will likely rarely be used but exists if you need your logic to be aware of the networked status of an entity.
/// This is shorthand for checking each of the many components for an adjacent [NetComp]/[DynNetComp].
/// Due to its sparse nature, its only recommended to use if you require absolute proof of networked state.
#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct NetEntity;

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