//! Contains the [MuxerPlugin] and all of its associated functionality.
//! A muxer is an independent unit that acts as a proxy between clients and servers.
//! For smaller titles, it is recommended to embed the muxer directly into the server.
//! 
//! The muxer is responsible for aggregating network messages and state to optimize bandwidth.

use bevy::app::Plugin;


/// A self contained muxer instance.
/// A muxer is responsible for listening to external connections and proxying traffic to the main server.
/// The muxer receives batches of updates from the main game server and dispatches updates according to the defined [RelevancyPolicy].
pub struct MuxerPlugin;

impl Plugin for MuxerPlugin {
    fn build(&self, app: &mut bevy::prelude::App) {
        
    }
}