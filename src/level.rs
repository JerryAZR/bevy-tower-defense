use bevy::prelude::*;
use serde::Deserialize;
use std::collections::HashMap;

use crate::map::{MapLayout, TileType};

#[derive(Debug, Deserialize, Resource)]
pub struct LevelData {
    pub map: MapData,
    pub paths: HashMap<String, PathData>,
    #[serde(default)]
    #[allow(dead_code)]
    pub waves: Vec<WaveData>,
}

#[derive(Debug, Deserialize)]
pub struct MapData {
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Deserialize, Clone)]
pub struct PathData {
    pub waypoints: Vec<[u32; 2]>,
}

#[derive(Debug, Deserialize)]
pub struct WaveData {
    // Reserved for Part 7
}

/// Load a level definition from a TOML file.
pub fn load_level(path: &str) -> LevelData {
    let content = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("Failed to read {}: {}", path, e));
    toml::from_str(&content)
        .unwrap_or_else(|e| panic!("Failed to parse {}: {}", path, e))
}

/// Build a MapLayout from level data: grass everywhere, then trace each path
/// and expand its centerline to a width-3 strip.
pub fn build_map_from_level(level: &LevelData) -> MapLayout {
    let width = level.map.width;
    let height = level.map.height;
    let mut tiles = vec![TileType::Grass; (width * height) as usize];

    for (id, path) in &level.paths {
        if path.waypoints.len() < 2 {
            panic!("Path '{}' must have at least 2 waypoints", id);
        }
        for window in path.waypoints.windows(2) {
            let [x1, y1] = window[0];
            let [x2, y2] = window[1];

            if x1 == x2 {
                let y_start = y1.min(y2);
                let y_end = y1.max(y2);
                for y in y_start..=y_end {
                    for dx in -1i32..=1i32 {
                        let nx = x1 as i32 + dx;
                        if nx >= 0 && nx < width as i32 {
                            let idx = (y * width + nx as u32) as usize;
                            tiles[idx] = TileType::Path;
                        }
                    }
                }
            } else if y1 == y2 {
                let x_start = x1.min(x2);
                let x_end = x1.max(x2);
                for x in x_start..=x_end {
                    for dy in -1i32..=1i32 {
                        let ny = y1 as i32 + dy;
                        if ny >= 0 && ny < height as i32 {
                            let idx = (ny as u32 * width + x) as usize;
                            tiles[idx] = TileType::Path;
                        }
                    }
                }
            } else {
                panic!(
                    "Path '{}' has a diagonal segment: ({},{}) -> ({},{}). \
                     Only axis-aligned segments are supported.",
                    id, x1, y1, x2, y2
                );
            }
        }
    }

    // Fill diagonal tiles at path corners so turns form complete 3×3 blocks.
    for (_id, path) in &level.paths {
        let wps = &path.waypoints;
        for i in 1..wps.len() - 1 {
            let [x1, y1] = wps[i - 1];
            let [x2, y2] = wps[i];
            let [x3, y3] = wps[i + 1];

            let dx1 = x2 as i32 - x1 as i32;
            let dy1 = y2 as i32 - y1 as i32;
            let dx2 = x3 as i32 - x2 as i32;
            let dy2 = y3 as i32 - y2 as i32;

            // The segment that is horizontal determines the missing x offset,
            // and the vertical segment determines the missing y offset.
            let mx = if dy1 == 0 { dx1.signum() } else { -dx2.signum() };
            let my = if dx1 == 0 { dy1.signum() } else { -dy2.signum() };

            let cx = x2 as i32 + mx;
            let cy = y2 as i32 + my;
            if cx >= 0 && cx < width as i32 && cy >= 0 && cy < height as i32 {
                let idx = (cy as u32 * width + cx as u32) as usize;
                tiles[idx] = TileType::Path;
            }
        }
    }

    MapLayout {
        width,
        height,
        tiles,
    }
}
