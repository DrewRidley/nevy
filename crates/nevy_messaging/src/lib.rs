use std::marker::PhantomData;

use bevy::{ecs::schedule::ScheduleLabel, prelude::*};
use deserialize::MessageDeserializationPlugin;
use serde::{de::DeserializeOwned, Serialize};
use serialize::MessageSerializationPlugin;

pub mod deserialize;
pub mod serialize;

pub mod prelude {
    pub use crate::serialize::{MessageId, MessageSerializationPlugin, MessageStreamState};

    pub use crate::deserialize::{
        EndpointMessagingHeader, MessageDeserializationPlugin, ReceivedMessages,
    };

    pub use crate::ProtocolBuilder;
}

pub struct ProtocolBuilder<C> {
    messages: Vec<Box<dyn MessageAdder<C>>>,
}

trait MessageAdder<C> {
    fn add_serializer(&self, plugin: &mut MessageSerializationPlugin<C>);

    fn add_deserializer(&self, plugin: &mut MessageDeserializationPlugin<C>);
}

struct MessageAdderType<T> {
    _p: PhantomData<T>,
}

impl<C: Component> ProtocolBuilder<C> {
    pub fn new() -> Self {
        ProtocolBuilder {
            messages: Vec::new(),
        }
    }

    pub fn add_message<T: Serialize + DeserializeOwned + Send + Sync + 'static>(
        &mut self,
    ) -> &mut Self {
        self.messages
            .push(Box::new(MessageAdderType::<T> { _p: PhantomData }));
        self
    }

    pub fn build_serialization(&self) -> MessageSerializationPlugin<C> {
        let mut plugin = MessageSerializationPlugin::new();

        for adder in self.messages.iter() {
            adder.add_serializer(&mut plugin);
        }

        plugin
    }

    pub fn build_deserialization(
        &self,
        schedule: impl ScheduleLabel,
    ) -> MessageDeserializationPlugin<C> {
        let mut plugin = MessageDeserializationPlugin::new(schedule);

        for adder in self.messages.iter() {
            adder.add_deserializer(&mut plugin);
        }

        plugin
    }

    pub fn build_symmetric(
        &self,
        schedule: impl ScheduleLabel,
    ) -> (
        MessageSerializationPlugin<C>,
        MessageDeserializationPlugin<C>,
    ) {
        (
            self.build_serialization(),
            self.build_deserialization(schedule),
        )
    }
}

impl<C: Component, T: Serialize + DeserializeOwned + Send + Sync + 'static> MessageAdder<C>
    for MessageAdderType<T>
{
    fn add_serializer(&self, plugin: &mut MessageSerializationPlugin<C>) {
        plugin.add_message::<T>();
    }

    fn add_deserializer(&self, plugin: &mut MessageDeserializationPlugin<C>) {
        plugin.add_message::<T>();
    }
}
