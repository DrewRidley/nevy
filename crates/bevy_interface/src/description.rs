use std::any::Any;

use transport_interface::*;

use crate::MismatchedType;

/// type erased description use for operations that don't have type info
///
/// because it is type erased it can be given to plugins and they don't need to have type info
///
/// if a plugin needs to use a description more than once see [CloneableStreamDescription]
pub struct Description {
    description: Box<dyn Any>,
}

impl Description {
    /// creates a new description used for connecting on an endpoint `E`
    pub fn new_connect_description<E: Endpoint>(description: E::ConnectDescription) -> Self {
        Description {
            description: Box::new(description),
        }
    }

    /// creates a new description used for opening a stream of `S`
    pub fn new_open_description<S: StreamId>(description: S::OpenDescription) -> Self {
        Description {
            description: Box::new(description),
        }
    }

    /// creates a new description used for closing a send stream of `S`
    pub fn new_send_close_description<'s, S: StreamId>(
        description: <S::SendMut<'s> as SendStreamMut<'s>>::CloseDescription,
    ) -> Self {
        Description {
            description: Box::new(description),
        }
    }

    /// creates a new description used for closing a recv stream of `S`
    pub fn new_recv_close_description<S: StreamId>(description: S::OpenDescription) -> Self
    where
        for<'a> <S::RecvMut<'a> as RecvStreamMut<'a>>::CloseDescription: 'static,
    {
        Description {
            description: Box::new(description),
        }
    }

    pub(crate) fn downcast<T: 'static>(self) -> Result<T, MismatchedType> {
        match self.description.downcast() {
            Ok(downcasted) => Ok(*downcasted),
            Err(_) => Err(MismatchedType {
                expected: std::any::type_name::<T>(),
            }),
        }
    }
}

trait CloneableDescriptionInner {
    fn clone(&self) -> Box<dyn CloneableDescriptionInner>;

    fn into_any(self: Box<Self>) -> Box<dyn Any>;
}

/// type erased description used for opening and closing streams
///
/// because it is type erased it can be given to plugins and they don't need to have type info
///
/// this type implements Into<[Description]>
pub struct CloneableDescription {
    description: Box<dyn CloneableDescriptionInner>,
}

impl<T: Clone + 'static> CloneableDescriptionInner for T {
    fn clone(&self) -> Box<dyn CloneableDescriptionInner> {
        Box::new(self.clone())
    }

    fn into_any(self: Box<Self>) -> Box<dyn Any> {
        self
    }
}

impl Clone for CloneableDescription {
    fn clone(&self) -> Self {
        CloneableDescription {
            description: self.description.clone(),
        }
    }
}

impl From<CloneableDescription> for Description {
    fn from(value: CloneableDescription) -> Self {
        Description {
            description: value.description.into_any(),
        }
    }
}

impl CloneableDescription {
    pub fn new<S: StreamId>(description: S::OpenDescription) -> Self
    where
        S::OpenDescription: Clone + 'static,
    {
        CloneableDescription {
            description: Box::new(description),
        }
    }
}
