use bevy::{ecs::{system::{Commands, Query}, entity::Entity, component::Component}, app::App, transform::components::Transform};

use crate::archetype::{NetComponent, self};

/// A trait Nevy uses internally to extend the bevy Commands API.
pub trait ClientCommandsExt {
    /// Will spawn the specified Component(s) on the server and replicate them.
    /// The replication strategy will be selected based on the adjacent [Net] component.
    /// // If a corresponding [Net] component is not provided, the default strategy will be assumed.
    fn spawn_networked();
    /// Will insert the specified Component(s) on the server and replicate them.
    /// The replication strategy will be selected based on the adjacent [Net] component.
    /// If a corresponding [Net] component is not provided, the default strategy will be assumed.
    fn insert_networked();
}

/// A trait Nevy uses internally to extend the bevy App API.
pub trait ClientAppExt {
    /// Registers the associated type to be automatically networked.
    /// Upon registering, any T with an adjacent [NetSync<T>] component will be synced according to its policy.
    fn register_net_type<T: NetComponent>(&mut self) -> &mut Self;
}

impl ClientAppExt for App {
    fn register_net_type<T: NetComponent>(&mut self) -> &mut Self {
        //Register the archetype cache maintenance systems.
        archetype::register_net_component::<T>(self);
        self
    }
}


//#[derive(Component)]
struct Blah;
