use std::sync::Arc;

use nevy_web_transport::{endpoint::WebTransportEndpoint, streams::WebTransportStreamId};
use quinn_proto::crypto::rustls::QuicClientConfig;
use transport_interface::*;

fn main() {
    simple_logger::SimpleLogger::new().env().init().unwrap();

    let mut endpoint = WebTransportEndpoint::new("0.0.0.0:0".parse().unwrap(), None, None).unwrap();

    let mut cfg = rustls_platform_verifier::tls_config_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .unwrap();
    cfg.alpn_protocols = vec![b"h3".to_vec()];

    let quic_cfg: QuicClientConfig = cfg.try_into().unwrap();
    let cfg = quinn_proto::ClientConfig::new(Arc::new(quic_cfg));

    endpoint
        .connect((
            cfg,
            "127.0.0.1:443".parse().unwrap(),
            "dev.drewridley.com".to_string(),
        ))
        .unwrap();

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

                    let mut connection = endpoint.connection_mut(connection_id).unwrap();

                    let stream_id: WebTransportStreamId =
                        connection.open_stream(quinn_proto::Dir::Uni).unwrap();
                    let mut stream = connection.send_stream_mut(stream_id).unwrap();
                    stream.send(&[1, 2, 3, 4]).unwrap();
                    stream.close(None).unwrap();
                }
                ConnectionEvent::Disconnected => {
                    println!("disconnection");
                }
            }
        }
    }
}
