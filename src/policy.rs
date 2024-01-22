use std::hash::Hash;
use smallvec::SmallVec;


/// A marker trait indicating that a given type can be used as a client identifier.
/// This will be auto-implemented on all types that implement [Hash], [Send] and [Sync].
/// [Send] and [Sync] are required because the underlying network buffer is potentially shared between threads.
pub trait ClientId: Hash + Eq + Send + Sync { }

//Implement ClientId for all hashable types.
impl<T: Hash + Eq + Send + Sync> ClientId for T { }


/// A policy dictating which clients shall receive a particular piece of state.
/// 
/// Particularly useful in games where some information must be hidden from the players.
/// For example, in a PvP game, you might want to hide the health of the enemy team to a player.
/// It is possible to set a single policy for an entire entity with [EntityRef::set_net_policy].
/// 
/// # Type Parameter
/// `I`: The type of the client identifier.
#[derive(Hash)]
pub enum ClientPolicy<I:  ClientId> {
    /// Synchronizes this state with all clients.
    All,
    /// Synchronizes this state with all EXCEPT the given clients.
    Exclude(SmallVec<[I; 32]>),
    /// Synchronizes this state with only the given clients.
    // Here, we avoid inlining because in most cases the inclusionary set will be quite large.
    Include(Vec<I>),
    /// Synchronizes this state with only the given client.
    One(I),
    /// Synchronize this unit of state exclusively with the owner.
    /// This will not synchronize if the entity does not have a marked client as the owner.
    Owner,
    /// Disable synchronization of this component (temporarily).
    None
}


macro_rules! consuming_method {
    ($method_name:ident ($($arg:ident: $type:ty),*) -> $return_type:ty) => {
        fn $method_name(self $(, $arg: $type)*) -> $return_type
        where
            Self: Sized,
        {
            match self {
                Self::All(iter) => iter.$method_name($($arg),*),
                Self::Exclude(iter) => iter.$method_name($($arg),*),
                Self::Include(iter) => iter.$method_name($($arg),*),
                Self::One(iter) => iter.$method_name($($arg),*),
                Self::Owner(iter) => iter.$method_name($($arg),*),
                Self::None => ::core::iter::empty::<<Self as ::core::iter::Iterator>::Item>().$method_name($($arg),*),
            }
        }
    };
}

macro_rules! consuming_ord_method {
    ($method_name:ident ($($arg:ident: $type:ty),*) -> $return_type:ty) => {
        fn $method_name(self $(, $arg: $type)*) -> $return_type
        where
            Self: Sized,
            T: Ord,
        {
            match self {
                Self::All(iter) => iter.$method_name($($arg),*),
                Self::Exclude(iter) => iter.$method_name($($arg),*),
                Self::Include(iter) => iter.$method_name($($arg),*),
                Self::One(iter) => iter.$method_name($($arg),*),
                Self::Owner(iter) => iter.$method_name($($arg),*),
                Self::None => ::core::iter::empty::<<Self as ::core::iter::Iterator>::Item>().$method_name($($arg),*),
            }
        }
    };
}

macro_rules! immutable_method {
    ($method_name:ident ($($arg:ident: $type:ty),*) -> $return_type:ty) => {
        fn $method_name(&self $(, $arg: $type)*) -> $return_type {
            match self {
                Self::All(iter) => iter.$method_name($($arg),*),
                Self::Exclude(iter) => iter.$method_name($($arg),*),
                Self::Include(iter) => iter.$method_name($($arg),*),
                Self::One(iter) => iter.$method_name($($arg),*),
                Self::Owner(iter) => iter.$method_name($($arg),*),
                Self::None => ::core::iter::empty::<<Self as ::core::iter::Iterator>::Item>().$method_name($($arg),*),
            }
        }
    };
}

macro_rules! mutable_method {
    ($method_name:ident ($($arg:ident: $type:ty),*) -> $return_type:ty) => {
        fn $method_name(&mut self $(, $arg: $type)*) -> $return_type {
            match self {
                Self::All(iter) => iter.$method_name($($arg),*),
                Self::Exclude(iter) => iter.$method_name($($arg),*),
                Self::Include(iter) => iter.$method_name($($arg),*),
                Self::One(iter) => iter.$method_name($($arg),*),
                Self::Owner(iter) => iter.$method_name($($arg),*),
                Self::None => ::core::iter::empty::<<Self as ::core::iter::Iterator>::Item>().$method_name($($arg),*),
            }
        }
    };
}

#[derive(Clone, Debug)]
pub enum PolicyIter<T, I1, I2, I3, I4, I5>
where
    I1: Iterator<Item = T>,
    I2: Iterator<Item = T>,
    I3: Iterator<Item = T>,
    I4: Iterator<Item = T>,
    I5: Iterator<Item = T>,
{
    All(I1),
    Exclude(I2),
    Include(I3),
    One(I4),
    Owner(I5),
    None,
}

impl<T, I1, I2, I3, I4, I5> Iterator for PolicyIter<T, I1, I2, I3, I4, I5>
where
    I1: Iterator<Item = T>,
    I2: Iterator<Item = T>,
    I3: Iterator<Item = T>,
    I4: Iterator<Item = T>,
    I5: Iterator<Item = T>,
{
    type Item = T;

    mutable_method!(next() -> Option<Self::Item>);
    consuming_method!(count() -> usize);
    consuming_method!(last() -> Option<Self::Item>);
    consuming_ord_method!(max() -> Option<Self::Item>);
    consuming_ord_method!(min() -> Option<Self::Item>);
    mutable_method!(nth(n: usize) -> Option<Self::Item>);
    immutable_method!(size_hint() -> (usize, Option<usize>));
}

impl<T, I1, I2, I3, I4, I5> core::iter::FusedIterator for PolicyIter<T, I1, I2, I3, I4, I5>
where
    I1: core::iter::FusedIterator + Iterator<Item = T>,
    I2: core::iter::FusedIterator + Iterator<Item = T>,
    I3: core::iter::FusedIterator + Iterator<Item = T>,
    I4: core::iter::FusedIterator + Iterator<Item = T>,
    I5: core::iter::FusedIterator + Iterator<Item = T>,
{
}

impl<T, I1, I2, I3, I4, I5> DoubleEndedIterator for PolicyIter<T, I1, I2, I3, I4, I5>
where
    I1: DoubleEndedIterator + Iterator<Item = T>,
    I2: DoubleEndedIterator + Iterator<Item = T>,
    I3: DoubleEndedIterator + Iterator<Item = T>,
    I4: DoubleEndedIterator + Iterator<Item = T>,
    I5: DoubleEndedIterator + Iterator<Item = T>,
{
    mutable_method!(next_back() -> Option<Self::Item>);
    mutable_method!(nth_back(n: usize) -> Option<Self::Item>);
}

impl<T, I1, I2, I3, I4, I5> ExactSizeIterator for PolicyIter<T, I1, I2, I3, I4, I5>
where
    I1: ExactSizeIterator + Iterator<Item = T>,
    I2: ExactSizeIterator + Iterator<Item = T>,
    I3: ExactSizeIterator + Iterator<Item = T>,
    I4: ExactSizeIterator + Iterator<Item = T>,
    I5: ExactSizeIterator + Iterator<Item = T>,
{
    immutable_method!(len() -> usize);
}
