use std::{io::Read, sync::Arc, time::Duration};

use bevy::{app::{App, Startup, Update}, ecs::system::{Commands, Query}, log::{debug, trace, Level, LogPlugin}, MinimalPlugins};
use quinn_proto::VarInt;
use rustls::server;
use transport::prelude::*;

fn load_certs() -> rustls::ServerConfig {
    let chain = std::fs::File::open("fullchain.pem").expect("failed to open cert file");
    let mut chain: std::io::BufReader<std::fs::File> = std::io::BufReader::new(chain);

    let chain: Vec<rustls::pki_types::CertificateDer> = rustls_pemfile::certs(&mut chain)
        .collect::<Result<_, _>>()
        .expect("failed to load certs");

    trace!("Loading private key for server.");
    let mut keys = std::fs::File::open("privkey.pem").expect("failed to open key file");

    let mut buf = Vec::new();
    keys.read_to_end(&mut buf).unwrap();

    let key = rustls_pemfile::private_key(&mut std::io::Cursor::new(&buf))
        .expect("failed to load private key")
        .expect("missing private key");

    debug!("Loaded certificate files.");

    let  config = rustls::ServerConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_protocol_versions(&[&rustls::version::TLS13]).unwrap()
    .with_no_client_auth()
    .with_single_cert(chain, key).unwrap();

    config
}



fn create_endpoint_sys(mut cmds: Commands) {
    let mut config = load_certs();

    config.max_early_data_size = u32::MAX;
    config.alpn_protocols = vec![b"h3".to_vec()]; // this one is important

    let config: quinn_proto::crypto::rustls::QuicServerConfig = config.try_into().unwrap();

    let mut server_config = quinn_proto::ServerConfig::with_crypto(Arc::new(config));

    let mut transport_config = quinn_proto::TransportConfig::default();
    transport_config.enable_segmentation_offload(false);
    transport_config.max_idle_timeout(Some(Duration::from_secs(600).try_into().unwrap()));

    server_config.transport = Arc::new(transport_config);

    let endpoint = NativeEndpoint::new("0.0.0.0:443".parse().unwrap(), None, Some(server_config), true).unwrap();

    cmds.spawn(endpoint);
}

fn receive_datagrams_system(mut ep_q: Query<&mut NativeEndpoint>) {
    for mut ep in ep_q.iter_mut() {
        for cid in ep.connections() {
            match ep.recv_datagram(cid) {
                Ok(res) => {
                    let Some(bytes) = res else {
                        continue;
                    };

                    println!("Received datagram: {}", String::from_utf8(bytes.to_vec()).unwrap());
                },
                Err(err) => unreachable!("The connection must exist!"),
            }
        }
    }
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
        .add_systems(Update, receive_datagrams_system)
        .run();
}