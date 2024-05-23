use bevy::{prelude::*, utils::HashMap};
use serde::{Deserialize, Serialize};

//use crate::{messages::NetworkReceiveBuffer, Net, NetComponent};



// fn stream_deleted_components<T: NetComponent>(
//     mut removals: RemovedComponents<Net<T>>,
//     outbound: Res<NetworkSendBuffer>
// ) {
//     for entity in removals.read() {

//         // sender.0.try_send(NetworkStreamMessage {
//         //     entity: entity.to_bits(),
//         //     event: ComponentUpdate::Delete(T::TYPE_ID.0)
//         // }.as_broadcast(MessageTarget::All).expect("Failed to serialize network message!")).expect("Error sending message!");
//     }
// }

// fn stream_new_components<T: NetComponent>(
//     additions: Query<(bevy::ecs::entity::Entity, &Net<T>), Added<Net<T>>>,
//     sender: Res<NetworkSender>
// ) {
//     for (entity, state) in additions.iter() {
//         let payload = bincode::serialize(&state.0).expect("Failed to serialize component state...");

//         sender.0.try_send(NetworkStreamMessage {
//             entity: entity.to_bits(),
//             event: ComponentUpdate::Update(T::TYPE_ID.0, payload)
//         }.as_broadcast(MessageTarget::All).expect("Failed to serialize network message!")).expect("Failed to send message");
//     }
// }

// fn stream_component_changes<T: NetComponent>(
//     changes: Query<(bevy::ecs::entity::Entity, &Net<T>), bevy::ecs::query::Changed<Net<T>>>,
//     sender: Res<NetworkSender>
// ) {
//     for (entity, state) in changes.iter() {
//         let payload = bincode::serialize(&state.0).expect("Failed to serialize component state...");

//         sender.0.try_send(NetworkStreamMessage {
//             entity: entity.to_bits(),
//             event: ComponentUpdate::Update(T::TYPE_ID.0, payload)
//         }.as_broadcast(MessageTarget::All).expect("Failed to serialize network message!")).expect("Failed to send message");
//     }
// }

// fn receive_streamed_changes<T: NetComponent>(
//     mut receiver: ResMut<ColumnSortedComponentUpdates>,
//     mut q: Query<&mut Net<T>>,
//     mut cmds: Commands
// ) {
//     while let Some(update) = receiver.next_stream_update(T::TYPE_ID.0) {
//         match update.event {
//             ComponentUpdate::Update(_, data) => {

//                 //TODO: we should probably match an update with a new entity.

//                 if let Ok(typed_data) = bincode::deserialize::<T>(&data) {
//                     match q.get_mut(Entity::from_bits(update.entity)) {
//                         Ok(mut state) => {
//                             //We already have this entity and component, so its a change.
//                             *state = Net(typed_data);
//                         },
//                         Err(e) => {
//                             match e {
//                                 bevy::ecs::query::QueryEntityError::QueryDoesNotMatch(_) => {
//                                     //This is a new component, so we need to add the component.
//                                     cmds.entity(Entity::from_bits(update.entity)).insert(Net(typed_data));
//                                 },
//                                 bevy::ecs::query::QueryEntityError::NoSuchEntity(_) => {
//                                     //This entity has not been replicated yet to this client.
//                                     cmds.get_or_spawn(Entity::from_bits(update.entity)).insert(Net(typed_data));
//                                 },
//                                 bevy::ecs::query::QueryEntityError::AliasedMutability(_) => panic!("Double mutable alias!"),
//                             }
//                         },
//                     }
//                 }
//             },
//             ComponentUpdate::Delete(_) => {

//             },
//         }
//         if let Ok(ent_state) = q.get_mut(Entity::from_bits(update.entity)) {

//         }
//     }
// }


// #[derive(Serialize, Deserialize)]
// enum StreamEvent {
//     SpawnEntity,
//     AddComponent(u16, Vec<u8>),
//     UpdateComponent(u16, Vec<u8>),
//     DeleteComponent(u16),
//     DeleteEntity
// }

// impl StreamEvent {
//     fn relevant_component(&self) -> Option<u16> {
//         match self {
//             Self::AddComponent(id, _) => Some(*id),
//             Self::UpdateComponent(id, _) => Some(*id),
//             Self::DeleteComponent(id) => Some(*id),
//             _ => None
//         }
//     }
// }

// #[derive(Serialize, Deserialize)]
// struct NetworkStreamMessage {
//     /// The index of the entity this message concerns.
//     entity: u64,
//     event: StreamEvent
// }

// impl NetworkStreamMessage {
//     //Will convert this stream message into a broadcast message.
//     fn as_broadcast(&self, dest: MessageTarget) -> bincode::Result<OutboundNetMessage> {
//         Ok(OutboundNetMessage {
//             id: 0,
//             target: MessageTarget::All,
//             data: bincode::serialize(&self)?,
//         })
//     }
// }

// pub mod server {
//     use bevy::app::App;

//     use crate::{Net, NetComponent};

//     use super::{stream_component_changes, stream_deleted_components, stream_new_components};

//     /// Associates a given [NetComponent] with networked behavior.
//     pub fn register_net_component<T: NetComponent>(app: &mut App) {
//         //Ensures that the Net<T> has been initialized.
//         app.world.init_component::<Net<T>>();

//         //Register the required systems for this component.
//         app.add_systems(bevy::app::Update, (stream_new_components::<T>, stream_deleted_components::<T>));
//         app.add_systems(bevy::app::Update, stream_component_changes::<T>);
//     }
// }

// pub mod client {
//     use bevy::app::App;
//     use crate::{Net, NetComponent};

//     use super::{receive_streamed_changes, ColumnSortedComponentUpdates};


//     pub fn register_net_component<T: NetComponent>(app: &mut App) {
//         app.world.init_component::<Net<T>>();

//         //The map used to efficiently pull updates back into the component tables.
//         app.insert_resource(ColumnSortedComponentUpdates::default());
//         app.add_systems(bevy::app::Update, receive_streamed_changes::<T>);
//     }
// }


// pub fn apply_networked_updates<T: NetComponent>(
//     messages: Res<NetworkReceiveBuffer>,
//     q: Query<&mut Net<T>>
// ) {

// }



// pub mod server {
//     use crate::NetComponent;

//     pub fn register_net_component<T: NetComponent>(app: &mut bevy::app::App) {

//     }
// }

// pub mod client {
//     use crate::NetComponent;

//     pub fn register_net_component<T: NetComponent>(app: &mut bevy::app::App) {

//     }
// }