use bevy::ptr::Ptr;
use serde::{Serializer, Serialize, Deserializer, Deserialize};


/// A type agnostic serializer function. Can be used in a [SerializeFn] context.
/// Useful for serializing a component with a Ptr and component ID.
pub unsafe fn type_erased_serialize<T: Serialize, S: Serializer>(
  ptr: Ptr<'_>,
  s: S,
) -> Result<S::Ok, S::Error> {
  let reference = unsafe { &*ptr.as_ptr().cast::<T>() };
  reference.serialize(s)
}


/// A type agnostic deserializer function. Can be used in a [DeserializeFn] context.
/// Useful for deserializing a component with a Ptr and component ID.
pub unsafe fn type_erased_deserialize<'a, T: Deserialize<'a>, D: Deserializer<'a>>(
  ptr: Ptr<'_>,
  de: D,
) -> Result<(), D::Error> {
  let reference = unsafe { &mut *ptr.as_ptr().cast::<T>() };
  *reference = T::deserialize(de)?;
  Ok(())
}