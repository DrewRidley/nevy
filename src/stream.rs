use bevy::prelude::*;

/// A plugin containing all of the neccessary systems to synchronize networked state.
/// This plugin will monitor and dispatch appropiate component changes to the connected peers.
pub struct NetStreamPlugin {}
impl Plugin for NetStreamPlugin {
    fn build(&self, app: &mut App) {

    }
}
