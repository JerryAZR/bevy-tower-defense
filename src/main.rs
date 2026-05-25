use bevy::prelude::*;
use bevy_ecs_tilemap::prelude::*;
use std::collections::HashMap;

/// Logical tile types. Gameplay systems query these, not visual atlas indices.
#[derive(Component, Clone, Copy, PartialEq, Eq, Debug, Hash)]
enum TileType {
    Grass,
    Path,
}

#[derive(Component)]
struct MapTile;

#[derive(Component)]
struct PathTile;

/// The authoritative grid of logical tile types.
#[derive(Resource)]
struct MapLayout {
    width: u32,
    height: u32,
    tiles: Vec<TileType>,
}

impl MapLayout {
    fn get(&self, x: u32, y: u32) -> Option<TileType> {
        if x < self.width && y < self.height {
            Some(self.tiles[(y * self.width + x) as usize])
        } else {
            None
        }
    }
}

/// Describes a requirement for a single neighbor.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum NeighborMatch {
    Same,
    Different,
    Any,
}

impl Default for NeighborMatch {
    fn default() -> Self {
        NeighborMatch::Any
    }
}

impl NeighborMatch {
    fn matches(self, center_type: TileType, neighbor: Option<TileType>) -> bool {
        match self {
            NeighborMatch::Any => true,
            NeighborMatch::Same => neighbor == Some(center_type),
            NeighborMatch::Different => neighbor != Some(center_type),
        }
    }
}

/// An 8-neighbor pattern used to select a visual tile.
#[derive(Default, Clone)]
struct NeighborPattern {
    north: NeighborMatch,
    south: NeighborMatch,
    east: NeighborMatch,
    west: NeighborMatch,
    north_east: NeighborMatch,
    north_west: NeighborMatch,
    south_east: NeighborMatch,
    south_west: NeighborMatch,
}

impl NeighborPattern {
    fn matches(&self, center_type: TileType, pos: TilePos, map: &MapLayout) -> bool {
        let n = pos.y.checked_add(1).and_then(|y| map.get(pos.x, y));
        let s = pos.y.checked_sub(1).and_then(|y| map.get(pos.x, y));
        let e = pos.x.checked_add(1).and_then(|x| map.get(x, pos.y));
        let w = pos.x.checked_sub(1).and_then(|x| map.get(x, pos.y));
        let ne = pos.y
            .checked_add(1)
            .and_then(|y| pos.x.checked_add(1).and_then(|x| map.get(x, y)));
        let nw = pos.y
            .checked_add(1)
            .and_then(|y| pos.x.checked_sub(1).and_then(|x| map.get(x, y)));
        let se = pos.y
            .checked_sub(1)
            .and_then(|y| pos.x.checked_add(1).and_then(|x| map.get(x, y)));
        let sw = pos.y
            .checked_sub(1)
            .and_then(|y| pos.x.checked_sub(1).and_then(|x| map.get(x, y)));

        self.north.matches(center_type, n)
            && self.south.matches(center_type, s)
            && self.east.matches(center_type, e)
            && self.west.matches(center_type, w)
            && self.north_east.matches(center_type, ne)
            && self.north_west.matches(center_type, nw)
            && self.south_east.matches(center_type, se)
            && self.south_west.matches(center_type, sw)
    }
}

/// One rule: if the neighbor pattern matches, use this atlas index.
#[derive(Clone)]
struct TileRule {
    pattern: NeighborPattern,
    atlas_index: u32,
}

/// All rules for a single tile type. Checked in order; first match wins.
#[derive(Clone)]
struct TileTypeRuleset {
    rules: Vec<TileRule>,
    fallback: u32,
}

impl TileTypeRuleset {
    fn resolve(&self, tile_type: TileType, pos: TilePos, map: &MapLayout) -> u32 {
        for rule in &self.rules {
            if rule.pattern.matches(tile_type, pos, map) {
                return rule.atlas_index;
            }
        }
        self.fallback
    }
}

/// The full rule book. Maps each tile type to its ruleset.
///
/// Assumption: all `atlas_index` values in this ruleset index into the same
/// texture atlas passed to the tilemap's `TilemapBundle`.
#[derive(Resource, Default)]
struct TileRules {
    rulesets: HashMap<TileType, TileTypeRuleset>,
}

impl TileRules {
    fn add(&mut self, tile_type: TileType, ruleset: TileTypeRuleset) {
        self.rulesets.insert(tile_type, ruleset);
    }

    fn resolve(&self, tile_type: TileType, pos: TilePos, map: &MapLayout) -> u32 {
        self.rulesets
            .get(&tile_type)
            .map(|rs| rs.resolve(tile_type, pos, map))
            .unwrap_or_else(|| panic!("No ruleset for {:?}", tile_type))
    }
}

/// Build a demo map: grass everywhere with a 3-tile-high horizontal strip of path tiles.
fn build_demo_map() -> MapLayout {
    let width: u32 = 15;
    let height: u32 = 10;
    let path_y = height / 2;

    let mut tiles = vec![TileType::Grass; (width * height) as usize];

    let has_top = path_y > 0;
    let has_bot = path_y < height - 1;
    let w = width as usize;

    for x in 0..width {
        let idx = (path_y * width + x) as usize;
        tiles[idx] = TileType::Path;
        if has_top { tiles[idx - w] = TileType::Path; }
        if has_bot { tiles[idx + w] = TileType::Path; }
    }

    MapLayout {
        width,
        height,
        tiles,
    }
}

/// Build the auto-tiling rule book.
fn build_rules() -> TileRules {
    let mut rules = TileRules::default();

    let same = NeighborMatch::Same;
    let diff = NeighborMatch::Different;
    let any = NeighborMatch::Any;

    rules.add(
        TileType::Path,
        TileTypeRuleset {
            rules: vec![
                // Corners (most specific)
                rule(pat(diff, same, same, diff, any, diff, any, any), 79),   // upper-left
                rule(pat(diff, same, diff, same, diff, any, any, any), 81),   // upper-right
                rule(pat(same, diff, same, diff, any, any, any, diff), 125),  // bottom-left
                rule(pat(same, diff, diff, same, any, any, diff, any), 127),  // bottom-right
                // Edges
                rule(pat(diff, same, same, same, any, any, any, any), 80),    // top
                rule(pat(same, diff, same, same, any, any, any, any), 126),   // bottom
                rule(pat(same, same, same, diff, any, any, any, any), 102),   // left
                rule(pat(same, same, diff, same, any, any, any, any), 104),   // right
            ],
            fallback: 103,
        },
    );

    rules.add(
        TileType::Grass,
        TileTypeRuleset {
            rules: vec![],
            fallback: 129,
        },
    );

    rules
}

// Helper: build a NeighborPattern from 8 NeighborMatch values.
fn pat(
    n: NeighborMatch,
    s: NeighborMatch,
    e: NeighborMatch,
    w: NeighborMatch,
    ne: NeighborMatch,
    nw: NeighborMatch,
    se: NeighborMatch,
    sw: NeighborMatch,
) -> NeighborPattern {
    NeighborPattern {
        north: n,
        south: s,
        east: e,
        west: w,
        north_east: ne,
        north_west: nw,
        south_east: se,
        south_west: sw,
    }
}

// Helper: build a TileRule.
fn rule(pattern: NeighborPattern, atlas_index: u32) -> TileRule {
    TileRule {
        pattern,
        atlas_index,
    }
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(ImagePlugin::default_nearest()))
        .add_plugins(TilemapPlugin)
        .add_systems(Startup, setup)
        .run();
}

fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    let map = build_demo_map();
    let rules = build_rules();

    commands.spawn(Camera2d);

    let texture_handle: Handle<Image> = asset_server.load("Tilesheet/towerDefense_tilesheet.png");

    let map_size = TilemapSize {
        x: map.width,
        y: map.height,
    };
    let tile_size = TilemapTileSize { x: 64.0, y: 64.0 };
    let grid_size = tile_size.into();
    let map_type = TilemapType::Square;

    let tilemap_entity = commands.spawn_empty().id();
    let mut tile_storage = TileStorage::empty(map_size);

    for x in 0..map.width {
        for y in 0..map.height {
            let pos = TilePos { x, y };
            let tile_type = map.get(x, y).unwrap();
            let visual_index = rules.resolve(tile_type, pos, &map);

            let tile_entity = commands
                .spawn((
                    TileBundle {
                        position: pos,
                        tilemap_id: TilemapId(tilemap_entity),
                        texture_index: TileTextureIndex(visual_index),
                        ..Default::default()
                    },
                    tile_type,
                    MapTile,
                ))
                .id();

            if tile_type == TileType::Path {
                commands.entity(tile_entity).insert(PathTile);
            }

            tile_storage.set(&pos, tile_entity);
        }
    }

    commands.entity(tilemap_entity).insert(TilemapBundle {
        grid_size,
        map_type,
        size: map_size,
        storage: tile_storage,
        texture: TilemapTexture::Single(texture_handle),
        tile_size,
        anchor: TilemapAnchor::Center,
        ..Default::default()
    });

    // Make map and rules available to future systems.
    commands.insert_resource(map);
    commands.insert_resource(rules);
}
