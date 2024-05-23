use bevy::{log::LogPlugin, prelude::*};
use nevy::client::{ClientPlugin, ConnectClient};

fn connect_to_server(mut writer: EventWriter<ConnectClient>) {
    writer.send(ConnectClient("https://dev.drewridley.com".into()));
}

fn main() {
    App::new()
    .add_plugins(MinimalPlugins)
    .add_plugins(LogPlugin {
        level: bevy::log::Level::DEBUG,
        ..Default::default()
    })
    .add_plugins(ClientPlugin)
    .add_systems(Startup, connect_to_server)
    .run();
}