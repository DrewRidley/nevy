use std::io::Cursor;

use crate::archetype::ArchetypeDispatcher;
use bevy::prelude::*;
use bincode::DefaultOptions;


#[derive(Resource)]
struct SendBuffer {
    buf: Cursor<Vec<u8>>,
    serializer: bincode::Serializer<Cursor<Vec<u8>>, DefaultOptions>
}

/// Stream all changes in the ECS world for this tick.
fn stream_changes(world: &World, dispatcher: Res<ArchetypeDispatcher<'static>>, mut send_buffer: ResMut<SendBuffer>) {
    for archetype in &dispatcher.archetypes {
        let archetype_ref = world.archetypes().get(archetype.id).expect("Archetype exists in cache but not in world!");

        for ent in archetype_ref.entities().iter().map(|ae| ae.entity()) {
            let we = world.entity(ent);

            for comp_entry in archetype.components.iter() {
                let change = we.get_change_ticks_by_id(comp_entry.id).unwrap();
                if change.last_changed_tick() == world.change_tick() {
                    //Writes the components data directly to the buffer.
                    //Prior to doing this, we need to build the mask for which components have changed.
                    (comp_entry.serializer)(we.get_by_id(comp_entry.id).unwrap(), &mut send_buffer.serializer).unwrap();
                }                
            }
        }
        //Get the entity ref for archetype_entity.

        
    }
}
