use bevy::{ecs::system::Commands, app::App};


/// A trait Nevy uses internally to extend the bevy Commands API.
pub trait ServerCommandsExt {
    fn spawn_networked();
    fn insert_networked();
}

/// A trait Nevy uses internally to extend the bevy App API.
pub trait ServerAppExt {
    fn register_server_netbundle();
    fn register_client_netbundle();
}


impl ServerCommandsExt for Commands<'_, '_> {
    /// Will spawn the specified Component(s) on the server and replicate them.
    /// The replication strategy will be selected based on the adjacent [Net] component.
    /// If a corresponding [Net] component is not provided, the default strategy will be assumed.
    fn insert_networked() {
        todo!()
    }

    /// Will insert the specified Component(s) on the server and replicate them.
    /// The replication strategy will be selected based on the adjacent [Net] component.
    /// If a corresponding [Net] component is not provided, the default strategy will be assumed.
    fn spawn_networked() {
        todo!()
    }
}