use std::{marker::PhantomData, io::Cursor};
use bevy::{prelude::*, ecs::{component::ComponentId, archetype::ArchetypeId}, ptr::Ptr};
use dipa::{Diffable, Patchable};
use bincode::{de::read::IoReader, DefaultOptions};
use serde::{Deserialize, Serialize, de::DeserializeOwned, Serializer, Deserializer};

use crate::NetSync;

//The NetComponent trait will require functions that follow this specification.
type SerializeFn<S> = for<'a> unsafe fn(Ptr<'a>, S) -> Result<<S as Serializer>::Ok, <S as Serializer>::Error>;

/// A type agnostic serializer function. Can be used in a [SerializeFn] context.
/// Useful for serializing a component with a Ptr and component ID.
fn type_erased_serialize<T: Serialize, S: Serializer>(
  ptr: Ptr<'_>,
  s: S,
) -> Result<S::Ok, S::Error> {
  let reference = unsafe { &*ptr.as_ptr().cast::<T>() };
  reference.serialize(s)
}

type DeserializeFn<'a, S> = for<'b> unsafe fn(Ptr<'b>, S) -> Result<(), <S as Deserializer>::Error>;


/// A type agnostic deserializer function. Can be used in a [DeserializeFn] context.
/// Useful for deserializing a component with a Ptr and component ID.
fn type_erased_deserialize<'a, T: Deserialize<'a>, S: Deserializer<'a>>(
  ptr: Ptr<'_>,
  s: S,
) -> Result<(), S::Error> {
  let reference = unsafe { &mut *ptr.as_ptr().cast::<T>() };
  *reference = T::deserialize(s)?;
  Ok(())
}

type DeltaSerializeFn<'a, S> = for<'b> unsafe fn(Ptr<'b>, Ptr<'b>, S) -> Result<<S as Serializer>::Ok, <S as Serializer>::Error>;

/// A type agnostic delta serializer. Can be used in a [DeltaSerializeFn] context.
/// Useful for serializing a component with a Ptr and component ID.
fn type_erased_delta_serialize<'a, T: Serialize, S: Serializer> (
    ptr: Ptr<'_>,
    old_state: Ptr<'_>,
    s: S,
  ) -> Result<S::Ok, S::Error>
  where
    T: Diffable<'a, 'a, T> + 'a,
    T::Delta: Serialize,
    
   {
    let reference = unsafe { &*ptr.as_ptr().cast::<T>() };
    let old_ref = unsafe { &*old_state.as_ptr().cast::<T>() };
    let delta = old_ref.create_delta_towards(reference).delta;
    delta.serialize(s)
  }
  
type DeltaDeserializeFn<'a, S> = for<'b> unsafe fn(Ptr<'b>, S) -> Result<(), <S as Deserializer>::Error>;

/// A type agnostic delta deserializer. Can be used in a [DeltaDeserializeFn] context.
/// Useful for deserializing a component with a Ptr and component ID.
fn type_erased_delta_deserialize<'a, T: DeserializeOwned + Diffable<'a, 'a, T>, S: Deserializer<'a>>(
    ptr: Ptr<'_>,
    s: S,
  ) -> Result<(), S::Error> 
  where
  T: Diffable<'a, 'a, T> + 'a,
  T::Delta: DeserializeOwned,
  T: Patchable<T::Delta>
  {
    let reference = unsafe { &mut *ptr.as_ptr().cast::<T>() };
    
    let diff = T::Delta::deserialize(s)?;
    (*reference).apply_patch(diff);
    //*reference = T::deserialize(s)?;
    Ok(())
  }


/// A trait implemented on all types which can be replicated in some capacity.
/// This trait does not implement an interface for delta encoding or other optimizations.
pub trait NetComponent: Component + Serialize + DeserializeOwned { }

/// Contracts that a given NetComponent has delta encoding available as an optimization.
/// For types that implement this trait, Nevy will automatically use delta-encoding where applicable.
pub trait DeltaNetComponent<'a>: NetComponent { }

/// Implement the NetworkComponent trait for all components have a serde impl.
impl<T: Component + Serialize + DeserializeOwned> NetComponent for T { }

/// Implement the DeltaNetComponent for all dipa::Diffable types.
impl<'a, T: NetComponent + Diffable<'a, 'a, T>> DeltaNetComponent<'a> for T
where
    <T as Diffable<'a, 'a, T>>::DeltaOwned : DeserializeOwned,
    <T as Diffable<'a, 'a, T>>::Delta: serde::Serialize,
 { 
 }




//To dispatch network events, we need the serializer.
pub(crate) struct NetDispatchEntry<'a> {
  pub id: ComponentId,
  pub serializer: SerializeFn<&'a mut bincode::Serializer<Cursor<Vec<u8>>, DefaultOptions>>,
  pub delta_serializer: Option<bincode::Deserializer<Cursor<Vec<u8>>, DefaultOptions>>
}

pub(crate) struct NetRecieverEntry<'a> {
  id: ComponentId,
  deserializer: DeserializeFn<'a,  &'a mut bincode::Deserializer<IoReader<Cursor<Vec<u8>>>, DefaultOptions>>,
  delta_deserializer: Option<DeltaDeserializeFn<'a, &'a mut bincode::Deserializer<IoReader<Cursor<Vec<u8>>>, DefaultOptions>>>
}



pub(crate) struct NetDispatchMetadata<'a> {
  pub id: ArchetypeId,
  //TODO: use smallvec here.
  pub components: Vec<NetDispatchEntry<'a>>
}

pub(crate) struct NetReceiverMetadata<'a> {
  pub id: ArchetypeId,
  //TODO: use smallvec here.
  pub components: Vec<NetRecieverEntry<'a>>
}

/// A resource responsible for holding relevant data to the archetype dispatcher.
/// This resource contains metadata used to enumerate and read networked archetypes.
#[derive(Resource)]
pub(crate) struct ArchetypeDispatcher<'a> {
  pub archetypes: Vec<NetDispatchMetadata<'a>>,
}

#[derive(Resource)]
pub(crate) struct ArchetypeReceiver<'a> {
  pub archetypes: Vec<NetReceiverMetadata<'a>>,
}

/// A resource that contains the buffers used for networking.
/// These buffers hold outgoing and incoming network packets.
#[derive(Component)]
struct NetBuffers {
  outbound: Cursor<Vec<u8>>,
  inbound: Cursor<Vec<u8>>
}


/// This will register the archetype maintenance methods for [T] on the given app.
pub fn register_net_component<T: NetComponent>(app: &mut App) -> &mut App {
  //Add the archetype cache maintenance systems.
  app.add_systems(Update, register_archetype::<T>);
  app.add_systems(Update, remove_archetype::<T>);
  app
}

/// This system is responsible for detecting and registering when an entity moves into a new archetype.
fn register_archetype<T: NetComponent>(
  world: &World, 
  q: Query<EntityRef, Added<NetSync<T>>>,
  mut dispatcher: ResMut<ArchetypeDispatcher<'static>>,
  mut receiver: ResMut<ArchetypeReceiver<'static>>
) {
    let comp_id = world.component_id::<T>().expect("Component ID should exist for T since its being registered.");

    for ent_ref in q.iter() {
      let archetype_id = ent_ref.archetype().id();

      //Verify that the archetype is registered.
      if let Some(archetype) = dispatcher.archetypes.iter_mut().find(|arch| arch.id == archetype_id) {
        //Verify that this component doesn't already exist on this archetype. Add if it doesn't.
        if !archetype.components.iter().any(|comp| comp.id == comp_id) {
          archetype.components.push(NetDispatchEntry {
            id: comp_id,
            serializer: type_erased_serialize::<T, _>,
            delta_serializer: None
          });
        }
      }
      else {
        dispatcher.archetypes.push(NetDispatchMetadata {
          id: archetype_id,
          components: vec![NetDispatchEntry {
            id: comp_id,
            serializer: type_erased_serialize::<T, _>,
            delta_serializer: None
          }]
        }); 
      }

      if let Some(archetype) = receiver.archetypes.iter_mut().find(|arch| arch.id == archetype_id) {
        //Verify that this component doesn't already exist on this archetype. Add if it doesn't.
        if !archetype.components.iter().any(|comp| comp.id == comp_id) {
          archetype.components.push(NetRecieverEntry {
            id: comp_id,
            deserializer: type_erased_deserialize::<T, _>,
            delta_deserializer: None
          });
        }
      }
      else {
        receiver.archetypes.push(NetReceiverMetadata {
          id: archetype_id,
          components: vec![NetRecieverEntry {
            id: comp_id,
            deserializer: type_erased_deserialize::<T, _>,
            delta_deserializer: None
          }]
        }); 
      }
    }
}

/// This system is responsible for detecting when a component in an archetype is no longer networked.
/// When a entity removes a networked component, it has to be removed from its corresponding archetype.
fn remove_archetype<T: NetComponent>(
  world: &World, 
  mut q: RemovedComponents<NetSync<T>>,
  mut dispatcher: ResMut<ArchetypeDispatcher<'static>>,
  mut receiver: ResMut<ArchetypeReceiver<'static>>,
) {
  let comp_id = world.component_id::<T>().expect("Component ID should exist for T since its being used.");

  for ent in q.read() {
      let archetype_id = world.entity(ent).archetype().id();

      // Remove the component from the dispatcher
      if let Some(archetype) = dispatcher.archetypes.iter_mut().find(|arch| arch.id == archetype_id) {
          archetype.components.retain(|comp| comp.id != comp_id);

          // If no components left, remove the archetype
          if archetype.components.is_empty() {
              dispatcher.archetypes.retain(|arch| arch.id != archetype_id);
          }
      }

      // Remove the component from the receiver
      if let Some(archetype) = receiver.archetypes.iter_mut().find(|arch| arch.id == archetype_id) {
          archetype.components.retain(|comp| comp.id != comp_id);

          // If no components left, remove the archetype
          if archetype.components.is_empty() {
              receiver.archetypes.retain(|arch| arch.id != archetype_id);
          }
      }
  }
}
