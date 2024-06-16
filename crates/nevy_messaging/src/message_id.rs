use std::{collections::VecDeque, marker::PhantomData};

use bevy::{ecs::schedule::ScheduleLabel, prelude::*, utils::intern::Interned};
use serde::de::DeserializeOwned;

pub struct MessageDeserializationPlugin {
    schedule: Interned<dyn ScheduleLabel>,
    messages: Vec<Box<dyn MessageIdBuilder>>,
}

trait MessageIdBuilder: Send + Sync + 'static {
    fn build(&self, schedule: Interned<dyn ScheduleLabel>, message_id: u16, app: &mut App);
}

struct MessageIdBuilderType<T>(PhantomData<T>);

impl MessageDeserializationPlugin {
    pub fn new(schedule: impl ScheduleLabel) -> Self {
        MessageDeserializationPlugin {
            schedule: schedule.intern(),
            messages: Vec::new(),
        }
    }
}

impl Default for MessageDeserializationPlugin {
    fn default() -> Self {
        MessageDeserializationPlugin::new(PreUpdate)
    }
}

impl MessageDeserializationPlugin {
    pub fn add_message<T: DeserializeOwned + Send + Sync + 'static>(&mut self) -> &mut Self {
        self.messages
            .push(Box::new(MessageIdBuilderType::<T>(PhantomData)));

        self
    }
}

impl Plugin for MessageDeserializationPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ReceivedSerializedMessages>();

        for (message_id, builder) in self.messages.iter().enumerate() {
            builder.build(self.schedule, message_id as u16, app);
        }
    }
}

impl<T: DeserializeOwned + Send + Sync + 'static> MessageIdBuilder for MessageIdBuilderType<T> {
    fn build(&self, schedule: Interned<dyn ScheduleLabel>, message_id: u16, app: &mut App) {
        app.insert_resource(MessageId::<T> {
            _p: PhantomData,
            message_id,
        });

        app.insert_resource(ReceivedMessages::<T> {
            messages: VecDeque::default(),
        });

        app.add_systems(schedule, deserialize_messages::<T>);
    }
}

#[derive(Resource, Default)]
pub(crate) struct ReceivedSerializedMessages {
    buffers: Vec<VecDeque<Box<[u8]>>>,
}

impl ReceivedSerializedMessages {
    pub fn push_message(&mut self, message_id: u16, message: Box<[u8]>) {
        let buffer = loop {
            if let Some(buffer) = self.buffers.get_mut(message_id as usize) {
                break buffer;
            } else {
                self.buffers.push(VecDeque::new());
            }
        };

        buffer.push_back(message);
    }

    pub fn poll_message_received(&mut self, message_id: u16) -> Option<Box<[u8]>> {
        let buffer = self.buffers.get_mut(message_id as usize)?;
        buffer.pop_front()
    }
}

#[derive(Resource)]
pub(crate) struct MessageId<T> {
    _p: PhantomData<T>,
    message_id: u16,
}

impl<T> MessageId<T> {
    pub fn get(&self) -> u16 {
        self.message_id
    }
}

#[derive(Resource, Default)]
pub struct ReceivedMessages<T> {
    messages: VecDeque<T>,
}

impl<T> ReceivedMessages<T> {
    pub fn drain(&mut self) -> impl Iterator<Item = T> + '_ {
        self.messages.drain(..)
    }
}

fn deserialize_messages<T: DeserializeOwned + Send + Sync + 'static>(
    message_id: Res<MessageId<T>>,
    mut serialized_messages: ResMut<ReceivedSerializedMessages>,
    mut deserialized_messages: ResMut<ReceivedMessages<T>>,
) {
    while let Some(bytes) = serialized_messages.poll_message_received(message_id.get()) {
        let Ok(deserialized) = bincode::deserialize(bytes.as_ref()) else {
            warn!(
                "failed to deserialize a \"{}\" message",
                std::any::type_name::<T>()
            );
            continue;
        };

        deserialized_messages.messages.push_back(deserialized);
    }
}
