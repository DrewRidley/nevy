use std::{io::Read, sync::Arc, time::Duration};

use bevy::log::Level;
use bevy::log::LogPlugin;
use bevy::prelude::*;
use transport::prelude::*;
use transport::bevy::*;

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



fn create_endpoint_sys(mut commands: Commands) {
    let mut config = load_certs();

    config.max_early_data_size = u32::MAX;
    config.alpn_protocols = vec![b"h3".to_vec()]; // this one is important

    let config: quinn_proto::crypto::rustls::QuicServerConfig = config.try_into().unwrap();

    let mut server_config = quinn_proto::ServerConfig::with_crypto(Arc::new(config));

    let mut transport_config = quinn_proto::TransportConfig::default();
    transport_config.enable_segmentation_offload(false);
    transport_config.max_idle_timeout(Some(Duration::from_secs(600).try_into().unwrap()));

    server_config.transport = Arc::new(transport_config);

    let endpoint = EndpointState::new("0.0.0.0:443".parse().unwrap(), None, Some(server_config)).unwrap();

    commands.spawn((
        endpoint,
        BevyEndpoint,
    ));
}

#[derive(Component)]
struct Stream {
    endpoint_entity: Entity,
    connection_id: ConnectionId,
    stream_id: StreamId,
    data: Vec<u8>,
}

fn spawn_streams(
    mut commands: Commands,
    mut new_stream_r: EventReader<NewStream>,
) {
    for &NewStream { endpoint_entity, connection_id, stream_id, .. } in new_stream_r.read() {
        commands.spawn(Stream {
            endpoint_entity,
            connection_id,
            stream_id,
            data: Vec::new(),
        });
    }
}

fn read_streams(
    mut stream_q: Query<&mut Stream>,
    mut endpoint_q: Query<&mut EndpointState>,
) {
    for mut stream in stream_q.iter_mut() {
        let mut endpoint = endpoint_q.get_mut(stream.endpoint_entity).unwrap();
        let connection = endpoint.get_connection_mut(stream.connection_id).unwrap();

        for data in connection.reader(stream.stream_id).read() {
            stream.data.extend(data.as_ref());
        }
    }
}

fn finish_streams(
    mut commands: Commands,
    mut closed_stream_r: EventReader<ReceiveStreamClosed>,
    stream_q: Query<(Entity, &Stream)>,
) {
    for &ReceiveStreamClosed { endpoint_entity, connection_id, stream_id, .. } in closed_stream_r.read() {
        for (stream_entity, stream) in stream_q.iter() {
            if {
                stream.endpoint_entity == endpoint_entity &&
                stream.connection_id == connection_id &&
                stream.stream_id == stream_id
            } {

                info!("stream finished, message: {:?}", std::str::from_utf8(&stream.data));

                commands.entity(stream_entity).despawn();
                continue;
            }
        }
    }
}



fn main() {
    App::new()
        .add_plugins(MinimalPlugins)
        .add_plugins(LogPlugin {
            // level: Level::TRACE,
            level: Level::DEBUG,
            ..Default::default()
        })
        .add_plugins(BevyEndpointPlugin::default())
        .add_systems(Startup, create_endpoint_sys)
        .add_systems(Update, (
            spawn_streams,
            read_streams,
            finish_streams,
        ))
        .run();
}