use std::hash::RandomState;

use crate::{server::ConnectionId, NetComponent, NetMessage};
use bevy::{ecs::system::SystemParam, prelude::*};
use dashmap::DashMap;


/// A resource containing outbound nextwork messages that have not yet been sent.
/// Used to fragment and buffer packets so the route doesn't become congested.
/// If sending these messages reliably, consider using a [web_transport::SendStream] directly.
#[derive(Resource, Default)]
pub(crate) struct NetworkSendBuffer {
    pub map: DashMap<ConnectionId, Vec<Box<[u8]>>>
}


/// A buffer containing all of the network messages.
/// Used by the receiving system to process messages associated wtih a given message type, 'u16'.
#[derive(Resource, Default)]
pub(crate) struct NetworkReceiveBuffer {
    pub map: DashMap<u16, Vec<Box<[u8]>>>
}

impl NetworkReceiveBuffer {
    fn next(&self, message_type: u16) -> Option<Box<[u8]>> {
        (*self.map.get_mut(&message_type).unwrap()).pop()
    }

    fn receive<T: NetMessage>(&self) -> DrainNetworkBufferIter<T> {
        let message_type = T::TYPE_ID.0;
        let Some(buffer) = self.map.get_mut(&message_type) else {
            panic!("");
        };

        DrainNetworkBufferIter {
            _p: std::marker::PhantomData,
            buffer,
        }
    }
}

struct DrainNetworkBufferIter<'a, T> {
    _p: std::marker::PhantomData<T>,
    buffer: dashmap::mapref::one::RefMut<'a, u16, Vec<Box<[u8]>>>,
}

impl<'a, T: NetMessage> Iterator for DrainNetworkBufferIter<'a, T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let bytes = self.buffer.pop()?;

            if let Ok(out) = bincode::deserialize(bytes.as_ref()) {
                return Some(out)
            } else {
                warn!("failed to deserialize \"{}\"", std::any::type_name::<T>())
            }
        }
    }
}


#[derive(SystemParam)]
pub struct ReceiveNetMessages<'w> {
    buffer: Res<'w, NetworkReceiveBuffer>
}

impl<'w> ReceiveNetMessages<'w> {
    pub fn receive<T: NetMessage>(&self) -> impl Iterator<Item = T> + '_ {
        self.buffer.receive()
    }
}
