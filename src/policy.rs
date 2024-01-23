//! Contains all types and systems associated with policies.
//! Policies are used to control when entity updates should be dispatched, and who shall receive them.
//! These are useful to dynamically hide game state to eliminate some forms of cheating,
//! or to reduce bandwidth usage.
//! 
//! The two primary interfaces are a [ComponentPolicy] or an [EntityRingPolicy].
//! 
//! The former is used to declare which clients shall receive a particular component,
//! while the latter is used to establish rings of influence around a particular entity,
//! and the maximum update rate for each ring.
use std::{marker::PhantomData, time::Duration};
use bevy::ecs::component::{Component, ComponentId};
use num_traits::Num;
use smallvec::SmallVec;
use crate::ClientId;



/// The clients selected by this particular policy.
/// 
/// Use this to dynamically change which client's receive a given components state.
/// If a client is removed from all component policies, the client will implicilty receive a cleanup message to remove it locally.
#[derive(PartialEq, Eq, Hash)]
pub enum PolicyType<I: ClientId> {
    All,
    Exclude(SmallVec<[I; 32]>),
    Include(Vec<I>),
    Owner,
    None
}

impl<I: ClientId> PolicyType<I> {
    /// Checks whether a given client is included in this policy.
    /// Has a O(n) complexity, so use sparingly.
    pub fn is_included(&self, id: &I) -> bool {
        match self {
            PolicyType::All => true,
            PolicyType::Exclude(exclude) => !exclude.contains(id),
            PolicyType::Include(include) => include.contains(id),
            PolicyType::Owner => false,
            PolicyType::None => false
        }
    }

    /// Checks whether a given client is excluded from this policy.
    /// Has a O(n) complexity, so use sparingly.
    pub fn is_excluded(&self, id: &I) -> bool {
        match self {
            PolicyType::All => false,
            PolicyType::Exclude(exclude) => exclude.contains(id),
            PolicyType::Include(include) => !include.contains(id),
            PolicyType::Owner => true,
            PolicyType::None => true
        }
    }

    /// Change the policy so it includes all clients.
    pub fn include_all(&mut self) {
        *self = PolicyType::All;
    }

    /// Change the policy so it excludes all clients.
    pub fn exclude_all(&mut self) {
        *self = PolicyType::None;
    }

    /// Add the specified client to the list of included clients.
    /// 
    /// This may change the policy type if it is 'Owner' or 'None'
    pub fn include(&mut self, id: I) {
        match self {
            PolicyType::All => {},
            PolicyType::Exclude(exclude) => {
                if let Some(index) = exclude.iter().position(|x| *x == id) {
                    exclude.remove(index);
                }
            },
            PolicyType::Include(include) => {
                if !include.contains(&id) {
                    include.push(id);
                }
            },
            PolicyType::Owner => {
                *self = PolicyType::Include(vec![id]);
            },
            PolicyType::None => {
                *self = PolicyType::Include(vec![id]);
            }
        }
    }

    /// Add the specified client to the list of excluded clients.
    /// 
    /// This will change the policy type from 'Owner' or 'None' to 'Exclude'.
    pub fn exclude(&mut self, id: I) {
        match self {
            PolicyType::All => {},
            PolicyType::Exclude(exclude) => {
                if !exclude.contains(&id) {
                    exclude.push(id);
                }
            },
            PolicyType::Include(include) => {
                if let Some(index) = include.iter().position(|x| *x == id) {
                    include.remove(index);
                }
            },
            PolicyType::Owner => {
                *self = PolicyType::Exclude(SmallVec::from_elem(id, 1));
            },
            PolicyType::None => {}
        }
    }

    /// Toggles the inclusion of the specified client.
    pub fn toggle(&mut self, id: I) {
        match self {
            PolicyType::All => {},
            PolicyType::Exclude(exclude) => {
                if let Some(index) = exclude.iter().position(|x| *x == id) {
                    exclude.remove(index);
                } else {
                    exclude.push(id);
                }
            },
            PolicyType::Include(include) => {
                if let Some(index) = include.iter().position(|x| *x == id) {
                    include.remove(index);
                } else {
                    include.push(id);
                }
            },
            PolicyType::Owner => {},
            PolicyType::None => {}
        }
    }
}

/// A overarching policy guiding how and when the component 'C' shall be streamed on a particular entity.
/// 
/// If all networked components contain an identical policy, the entire entity will be synchronized
/// according to that policy.
/// 
/// This is the default stream policy. For limiting refresh rate or other customized stream behavior,
/// Write your own policy and ensure that it impls [StreamDecision].
#[derive(Component, PartialEq, Eq)]
pub struct ComponentPolicy<I: ClientId, C: Component> {
    pub component: PhantomData<C>,
    pub policy: PolicyType<I>,
}

impl<I: ClientId, C: Component> ComponentPolicy<I, C> {
    pub fn new(pol: PolicyType<I>) -> Self {
        Self {
            component: PhantomData,
            policy: pol
        }
    }

    /// Gets a mutable reference to the underlying policy.
    pub fn policy_mut(&mut self) -> &mut PolicyType<I> {
        &mut self.policy
    }

    /// Gets a reference to the underlying policy.
    pub fn policy(&self) -> &PolicyType<I> {
        &self.policy
    }
}


/// A policy that can optionally be associated with a client and a particular 'ring'.
/// Any other entities that lie within 'N' distance to this one will receive
/// the state of this entity, but will be limited by refresh rate.
/// 
/// If at least one ring is specified on an entity, all component policies will be adjusted to exclude
/// clients once they fall off from the final ring.
/// 
/// It is logically invalid to have multiple rings with the same 'N' value. 
/// 
/// It is strongly advised to avoid having multiple rings with an identical 'max_refresh_rate' value,
/// as this negatively affects performance.
/// 
/// /// # Type Parameters
/// `N`: The numeric type used to represent distance.
#[derive(Component)]
pub struct EntityRingPolicy<N: Num, P> {
    pub ring: N,
    pub max_refresh_rate: Duration,

    /// The associated marker for this policy.
    /// For each (ring, Duration) pair, there should exist a unique marker type.
    /// This allows you to have multiple rings on a single entity.
    pol: PhantomData<P>
}

/// A cache containing all of the unique policies in a given entity.
/// 
/// This is used by the update dispatcher to efficiently determine policy overlap without checking each component.
/// For each unique policy, updates are constructed in a manner that respects the entity-wide [EntityRingPolicy].
/// Components with no policy associated will all be included in a set with 'None' as the policy type.
#[derive(Component)]
pub(crate) struct EntityPolicyCache<I: ClientId>(SmallVec<[(Option<PolicyType<I>>, SmallVec<[ComponentId; 6]>); 3]>);

