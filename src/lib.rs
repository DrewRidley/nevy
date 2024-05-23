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

use unique_type_id::UniqueTypeId;

pub mod client;
pub mod server;
mod streaming;
mod messages;

/// A trait implemented on all networked types.
/// Describes that a particular piece of state is to be networked.
pub trait NetComponent: serde::Serialize + serde::de::DeserializeOwned + unique_type_id::UniqueTypeId<u16> + Send + Sync + 'static { }

pub trait NetMessage: UniqueTypeId<u16> + serde::Serialize + serde::de::DeserializeOwned + 'static {}

/// A component whose state is streamed over the network.
/// Any entity with one or more 'Net<T>' will be automatically replicated to peers.
#[derive(bevy::ecs::component::Component)]
pub struct Net<T: NetComponent>(T);

