//! Contains all types and systems required to exchange messages in Nevy.
//! This module has two primary goals; 
//!     To facilitate the exchange of 'global' messages between peers
//! 
//!     To facilitate the exchange of entity-directed events (ie RPCs).
//!     These RPCS should also have some mechanism to facilitate response handling.

/// A plugin used to facilitate the exchange of global messages between peers.
pub struct MessagePlugin {

}