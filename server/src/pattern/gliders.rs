use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::sleep;
use std::time::Duration;

use super::conway::Grid;
use super::{DisplayState, is_pattern, params_for, parse_hex_color};
use crate::matrix::Color;

pub const NAME: &str = "game of life - gliders";
const FRAME_DELAY: Duration = Duration::from_millis(200);

const DEFAULT_COLOR: Color = Color::new(80, 200, 220);
const DEFAULT_COLOR_HEX: &str = "#50c8dc";

// Place SE-moving gliders on parallel diagonals. Each glider's cells span four
// adjacent d=y-x values; with D_SPACING of 8 (4 diagonals on a 32-wide torus)
// and ALONG_SPACING of 4 within a diagonal (8 per diagonal), nearest cells of
// distinct gliders sit Chebyshev-distance 3 apart, so no glider is ever a
// neighbor of another. They all move SE at the same speed, so their relative
// positions never change. Result: 32 independent gliders, deterministic,
// runs forever.
const D_SPACING: usize = 8;
const ALONG_SPACING: usize = 4;

pub fn info() -> serde_json::Value {
    serde_json::json!({
        "name": NAME,
        "inputs": [
            { "key": "color", "label": "Cell color", "type": "color", "default": DEFAULT_COLOR_HEX },
        ],
    })
}

fn seed(grid: &mut Grid) {
    let w = grid.width;
    let h = grid.height;
    // SE-moving glider stamp:
    // .#.
    // ..#
    // ###
    let stamp = [(1usize, 0usize), (2, 1), (0, 2), (1, 2), (2, 2)];
    let n_diagonals = w / D_SPACING;
    let n_along = w / ALONG_SPACING;

    for di in 0..n_diagonals {
        let d_offset = di * D_SPACING;
        for ai in 0..n_along {
            let along = ai * ALONG_SPACING;
            let px = along;
            let py = (along + d_offset) % h;
            for (dx, dy) in stamp {
                let xi = (px + dx) % w;
                let yi = (py + dy) % h;
                let i = grid.idx(xi, yi);
                grid.cells_mut()[i] = true;
            }
        }
    }
}

fn current_color(state: &DisplayState) -> Color {
    params_for(state, NAME)
        .and_then(|p| p.get("color").and_then(|v| v.as_str()).map(str::to_string))
        .and_then(|hex| parse_hex_color(&hex))
        .unwrap_or(DEFAULT_COLOR)
}

pub fn run<M: crate::matrix::Matrix>(
    matrix: &mut M,
    state: &DisplayState,
    shutdown: &Arc<AtomicBool>,
) {
    let mut grid = Grid::new(matrix.width(), matrix.height());
    seed(&mut grid);

    while !shutdown.load(Ordering::SeqCst) {
        if !is_pattern(state, NAME) {
            return;
        }
        grid.draw(matrix, current_color(state));
        let _ = matrix.flush();
        grid.step();
        sleep(FRAME_DELAY);
    }
}
