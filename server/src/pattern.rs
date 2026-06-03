mod clock;
mod conway;
mod gliders;
mod image;
mod life;
mod random;
mod snake;
mod trains;

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::sleep;
use std::time::Duration;

use crate::matrix::{Color, Matrix};

pub type Params = HashMap<String, serde_json::Value>;

#[derive(Clone, PartialEq)]
pub enum Display {
    Pattern(String, Params),
    Image(Vec<Color>),
}

pub type DisplayState = Arc<Mutex<Option<Display>>>;

pub const AVAILABLE: &[&str] = &[
    random::NAME,
    snake::NAME,
    life::NAME,
    gliders::NAME,
    clock::NAME,
    trains::NAME,
];

pub fn infos() -> Vec<serde_json::Value> {
    vec![
        random::info(),
        snake::info(),
        life::info(),
        gliders::info(),
        clock::info(),
        trains::info(),
    ]
}

const IDLE_DELAY: Duration = Duration::from_millis(100);

pub(crate) fn is_pattern(state: &DisplayState, name: &str) -> bool {
    matches!(state.lock().unwrap().as_ref(), Some(Display::Pattern(n, _)) if n == name)
}

pub(crate) fn is_image(state: &DisplayState, pixels: &[Color]) -> bool {
    matches!(
        state.lock().unwrap().as_ref(),
        Some(Display::Image(p)) if p.as_slice() == pixels
    )
}

/// Snapshot of the params for the named pattern, if it's the active display.
pub(crate) fn params_for(state: &DisplayState, name: &str) -> Option<Params> {
    match state.lock().unwrap().as_ref() {
        Some(Display::Pattern(n, p)) if n == name => Some(p.clone()),
        _ => None,
    }
}

/// Parse "#rrggbb" or "rrggbb" hex strings.
pub(crate) fn parse_hex_color(hex: &str) -> Option<Color> {
    let hex = hex.trim_start_matches('#');
    if hex.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some(Color::new(r, g, b))
}

pub fn run<M: Matrix>(matrix: &mut M, state: &DisplayState, shutdown: &Arc<AtomicBool>) {
    while !shutdown.load(Ordering::SeqCst) {
        let current = state.lock().unwrap().clone();
        match current {
            Some(Display::Pattern(ref n, _)) if n == random::NAME => {
                random::run(matrix, state, shutdown);
                matrix.clear();
                let _ = matrix.flush();
            }
            Some(Display::Pattern(ref n, _)) if n == snake::NAME => {
                snake::run(matrix, state, shutdown);
                matrix.clear();
                let _ = matrix.flush();
            }
            Some(Display::Pattern(ref n, _)) if n == life::NAME => {
                life::run(matrix, state, shutdown);
                matrix.clear();
                let _ = matrix.flush();
            }
            Some(Display::Pattern(ref n, _)) if n == gliders::NAME => {
                gliders::run(matrix, state, shutdown);
                matrix.clear();
                let _ = matrix.flush();
            }
            Some(Display::Pattern(ref n, _)) if n == clock::NAME => {
                clock::run(matrix, state, shutdown);
                matrix.clear();
                let _ = matrix.flush();
            }
            Some(Display::Pattern(ref n, _)) if n == trains::NAME => {
                trains::run(matrix, state, shutdown);
                matrix.clear();
                let _ = matrix.flush();
            }
            Some(Display::Image(pixels)) => {
                image::run(matrix, &pixels, state, shutdown);
                matrix.clear();
                let _ = matrix.flush();
            }
            _ => {
                sleep(IDLE_DELAY);
            }
        }
    }
}
