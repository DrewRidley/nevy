use std::{collections::HashMap, io::Read, sync::Arc, time::Duration};

use bevy::prelude::*;
use bevy_interface::prelude::*;
use nevy_quic::prelude::*;
use transport_interface::*;

fn main() {
    let mut app = App::new();

    app.add_plugins(MinimalPlugins);
    app.add_plugins(bevy::log::LogPlugin {
        level: bevy::log::Level::DEBUG,
        ..default()
    });

    app.add_plugins(EndpointPlugin::<QuinnEndpoint>::default());

    app.add_systems(Startup, (spawn_endpoint, apply_deferred, connect));

    app.run()
}

#[derive(Component)]
struct ExampleEndpoint;

fn load_certs() -> rustls::ServerConfig {
    let chain = std::fs::File::open("fullchain.pem").expect("failed to open cert file");
    let mut chain: std::io::BufReader<std::fs::File> = std::io::BufReader::new(chain);

    let chain: Vec<rustls::pki_types::CertificateDer> = rustls_pemfile::certs(&mut chain)
        .collect::<Result<_, _>>()
        .expect("failed to load certs");
    let mut keys = std::fs::File::open("privkey.pem").expect("failed to open key file");

    let mut buf = Vec::new();
    keys.read_to_end(&mut buf).unwrap();

    let key = rustls_pemfile::private_key(&mut std::io::Cursor::new(&buf))
        .expect("failed to load private key")
        .expect("missing private key");

    let config = rustls::ServerConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_protocol_versions(&[&rustls::version::TLS13])
    .unwrap()
    .with_no_client_auth()
    .with_single_cert(chain, key)
    .unwrap();

    config
}

fn spawn_endpoint(mut commands: Commands) {
    let mut config = load_certs();

    config.max_early_data_size = u32::MAX;
    config.alpn_protocols = vec![b"h3".to_vec()]; // this one is important

    let config: nevy_quic::quinn_proto::crypto::rustls::QuicServerConfig =
        config.try_into().unwrap();

    let mut server_config = nevy_quic::quinn_proto::ServerConfig::with_crypto(Arc::new(config));

    let mut transport_config = nevy_quic::quinn_proto::TransportConfig::default();
    transport_config.enable_segmentation_offload(false);
    transport_config.max_idle_timeout(Some(Duration::from_secs(600).try_into().unwrap()));

    server_config.transport = Arc::new(transport_config);

    let endpoint =
        QuinnEndpoint::new("0.0.0.0:27018".parse().unwrap(), None, Some(server_config)).unwrap();

    commands.spawn((ExampleEndpoint, BevyEndpoint::new(endpoint)));
}

fn connect(endpoint_q: Query<Entity, With<ExampleEndpoint>>, mut connections: Connections) {
    let endpoint_entity = endpoint_q.single();

    connections.connect(endpoint_entity, ());
}
