use std::sync::Arc;

use nevy_quic::prelude::*;
use quinn_proto::crypto::rustls::QuicClientConfig;
use transport_interface::*;

fn main() {
    let mut endpoint = QuinnEndpoint::new("0.0.0.0:0".parse().unwrap(), None, None).unwrap();

    let mut cfg = rustls_platform_verifier::tls_config_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .unwrap();
    cfg.alpn_protocols = vec![b"h3".to_vec()];

    let quic_cfg: QuicClientConfig = cfg.try_into().unwrap();
    let cfg = quinn_proto::ClientConfig::new(Arc::new(quic_cfg));

    let mut context = QuinnContext::default();

    let mut endpoint = EndpointRefMut {
        endpoint: &mut endpoint,
        context: &mut context,
    };

    endpoint
        .connect((
            cfg,
            "127.0.0.1:27018".parse().unwrap(),
            "dev.drewridley.com".to_string(),
        ))
        .unwrap();

    event_loop(endpoint);
}

fn event_loop<E: Endpoint>(mut endpoint: EndpointRefMut<E>) {
    loop {
        endpoint.update();

        while let Some(event) = endpoint.poll_event() {
            match event.event {
                ConnectionEvent::Connected => {
                    println!("connection");
                }
                ConnectionEvent::Disconnected => {
                    println!("disconnection");
                }
            }
        }
    }
}
