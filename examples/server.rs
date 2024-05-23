use bevy::{log::LogPlugin, prelude::*};
use nevy::server::ServerPlugin;

fn main() {
    App::new()
    .add_plugins(MinimalPlugins)
    .add_plugins(LogPlugin {
        level: bevy::log::Level::DEBUG,
        ..Default::default()
    })
    .add_plugins(ServerPlugin)
    .run();
}