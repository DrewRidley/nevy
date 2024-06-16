use bevy::prelude::*;
use std::marker::PhantomData;

use super::Header;

/// add this plugin to assign a [HeaderId] for `T`
pub struct HeaderPlugin<T>(PhantomData<T>);

impl<T> Default for HeaderPlugin<T> {
    fn default() -> Self {
        HeaderPlugin(PhantomData)
    }
}

impl<T: Send + Sync + 'static> Plugin for HeaderPlugin<T> {
    fn build(&self, app: &mut App) {
        let header = if let Some(mut next_header_id) = app.world.get_resource_mut::<NextHeaderId>()
        {
            let header = next_header_id.0;
            next_header_id.0 += 1;
            header
        } else {
            let header = 0;
            app.world.insert_resource(NextHeaderId(header + 1));
            header
        };

        app.insert_resource(HeaderId::<T> {
            _p: PhantomData,
            header,
        });
    }
}

#[derive(Resource)]
struct NextHeaderId(Header);

/// use this resource to get the header id for `T`
#[derive(Resource)]
pub struct HeaderId<T> {
    _p: PhantomData<T>,
    header: Header,
}

impl<T> HeaderId<T> {
    pub fn get(&self) -> Header {
        self.header
    }
}
