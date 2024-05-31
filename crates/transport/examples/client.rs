use std::sync::Arc;

use bevy::{log::{Level, LogPlugin}, prelude::*};
use quinn_proto::crypto::rustls::QuicClientConfig;
use transport::{prelude::*, web_transport::{WebTransportEndpoint, WebTransportEndpointPlugin}};
use transport::bevy::*;



fn create_endpoint_sys(mut cmds: Commands) {
    let mut endpoint = EndpointState::new("0.0.0.0:0".parse().unwrap(), None, None).unwrap();

    let mut cfg = rustls_platform_verifier::tls_config_with_provider(Arc::new(rustls::crypto::ring::default_provider())).unwrap();
    cfg.alpn_protocols = vec![b"h3".to_vec()];

    let quic_cfg: QuicClientConfig = cfg.try_into().unwrap();
    let cfg = quinn_proto::ClientConfig::new(Arc::new(quic_cfg));

    endpoint.connect(cfg, "127.0.0.1:443".parse().unwrap(), "dev.drewridley.com").unwrap();

    cmds.spawn((
        endpoint,
        WebTransportEndpoint::default(),
    ));
}

fn send_message(
    mut connected_r: EventReader<Connected>,
    mut endpoint_q: Query<&mut EndpointState>,
) {
    for &Connected { endpoint_entity, connection_id } in connected_r.read() {
        info!("sending hello world message");

        let mut endpoint = endpoint_q.get_mut(endpoint_entity).unwrap();
        let connection = endpoint.get_connection_mut(connection_id).unwrap();

        let stream_id = connection.open_uni().unwrap();
        connection.write(stream_id, b"Hello Server!").unwrap();
        connection.finish(stream_id);
    }
}

fn main() {
    App::new()
        .add_plugins(MinimalPlugins)
        .add_plugins(LogPlugin {
            // level: Level::TRACE,
            level: Level::TRACE,
            filter: "wgpu=error,naga=warn,quinn_proto::connection=debug,quinn_proto::endpoint=debug".into(),
            ..default()
        })
        .add_plugins(BevyEndpointPlugin::default())
        .add_plugins(WebTransportEndpointPlugin::default())
        .add_systems(Startup, create_endpoint_sys)
        .add_systems(Update, send_message)
        .run();
}
