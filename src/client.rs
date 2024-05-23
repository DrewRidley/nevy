use bevy::prelude::*;
use bevy_async_task::AsyncTaskRunner;
use std::{borrow::BorrowMut, str::FromStr, sync::Arc};
use url::Url;
use web_transport::{RecvStream, SendStream, Session};

//pub use crate::streaming::client::register_net_component;


async fn connect_client(host: String) -> anyhow::Result<ClientConnection> {
    let mut session: web_transport::Session;

    //On wasm32, we have to use the wasm layer.
    #[cfg(all(target_arch = "wasm32", not(target_os = "wasi")))]
    {
        session = web_transport::Session::new(host)?;
    }

    //For native platforms, we use the quinn abstraction.
    #[cfg(any(not(target_arch = "wasm32"), target_os = "wasi"))]
    {

        let arc_crypto_provider = std::sync::Arc::new(rustls::crypto::ring::default_provider());

        //We use the platform verifier here to have the client validate a certificate against the CA.
        let mut config = rustls_platform_verifier::tls_config_with_provider(arc_crypto_provider)?;

        //TODO: add the local cert manually to the client to validate it for development.

        //Add ALPN so the connection is established.
        config.alpn_protocols = vec![web_transport_quinn::ALPN.to_vec()];

        let config: quinn::crypto::rustls::QuicClientConfig = config.try_into()?;
        let config = quinn::ClientConfig::new(Arc::new(config));


        let mut client = quinn::Endpoint::client("[::]:0".parse()?)?;
        client.set_default_client_config(config);

        client.accept();

        let url = Url::from_str(&host)?;
        session = web_transport_quinn::connect(&client, &url).await?.into();

    }

    let streams = session.open_bi().await?;
    return Ok(ClientConnection {
            session,
            streams
    });
}


#[derive(Event)]
pub struct DisconnectClient;

#[derive(Event)]
pub struct ConnectClient(pub String);


struct ClientConnection {
    session: Session,
    streams: (SendStream, RecvStream)
}

#[derive(Resource, Default)]
struct ClientTransport(Option<ClientConnection>);


fn connect_client_system(
    mut executor: AsyncTaskRunner<anyhow::Result<ClientConnection>>,
    mut connection: ResMut<ClientTransport>,
    mut connect_events: EventReader<ConnectClient>
) {
    match executor.poll() {
        bevy_async_task::AsyncTaskStatus::Idle => {
            for connect in connect_events.read() {
                executor.start(connect_client(connect.0.clone()));
                if let Some(conn) = connection.0.take() {
                    warn!("Closed connection to faciltiate new connection.");
                    conn.session.close(1, "Closed to replace connection");
                }
            }
        },
        bevy_async_task::AsyncTaskStatus::Pending => {
            //We are actively awaiting a connection attempt.
        },
        bevy_async_task::AsyncTaskStatus::Finished(conn) => {
            match conn {
                Ok(conn) => {
                    debug!("Successfully connected to remote host.");
                    *connection = ClientTransport(Some(conn))
                },
                Err(e) => {
                    warn!("Received error on a connection attempt: {}", e);
                },
            }
        },
    }
}

async fn message_processor_task() {

}

fn send_message_sys(
    connected: Res<ClientTransport>,
    mut executor: AsyncTaskRunner<()>
) {
    match executor.poll() {
        bevy_async_task::AsyncTaskStatus::Idle => {
            if connected.0.is_some() {
                executor.start(message_processor_task());
            }
        },
        bevy_async_task::AsyncTaskStatus::Pending => {},
        bevy_async_task::AsyncTaskStatus::Finished(_) => {
            if connected.0.is_some() {
                executor.start(message_processor_task());
            }
        }
    }
}


pub struct ClientPlugin;

impl Plugin for ClientPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<ConnectClient>();
        app.add_event::<DisconnectClient>();
        app.insert_resource(ClientTransport::default());

        app.add_systems(Update, connect_client_system);
    }
}