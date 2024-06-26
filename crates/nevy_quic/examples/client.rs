use std::{sync::Arc, time::Duration};

use nevy_quic::prelude::*;
use transport_interface::*;

fn main() {
    let mut endpoint = QuinnEndpoint::new("0.0.0.0:0".parse().unwrap(), None, None).unwrap();

    let mut config = rustls_platform_verifier::tls_config_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .unwrap();
    config.alpn_protocols = vec![b"h3".to_vec()];

    let quic_config: nevy_quic::quinn_proto::crypto::rustls::QuicClientConfig =
        config.try_into().unwrap();
    let mut quinn_client_config = nevy_quic::quinn_proto::ClientConfig::new(Arc::new(quic_config));

    let mut transport_config = nevy_quic::quinn_proto::TransportConfig::default();
    transport_config.max_idle_timeout(Some(Duration::from_secs(10).try_into().unwrap()));
    transport_config.keep_alive_interval(Some(Duration::from_millis(200)));

    quinn_client_config.transport_config(Arc::new(transport_config));

    endpoint
        .connect((
            quinn_client_config,
            "127.0.0.1:27018".parse().unwrap(),
            "dev.drewridley.com".into(),
        ))
        .unwrap();

    loop {
        struct Handler {
            connections: Vec<QuinnConnectionId>,
        }

        impl EndpointEventHandler<QuinnEndpoint> for Handler {
            fn connection_request<'a>(
                &mut self,
                _request: <QuinnEndpoint as Endpoint>::IncomingConnectionInfo<'a>,
            ) -> bool {
                false
            }

            fn connected(&mut self, connection_id: <QuinnEndpoint as Endpoint>::ConnectionId) {
                println!("connection");
                self.connections.push(connection_id);
            }

            fn disconnected(&mut self, _connection_id: <QuinnEndpoint as Endpoint>::ConnectionId) {
                println!("disconnection");
            }
        }

        let mut handler = Handler {
            connections: Vec::new(),
        };

        endpoint.update(&mut handler);

        for connection_id in handler.connections {
            let mut connection = endpoint.connection_mut(connection_id).unwrap();

            let stream_id: QuinnStreamId = connection
                .open_stream(nevy_quic::quinn_proto::Dir::Uni)
                .unwrap();
            let mut stream = connection.send_stream(stream_id).unwrap();
            stream.send(&[1, 2, 3, 4]).unwrap();
            stream.close(None).unwrap();
        }
    }
}
