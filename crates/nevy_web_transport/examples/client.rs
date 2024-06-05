use std::sync::Arc;

use nevy_quic::connection::QuinnConnectionId;
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
        .connect((cfg, "127.0.0.1:443".parse().unwrap(), "dev.drewridley.com"))
        .unwrap();

    loop {
        struct Handler {
            connections: Vec<QuinnConnectionId>,
        }

        impl EndpointEventHandler<WebTransportEndpoint> for Handler {
            fn connection_request<'a>(
                &mut self,
                _request: <WebTransportEndpoint as Endpoint>::IncomingConnectionInfo<'a>,
            ) -> bool {
                false
            }

            fn connected(
                &mut self,
                connection_id: <WebTransportEndpoint as Endpoint>::ConnectionId,
            ) {
                println!("connection");
                self.connections.push(connection_id);
            }

            fn disconnected(
                &mut self,
                _connection_id: <WebTransportEndpoint as Endpoint>::ConnectionId,
            ) {
                println!("disconnection");
            }
        }

        let mut handler = Handler {
            connections: Vec::new(),
        };

        endpoint.update(&mut handler);

        for connection_id in handler.connections {
            let mut connection = endpoint.connection_mut(connection_id).unwrap();

            let stream_id: WebTransportStreamId =
                connection.open_stream(quinn_proto::Dir::Uni).unwrap();
            let mut stream = connection.send_stream_mut(stream_id).unwrap();
            stream.send(&[1, 2, 3, 4]).unwrap();
            stream.close(None).unwrap();
        }
    }
}
