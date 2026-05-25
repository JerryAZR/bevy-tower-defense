use bevy::prelude::*;
use bevy_ecs_tilemap::prelude::*;
use std::collections::HashMap;

use crate::map::{MapLayout, TileType};

/// Describes a requirement for a single neighbor.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NeighborMatch {
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
    pub fn matches(self, center_type: TileType, neighbor: Option<TileType>) -> bool {
        match self {
            NeighborMatch::Any => true,
            NeighborMatch::Same => neighbor == Some(center_type),
            NeighborMatch::Different => neighbor != Some(center_type),
        }
    }
}

/// An 8-neighbor pattern used to select a visual tile.
#[derive(Default, Clone)]
pub struct NeighborPattern {
    pub north: NeighborMatch,
    pub south: NeighborMatch,
    pub east: NeighborMatch,
    pub west: NeighborMatch,
    pub north_east: NeighborMatch,
    pub north_west: NeighborMatch,
    pub south_east: NeighborMatch,
    pub south_west: NeighborMatch,
}

impl NeighborPattern {
    pub fn matches(&self, center_type: TileType, pos: TilePos, map: &MapLayout) -> bool {
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
pub struct TileRule {
    pub pattern: NeighborPattern,
    pub atlas_index: u32,
}

/// All rules for a single tile type. Checked in order; first match wins.
#[derive(Clone)]
pub struct TileTypeRuleset {
    pub rules: Vec<TileRule>,
    pub fallback: u32,
}

impl TileTypeRuleset {
    pub fn resolve(&self, tile_type: TileType, pos: TilePos, map: &MapLayout) -> u32 {
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
pub struct TileRules {
    rulesets: HashMap<TileType, TileTypeRuleset>,
}

impl TileRules {
    pub fn add(&mut self, tile_type: TileType, ruleset: TileTypeRuleset) {
        self.rulesets.insert(tile_type, ruleset);
    }

    pub fn resolve(&self, tile_type: TileType, pos: TilePos, map: &MapLayout) -> u32 {
        self.rulesets
            .get(&tile_type)
            .map(|rs| rs.resolve(tile_type, pos, map))
            .unwrap_or_else(|| panic!("No ruleset for {:?}", tile_type))
    }
}

/// Build the auto-tiling rule book.
pub fn build_rules() -> TileRules {
    let mut rules = TileRules::default();

    let same = NeighborMatch::Same;
    let diff = NeighborMatch::Different;
    let any = NeighborMatch::Any;

    rules.add(
        TileType::Path,
        TileTypeRuleset {
            rules: vec![
                // Inner corners
                rule(pat(same, same, same, same, same, same, diff, same), 82),   // upper-left
                rule(pat(same, same, same, same, same, same, same, diff), 83),   // upper-right
                rule(pat(same, same, same, same, diff, same, same, same), 105),  // bottom-left
                rule(pat(same, same, same, same, same, diff, same, same), 106),  // bottom-right
                // Corners
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
