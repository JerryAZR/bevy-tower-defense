use bevy::prelude::*;

/// Logical tile types. Gameplay systems query these, not visual atlas indices.
#[derive(Component, Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum TileType {
    Grass,
    Path,
}

#[derive(Component)]
pub struct MapTile;

#[derive(Component)]
pub struct PathTile;

/// The authoritative grid of logical tile types.
#[derive(Resource)]
pub struct MapLayout {
    pub width: u32,
    pub height: u32,
    pub tiles: Vec<TileType>,
}

impl MapLayout {
    pub fn get(&self, x: u32, y: u32) -> Option<TileType> {
        if x < self.width && y < self.height {
            Some(self.tiles[(y * self.width + x) as usize])
        } else {
            None
        }
    }
}
