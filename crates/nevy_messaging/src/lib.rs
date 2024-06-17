use std::marker::PhantomData;

use bevy::{ecs::schedule::ScheduleLabel, prelude::*, utils::intern::Interned};
use serde::de::DeserializeOwned;

pub mod deserialize;
pub mod serialize;

pub use deserialize::*;
pub use serialize::*;
