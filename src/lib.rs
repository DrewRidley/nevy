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
use std::hash::Hash;

pub mod policy;
pub mod stream;
pub mod messages;


/// A marker trait indicating that a given type can be used as a client identifier.
/// This will be auto-implemented on all types that implement [Hash], [Send] and [Sync].
/// [Send] and [Sync] are required because the underlying network buffer is potentially shared between threads.
pub trait ClientId: Hash + Eq + Send + Sync + Clone { }

//Implement ClientId for all hashable types.
impl<T: Hash + Eq + Send + Sync + Clone> ClientId for T { }
