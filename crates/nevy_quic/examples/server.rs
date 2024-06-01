use std::{collections::HashMap, io::Read, sync::Arc, time::Duration};

use nevy_quic::prelude::*;
use transport_interface::*;

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

fn main() {
    let mut config = load_certs();

    config.max_early_data_size = u32::MAX;
    config.alpn_protocols = vec![b"h3".to_vec()]; // this one is important

    let config: quinn_proto::crypto::rustls::QuicServerConfig = config.try_into().unwrap();

    let mut server_config = quinn_proto::ServerConfig::with_crypto(Arc::new(config));

    let mut transport_config = quinn_proto::TransportConfig::default();
    transport_config.enable_segmentation_offload(false);
    transport_config.max_idle_timeout(Some(Duration::from_secs(600).try_into().unwrap()));

    server_config.transport = Arc::new(transport_config);

    let mut endpoint =
        QuinnEndpoint::new("0.0.0.0:27018".parse().unwrap(), None, Some(server_config)).unwrap();

    let mut connections = HashMap::new();

    loop {
        endpoint.update();

        while let Some(EndpointEvent {
            connection_id,
            event,
        }) = endpoint.poll_event()
        {
            match event {
                ConnectionEvent::Connected => {
                    println!("connection");
                    connections.insert(connection_id, HashMap::new());
                }
                ConnectionEvent::Disconnected => {
                    println!("disconnection");
                    connections.remove(&connection_id);
                }
            }
        }

        for (&connection_id, streams) in connections.iter_mut() {
            let mut connection = endpoint.connection_mut(connection_id).unwrap();

            while let Some(event) = connection.poll_stream_events::<QuinnStreamId>() {
                match event {
                    StreamEvent::NewRecvStream(stream_id) => {
                        streams.insert(stream_id, Vec::<u8>::new());
                    }
                    StreamEvent::ClosedRecvStream(stream_id) => {
                        let stream = streams.remove(&stream_id).unwrap();

                        println!("stream closed, message {:?}", stream);
                    }
                    _ => (),
                }
            }

            for (&stream_id, stream) in streams.iter_mut() {
                if let Ok(bytes) = connection
                    .recv_stream_mut(stream_id)
                    .unwrap()
                    .recv(usize::MAX)
                {
                    stream.extend(bytes.as_ref());
                }
            }
        }
    }
}
