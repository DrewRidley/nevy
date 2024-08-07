use std::{io::Read, sync::Arc, time::Duration};

use bevy::prelude::*;
use bevy_interface::prelude::*;
use nevy_quic::prelude::*;

fn main() {
    let mut app = App::new();

    app.add_plugins(MinimalPlugins);
    app.add_plugins(bevy::log::LogPlugin {
        level: bevy::log::Level::DEBUG,
        ..default()
    });

    app.add_plugins(EndpointPlugin::default());
    app.add_plugins(StreamHeaderPlugin::default());

    app.add_systems(Startup, spawn_endpoint);
    app.add_systems(Update, (log_events, spawn_streams, receive_data));

    app.run();
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
        QuinnEndpoint::new("0.0.0.0:27018".parse().unwrap(), None, Some(server_config)).unwrap();

    commands.spawn((
        ExampleEndpoint,
        EndpointStreamHeaders,
        BevyEndpoint::new(endpoint),
    ));
}

fn spawn_streams(
    mut commands: Commands,
    endpoint_q: Query<(), With<ExampleEndpoint>>,
    mut stream_event_r: EventReader<HeaderStreamEvent>,
) {
    for HeaderStreamEvent {
        endpoint_entity,
        connection_entity,
        stream_id,
        event_type,
        ..
    } in stream_event_r.read()
    {
        if !endpoint_q.contains(*endpoint_entity) {
            continue;
        }

        if let HeaderStreamEventType::NewRecvStream(header) = event_type {
            info!("new recv stream with header {}", header);

            commands
                .spawn(ExampleStream {
                    connection_entity: *connection_entity,
                    stream_id: stream_id.clone(),
                    buffer: Vec::new(),
                })
                .set_parent(*connection_entity);
        }
    }
}

fn receive_data(
    mut commands: Commands,
    mut stream_q: Query<(Entity, &mut ExampleStream)>,
    mut connections: Connections,
) {
    for (stream_entity, mut example_stream) in stream_q.iter_mut() {
        let mut endpoint = connections
            .connection_endpoint_mut(example_stream.connection_entity)
            .unwrap();

        let mut connection = endpoint
            .connection_mut(example_stream.connection_entity)
            .unwrap();

        let mut stream = connection
            .recv_stream(example_stream.stream_id.clone())
            .unwrap()
            .unwrap();

        loop {
            match stream.recv(usize::MAX) {
                Ok(data) => {
                    example_stream.buffer.extend(data.as_ref());
                }
                Err(err) => {
                    if err.is_fatal() {
                        panic!("fatal error reading stream");
                    }

                    if !stream.is_open() {
                        info!(
                            "message received: {:?}",
                            String::from_utf8_lossy(&example_stream.buffer)
                        );

                        commands.entity(stream_entity).despawn();
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
