//! Contains all of the necessary server logic to facilitate networking.
//! The primary interface exported by this library is the [ServerPlugin].
//! When attached to a bevy instance, this plugin will process received messages and forward relevant state changes to the muxer(s).
use bevy::{ecs::storage::Table, prelude::*};
use crate::Net;

pub struct ServerPlugin;

impl Plugin for ServerPlugin {
    fn build(&self, app: &mut bevy::prelude::App) {
        
    }
}


/// A copy of an Archetypal [Table] used for delta encoding.
///
/// Whenever a networked archetype has a change, it is XORed against the prior state here to get a delta bitmap.
/// This is compressed and sent to the connected peers (muxers).
/// This is only used when a 'Muxer' has been enabled and connected to the server.
/// It only contains duplicates for the [Table::entities] field, as well as any [Net<T>] component columns.
struct NetworkArchetype(Table);


/// Registers the specified T as a networked component.
///
/// This function adds the required systems to the app that can propogate archetypal changes as needed.
pub fn register_net_component<T: Component>(app: &mut bevy::prelude::App) {
    // Detects when an entity moves into a specific NetArchetype.
    let move_into_detector = | q: Query<Entity, Added<Net<T>>>| {
        //When an entity moves into an archetype that is networked, two things must happen:
        //
    };

    // Responsible for detecting when an entity moves out of a specific NetArchetype.
    // This will not detect if an entity is moving between identical archetypes that only differ in non-networked columns.
    // For example, if an entity moves from (Net<A>, Net<B>, C) to (Net<A>, Net<B>), the first archetype wouldn't be informed of its departure.
    let move_out_detector = | mut removals: RemovedComponents<Net<T>> | {

    };
    

    app.add_systems(Update, (move_into_detector, move_out_detector));

}






/// A system responsible for tracking all 'networked' archetypes, and streaming changes to the muxer accordingly.
/// 
/// 
/// A 'networked' archetype is one that consists of at least one Net<T> component.
/// This system only detects cell level changes to an archetype (refer to <https://taintedcoders.com/bevy/archetypes/> for more info).
/// The alternative system, [track_archetype_moves] announces changes to an entities composition to relevant peers.
fn track_archetype_changes(
    world: &World    
) {
    //For each archetype that contains at least one Net<T> marker,
    for net_archetype in world.archetypes().iter() {

    }

}

/// A system responsible for tracking any changes to an archetype's members. 
/// 
/// This system will broadcast add/remove events on an archetype to the connected peers (muxers).
fn track_archetype_moves() {

}