use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::sleep;
use std::time::Duration;

use rand::Rng;

use super::conway::{Grid, StepResult};
use super::{DisplayState, is_pattern, params_for, parse_hex_color};
use crate::matrix::Color;

pub const NAME: &str = "game of life - random";
const FRAME_DELAY: Duration = Duration::from_millis(200);
const INITIAL_DENSITY: f64 = 0.25;
const STAGNATION_LIMIT: usize = 10;

const DEFAULT_COLOR: Color = Color::new(80, 200, 220);
const DEFAULT_COLOR_HEX: &str = "#50c8dc";

pub fn info() -> serde_json::Value {
    serde_json::json!({
        "name": NAME,
        "inputs": [
            { "key": "color", "label": "Cell color", "type": "color", "default": DEFAULT_COLOR_HEX },
        ],
    })
}

fn randomize(grid: &mut Grid) {
    let mut rng = rand::thread_rng();
    for cell in grid.cells_mut() {
        *cell = rng.gen_bool(INITIAL_DENSITY);
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
    randomize(&mut grid);
    let mut stuck = 0usize;

    while !shutdown.load(Ordering::SeqCst) {
        if !is_pattern(state, NAME) {
            return;
        }
        grid.draw(matrix, current_color(state));
        let _ = matrix.flush();
        match grid.step() {
            StepResult::Changed => stuck = 0,
            _ => {
                stuck += 1;
                if stuck > STAGNATION_LIMIT {
                    randomize(&mut grid);
                    stuck = 0;
                }
            }
        }
        sleep(FRAME_DELAY);
    }
}
