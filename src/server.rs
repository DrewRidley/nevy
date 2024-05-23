use std::{io::Read, sync::Arc};

use anyhow::Context;
use bevy::{prelude::*, tasks::{block_on, AsyncComputeTaskPool, IoTaskPool}};
use bevy_async_task::AsyncTaskRunner;
use quinn::{Endpoint};
use slotmap::SlotMap;

//pub use crate::streaming::server::register_net_component;

fn create_server() -> anyhow::Result<Endpoint> {
    // Read the PEM certificate chain
    trace!("Loading certificate chain for server.");
    let chain = std::fs::File::open("/etc/letsencrypt/live/dev.drewridley.com/fullchain.pem").context("failed to open cert file")?;
    let mut chain = std::io::BufReader::new(chain);

    let chain: Vec<rustls::pki_types::CertificateDer> = rustls_pemfile::certs(&mut chain)
        .collect::<Result<_, _>>()
        .context("failed to load certs")?;

    trace!("Loading private key for server.");
    let mut keys = std::fs::File::open("/etc/letsencrypt/live/dev.drewridley.com/privkey.pem").context("failed to open key file")?;

    let mut buf = Vec::new();
    keys.read_to_end(&mut buf)?;

    let key = rustls_pemfile::private_key(&mut std::io::Cursor::new(&buf))
        .context("failed to load private key")?
        .context("missing private key")?;

    debug!("Loaded certificate files.");

    let mut config = rustls::ServerConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_protocol_versions(&[&rustls::version::TLS13])?
    .with_no_client_auth()
    .with_single_cert(chain, key)?;


    config.max_early_data_size = u32::MAX;
    config.alpn_protocols = vec![web_transport_quinn::ALPN.to_vec()]; // this one is important

    let config: quinn::crypto::rustls::QuicServerConfig = config.try_into()?;


    let mut transport_config = quinn::TransportConfig::default();
    transport_config.enable_segmentation_offload(false);

    let mut config = quinn::ServerConfig::with_crypto(Arc::new(config));
    config.transport = Arc::new(transport_config);

    let listen_addr = "0.0.0.0:443".parse().expect("Faield to parse listen addr");

    let server = quinn::Endpoint::server(config, listen_addr)?;

    trace!("Constructed quinn server...");

    Ok(server)
}

use anyhow::Error;
use web_transport::SendStream;

use crate::messages::{NetworkReceiveBuffer, NetworkSendBuffer};

///Given a server, this listens for new connections.
async fn listen_on_server(server: Endpoint) -> anyhow::Result<ServerConnection> {
    trace!("Waiting for next connection request");

    let conn = server.accept().await.ok_or_else(|| Error::msg("Failed to accept incoming connection!"))?;

    let conn = conn.await.context("Failed to accept connection request")?;

    trace!("Received incoming request!");

    let request = web_transport_quinn::accept(conn).await?;

    trace!("Accepted connection request!");
    //Create a ubiqutous session.
    //The universal session api is used everywhere...
    let session = request.ok().await.context("Failed to accept session!")?;

    trace!("Accpeted incoming session... \n Opening streams...");
    let (send_stream, recv_stream) = session.open_bi().await.context("failed to open stream.")?;

    trace!("Opened streams.");

    Ok(ServerConnection { session, send_stream, recv_stream })
}

#[derive(Event)]
pub struct ClientConnected(pub ConnectionId);

#[derive(Event)]
pub struct ClientDisconnect(pub ConnectionId);

struct ServerConnection {
    session: web_transport_quinn::Session,
    send_stream: web_transport_quinn::SendStream,
    recv_stream: web_transport_quinn::RecvStream,
}


slotmap::new_key_type! {
    //A key used to describe a particular connection.
    //These keys are ephemeral and may be re-used later.
    pub struct ConnectionId;
}

// The intended recipient of a particular message.
pub enum MessageTarget {
    One(ConnectionId),
    Many(Vec<ConnectionId>),
    All
}


#[derive(Resource, Default)]
struct Connections(SlotMap<ConnectionId, ServerConnection>);

// Creates a server and listens for any new connections.
// This can be installed in a SystemSet or with run_criteria to control when connections happen.
fn server_listener_system(
    mut executor: AsyncTaskRunner<anyhow::Result<ServerConnection>>,
    mut endpoint: Local<Option<Endpoint>>,
    mut connections: ResMut<Connections>,
) {
    let server: &mut Endpoint = endpoint.get_or_insert_with(|| create_server().unwrap());

    match executor.poll() {
        bevy_async_task::AsyncTaskStatus::Idle => executor.start(listen_on_server(server.clone())),
        bevy_async_task::AsyncTaskStatus::Pending => {}
        bevy_async_task::AsyncTaskStatus::Finished(conn) => {
            match conn {
                Ok(c) => {
                    connections.0.insert(c);
                }
                Err(e) => {
                    debug!("Received an error on a connection attempt: {:?}", e);
                },
            }
        }
    }
}



fn send_message_system(
    buffers: Res<NetworkSendBuffer>,
    mut connections: ResMut<Connections>
) {
    buffers.map.alter_all(|id, mut queue| {
        for message in queue {
            let conn = connections.0.get_mut(*id).unwrap();
            block_on(conn.send_stream.write(&message)).unwrap();
        }
        Vec::new()
    });
}

struct ReceiveMessageBuffer(Box<[u8]>);
impl Default for ReceiveMessageBuffer {
    fn default() -> Self {
        ReceiveMessageBuffer(vec![0; u16::MAX.into()].into_boxed_slice())
    }
}

fn receive_message_sysetm(
    buffers: Res<NetworkReceiveBuffer>,
    mut connections: ResMut<Connections>,
    mut buffer: Local<ReceiveMessageBuffer>,
) {
    let buffer = &mut buffer.0;
    for (id, mut connection) in connections.0.iter_mut() {

        let mut message_len_buffer = [0; 2];
        block_on(connection.recv_stream.read_exact(&mut message_len_buffer)).unwrap();


        let anyhow::Result::Ok(bytes) = block_on(connection.recv_stream.read(buffer)) else {
            error!("failed to read from {:?}", id);
            continue;
        };

        if let Some(bytes) = bytes {
            let data = Box::from(&buffer[..bytes]);

        }
    }
}


pub struct ServerPlugin;
impl Plugin for ServerPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Connections::default());
        app.add_systems(Update, server_listener_system);
    }

    fn is_unique(&self) -> bool {
        false
    }
}