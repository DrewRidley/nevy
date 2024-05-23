
use std::net::{SocketAddr, UdpSocket};

use bevy::{prelude::*, tasks::*};
use rustls::client::danger::DangerousClientConfig;
use rustls::server::NoClientAuth;

#[derive(Component)]
pub struct Endpoint {
    endpoint: quinn::Endpoint,
    listener: Task<quinn::Connection>,
}

#[derive(Debug)]
struct EndpointRuntime {

}

#[derive(Debug)]
struct EndpointTimer(std::time::Instant);

impl quinn::AsyncTimer for EndpointTimer {
    fn reset(self: std::pin::Pin<&mut Self>, i: std::time::Instant) {
        self.get_mut().0 = i;
    }

    fn poll(self: std::pin::Pin<&mut Self>, _cx: &mut std::task::Context) -> std::task::Poll<()> {
        if std::time::Instant::now() > self.0 {
            std::task::Poll::Ready(())
        } else {
            std::task::Poll::Pending
        }
    }
}


impl quinn::Runtime for EndpointRuntime {
    fn new_timer(&self, i: std::time::Instant) -> std::pin::Pin<Box<dyn quinn::AsyncTimer>> {
        Box::pin(EndpointTimer(i))
    }

    fn spawn(&self, future: std::pin::Pin<Box<dyn futures_lite::prelude::Future<Output = ()> + Send>>) {
        AsyncComputeTaskPool::get().spawn(future).detach();
    }

    fn wrap_udp_socket(&self, t: std::net::UdpSocket) -> std::io::Result<std::sync::Arc<dyn quinn::AsyncUdpSocket>> {
        todo!()
    }
}

impl Endpoint {
    fn new(addr: SocketAddr) -> Result<Endpoint, ()> {
        let socket = UdpSocket::bind(addr).expect("Failed to bind to socket");

        let config = todo!();

        let runtime = todo!();

        let endpoint = quinn::Endpoint::new(config, None, socket, runtime);
    }

    fn connect(&mut self) {

    }

    fn spawn_listen(endpoint: &quinn::Endpoint) -> Task<quinn::Connection> {
        AsyncComputeTaskPool::get().spawn(Self::listen(endpoint.clone()))
    }

    async fn listen(endpoint: quinn::Endpoint) -> quinn::Connection {
        loop {
            let Some(incoming) = endpoint.accept().await else {
                debug!("Failed to accept incoming CONNECT");
                continue;
            };

            let Ok(connecting) = incoming.accept() else {
                debug!("Failed to negotiate incoming stream");
                continue;
            };

            let Ok(connection) = connecting.await else {
                debug!("Failed to finalize incoming connection");
                continue;
            };

            return connection;
        }
    }

    fn poll_new_connection(&mut self) -> Option<quinn::Connection> {
        if let Some(new_connection) = block_on(poll_once(&mut self.listener)) {
            self.listener = Self::spawn_listen(&self.endpoint);

            return Some(new_connection)
        }

        None
    }
}


fn update_endpoints(
    mut endpoint_q: Query<&mut Endpoint>,
) {
    for mut endpoint in endpoint_q.iter_mut() {
        while let Some(new_connection) = endpoint.poll_new_connection() {

        }
    }
}

