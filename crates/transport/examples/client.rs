use std::{io::Read, sync::Arc};

use bevy::{app::{App, Startup}, ecs::system::Commands, log::{debug, trace, Level, LogPlugin}, MinimalPlugins};
use quinn_proto::crypto::rustls::QuicClientConfig;
use transport::prelude::*;



fn create_endpoint_sys(mut cmds: Commands) {
    let mut endpoint = NativeEndpoint::new("0.0.0.0:0".parse().unwrap(), None, None, true).unwrap();

    let mut cfg = rustls_platform_verifier::tls_config_with_provider(Arc::new(rustls::crypto::ring::default_provider())).unwrap();
    cfg.alpn_protocols = vec![b"h3".to_vec()];

    let quic_cfg: QuicClientConfig = cfg.try_into().unwrap();
    let cfg = quinn_proto::ClientConfig::new(Arc::new(quic_cfg));

    endpoint.connect(cfg, "127.0.0.1:443".parse().unwrap(), "dev.drewridley.com").unwrap();

    cmds.spawn(endpoint);
}

fn main() {
    App::new()
        .add_plugins(MinimalPlugins)
        .add_plugins(LogPlugin {
            level: Level::TRACE,
            ..Default::default()
        })
        .add_plugins(NativeEndpointPlugin)
        .add_systems(Startup, create_endpoint_sys)
        .run();
}