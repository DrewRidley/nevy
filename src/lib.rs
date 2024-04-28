#![feature(mem_copy_fn)]
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
use bevy::{ecs::{component::ComponentId, schedule::ScheduleLabel, storage::Tables}, prelude::*, utils::{hashbrown::HashSet}};
use std::{any::{Any, TypeId}, hash::Hash, marker::PhantomData, mem::size_of_val};

pub mod client;
pub mod muxer;
pub mod server;
pub mod messages;
pub mod rpc;
mod encoder;


/// A marker trait indicating that a given type can be used as a client identifier.
/// This will be auto-implemented on all types that implement [Hash], [Send] and [Sync].
/// [Send] and [Sync] are required because the underlying network buffer is potentially shared between threads.
pub trait ClientId: Hash + Eq + Send + Sync + Clone { }

//Implement ClientId for all types that can be used as an ID.
impl<T: Hash + Eq + Send + Sync + Clone> ClientId for T { }


/// A marker component indicating that the adjacent type 'T' shall be networked on this specific entity.
#[derive(Component)]
pub struct Net<T: Component>(PhantomData<T>);


///A marker component to indiciate that the specified entity is 'owned' by a particular client.
/// This state is NOT replicated by default.
/// If ownership of a particular entity needs to be networked, consider [Net<Owned>].
pub struct Owned<I: ClientId>(PhantomData<I>);


#[derive(ScheduleLabel, Debug, Hash, PartialEq, Eq, Clone)]
struct NetworkStreamSet;

pub trait AppExtension {
    fn register_net_component<C: Component>(&mut self) -> &mut Self;
}

#[derive(Debug)]
struct NetArchetypeRegistry(HashSet<ComponentId>);

impl AppExtension for bevy::app::App {
    /// Registers the usage of a specific network component, 'C'.
    /// This method MUST be called on every component used.
    fn register_net_component<C: Component>(&mut self) -> &mut Self {
        // Detect new entities with Net<C>.
        // These entities might indicate a new networked archetype that has to be tracked and potentially streamed.
        self.add_systems(NetworkStreamSet, | q: Query<Entity, Added<Net<C>>>| {

        });

        self
    }
}


#[derive(Component)]
struct A(u32);


use lz4_flex::block::{compress_prepend_size, decompress_size_prepended};

#[test]
fn test_column_copy() {
    let mut server = App::new();

    // Spawn 9,999 entities with the same value
    for _ in 0..9999 {
        server.world.spawn(A(1));
    }

    // Spawn one entity with a distinct value
    let ent_to_change = server.world.spawn(A(2)).id();

    let archetype = server.world.archetypes().iter().filter(|arch| {
        arch.entities().len() > 0
    }).next().expect("Failed to get archetype!");
    let table_id = archetype.table_id();
    let table = server.world.storages().tables.get(table_id).unwrap();

    let column = table.get_column(server.world.component_id::<A>().expect("Component ID not found")).expect("Failed to get column!");

    let start_ptr: *mut u8 = column.get_data_ptr().as_ptr();
    let bytes_to_copy = table.entity_count() * column.item_layout().size(); // Only copying two entities for demonstration
    let mut copy: Vec<u8> = Vec::with_capacity(bytes_to_copy);

    unsafe {
        copy.set_len(bytes_to_copy);
        std::ptr::copy_nonoverlapping(start_ptr, copy.as_mut_ptr(), bytes_to_copy);
    }

    // Change the value of the specific entity
    *server.world.get_mut::<A>(ent_to_change).unwrap() = A(23432432);

    let mut delta: Vec<u8> = Vec::with_capacity(bytes_to_copy);

    unsafe {
        delta.set_len(bytes_to_copy);
        for i in 0..bytes_to_copy {
            delta[i] = *start_ptr.add(i) ^ copy[i];
        }
    }

    let compressed = compress_prepend_size(delta.as_slice());

    panic!("Compressed: {} | Uncompressed: {}", compressed.len(), delta.len());
}

