use std::marker::PhantomData;

use bevy::prelude::*;
use serde::Serialize;

/// Adds message serialization functionality
pub struct MessageSerializationPlugin<C> {
    _p: PhantomData<C>,
    messages: Vec<Box<dyn MessageIdBuilder<C>>>,
}

trait MessageIdBuilder<C>: Send + Sync + 'static {
    fn build(&self, message_id: u16, app: &mut App);
}

struct MessageIdBuilderType<T> {
    _p: PhantomData<T>,
}

impl<C> MessageSerializationPlugin<C> {
    pub fn new() -> Self {
        MessageSerializationPlugin {
            _p: PhantomData,
            messages: Vec::new(),
        }
    }
}

impl<C: Component> MessageSerializationPlugin<C> {
    /// adds a message type to the plugin, assigning it the next message id
    pub fn add_message<T: Serialize + Send + Sync + 'static>(&mut self) -> &mut Self {
        self.messages
            .push(Box::new(MessageIdBuilderType::<T> { _p: PhantomData }));

        self
    }
}

impl<C: Component> Plugin for MessageSerializationPlugin<C> {
    fn build(&self, app: &mut App) {
        for (message_id, builder) in self.messages.iter().enumerate() {
            builder.build(message_id as u16, app);
        }
    }
}

impl<T: Serialize + Send + Sync + 'static, C: Component> MessageIdBuilder<C>
    for MessageIdBuilderType<T>
{
    fn build(&self, message_id: u16, app: &mut App) {}
}
