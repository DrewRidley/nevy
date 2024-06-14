use std::any::Any;

use transport_interface::*;

use super::MismatchedStreamType;

/// type erased stream open description
///
/// because it is type erased it can be given to plugins to be able to open streams
///
/// if a plugin needs to open arbitrarily many streams from one description see [CloneableStreamDescription]
pub struct StreamDescription {
    description: Box<dyn Any>,
}

impl StreamDescription {
    pub fn new<S: StreamId>(description: S::OpenDescription) -> Self
    where
        S::OpenDescription: 'static,
    {
        StreamDescription {
            description: Box::new(description),
        }
    }

    pub(crate) fn downcast<T: 'static>(self) -> Result<T, MismatchedStreamType> {
        match self.description.downcast() {
            Ok(downcasted) => Ok(*downcasted),
            Err(_) => Err(MismatchedStreamType {
                expected: std::any::type_name::<T>(),
            }),
        }
    }
}

trait CloneableDescription {
    fn clone(&self) -> Box<dyn CloneableDescription>;

    fn into_any(self: Box<Self>) -> Box<dyn Any>;
}

/// type erased stream open description
///
/// because it is type erased it can be given to plugins to be able to open streams
///
/// this type implements `Into<StreamDescription>`
pub struct CloneableStreamDescription {
    description: Box<dyn CloneableDescription>,
}

impl<T: Clone + 'static> CloneableDescription for T {
    fn clone(&self) -> Box<dyn CloneableDescription> {
        Box::new(self.clone())
    }

    fn into_any(self: Box<Self>) -> Box<dyn Any> {
        self
    }
}

impl Clone for CloneableStreamDescription {
    fn clone(&self) -> Self {
        CloneableStreamDescription {
            description: self.description.clone(),
        }
    }
}

impl From<CloneableStreamDescription> for StreamDescription {
    fn from(value: CloneableStreamDescription) -> Self {
        StreamDescription {
            description: value.description.into_any(),
        }
    }
}

impl CloneableStreamDescription {
    pub fn new<S: StreamId>(description: S::OpenDescription) -> Self
    where
        S::OpenDescription: Clone + 'static,
    {
        CloneableStreamDescription {
            description: Box::new(description),
        }
    }
}
