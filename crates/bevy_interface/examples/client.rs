use std::{io::Read, sync::Arc, time::Duration};

use bevy::prelude::*;
use bevy_interface::prelude::*;
use nevy_quic::{prelude::*, quinn_proto::crypto::rustls::QuicClientConfig};

fn main() {
    let mut app = App::new();

    app.add_plugins(MinimalPlugins);
    app.add_plugins(bevy::log::LogPlugin {
        level: bevy::log::Level::DEBUG,
        ..default()
    });

    app.add_plugins(EndpointPlugin::default());

    app.add_systems(Startup, (spawn_endpoint, apply_deferred, connect).chain());
    app.add_systems(Update, (log_events, send_message, send_stream_data));

    app.run()
}

#[derive(Component)]
struct ExampleEndpoint;

#[derive(Component)]
struct ExampleStream {
    connection_entity: Entity,
    stream_id: BevyStreamId,
    buffer: Vec<u8>,
}

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
        QuinnEndpoint::new("0.0.0.0:0".parse().unwrap(), None, Some(server_config)).unwrap();

    commands.spawn((ExampleEndpoint, BevyEndpoint::new(endpoint)));
}

fn connect(endpoint_q: Query<Entity, With<ExampleEndpoint>>, mut connections: Connections) {
    let endpoint_entity = endpoint_q.single();

    let mut config = rustls_platform_verifier::tls_config_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .unwrap();
    config.alpn_protocols = vec![b"h3".to_vec()];

    let quic_config: QuicClientConfig = config.try_into().unwrap();
    let mut quinn_client_config = nevy_quic::quinn_proto::ClientConfig::new(Arc::new(quic_config));

    let mut transport_config = nevy_quic::quinn_proto::TransportConfig::default();
    transport_config.max_idle_timeout(Some(Duration::from_secs(10).try_into().unwrap()));
    transport_config.keep_alive_interval(Some(Duration::from_millis(200)));

    quinn_client_config.transport_config(Arc::new(transport_config));

    let connection_entity = connections
        .connect::<QuinnEndpoint>(
            endpoint_entity,
            Description::new_connect_description::<QuinnEndpoint>((
                quinn_client_config,
                "127.0.0.1:27018".parse().unwrap(),
                "dev.drewridley.com".into(),
            )),
        )
        .unwrap()
        .unwrap();

    info!("connected: {:?}", connection_entity);
}

fn send_message(
    mut commands: Commands,
    mut connected_r: EventReader<Connected>,
    endpoint_q: Query<(), With<ExampleEndpoint>>,
    mut connections: Connections,
) {
    for &Connected {
        endpoint_entity,
        connection_entity,
    } in connected_r.read()
    {
        if endpoint_q.contains(endpoint_entity) {
            let mut endpoint = connections
                .connection_endpoint_mut(connection_entity)
                .unwrap();

            let mut connection = endpoint.connection_mut(connection_entity).unwrap();

            let stream_id = connection
                .open_stream(Description::new_open_description::<QuinnStreamId>(
                    nevy_quic::quinn_proto::Dir::Uni,
                ))
                .unwrap()
                .unwrap();

            debug!("Opened stream");

            commands.spawn(ExampleStream {
                connection_entity,
                stream_id,
                buffer: b"Hello Bevy!".into(),
            });
        }
    }
}

fn send_stream_data(
    mut commands: Commands,
    mut stream_q: Query<(Entity, &mut ExampleStream)>,
    mut connections: Connections,
) {
    for (stream_entity, mut stream_queue) in stream_q.iter_mut() {
        let mut endpoint = connections
            .connection_endpoint_mut(stream_queue.connection_entity)
            .unwrap();

        let mut connection = endpoint
            .connection_mut(stream_queue.connection_entity)
            .unwrap();

        let mut stream = connection
            .send_stream(stream_queue.stream_id.clone())
            .unwrap()
            .unwrap();

        loop {
            if stream_queue.buffer.len() == 0 {
                stream.close(Description::new_send_close_description::<QuinnStreamId>(
                    None,
                ));
                commands.entity(stream_entity).despawn();
                break;
            }

            match stream.send(&stream_queue.buffer) {
                Ok(sent) => {
                    if sent == 0 {
                        break;
                    }

                    stream_queue.buffer.drain(..sent);
                    debug!("sent {} bytes", sent);
                }
                Err(err) => {
                    if err.is_fatal() {
                        panic!("fatal error sending data");
                    }

                    break;
                }
            }
        }
    }
}

fn log_events(
    mut connected_r: EventReader<Connected>,
    mut disconnected_r: EventReader<Disconnected>,
) {
    for &Connected {
        endpoint_entity,
        connection_entity,
    } in connected_r.read()
    {
        info!("{:?} connected on {:?}", connection_entity, endpoint_entity);
    }

    for &Disconnected {
        endpoint_entity,
        connection_entity,
    } in disconnected_r.read()
    {
        info!(
            "{:?} disconnected on {:?}",
            connection_entity, endpoint_entity
        );
    }
}
