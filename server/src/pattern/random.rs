use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::sleep;
use std::time::Duration;

use rand::distributions::{Distribution, Standard};
use rand::{Rng, random};

use super::{DisplayState, is_pattern};
use crate::matrix::{Color, Matrix};

pub const NAME: &str = "random";
const FRAME_DELAY: Duration = Duration::from_millis(500);

pub fn info() -> serde_json::Value {
    serde_json::json!({
        "name": NAME,
        "inputs": [],
    })
}

#[derive(Debug, PartialEq, Eq)]
enum ColorComponent {
    Red,
    Green,
    Blue,
}

impl Distribution<ColorComponent> for Standard {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> ColorComponent {
        match rng.gen_range(0..=2) {
            0 => ColorComponent::Red,
            1 => ColorComponent::Green,
            _ => ColorComponent::Blue,
        }
    }
}

fn frame<M: Matrix>(matrix: &mut M) {
    let (w, h) = (matrix.width(), matrix.height());
    for y in 0..h {
        for x in 0..w {
            let picked: [ColorComponent; 2] = [random(), random()];
            let color = Color::new(
                if picked.contains(&ColorComponent::Red) { 200 } else { 0 },
                if picked.contains(&ColorComponent::Green) { 200 } else { 0 },
                if picked.contains(&ColorComponent::Blue) { 200 } else { 0 },
            );
            matrix.set(x, y, color);
        }
    }
}

pub fn run<M: Matrix>(matrix: &mut M, state: &DisplayState, shutdown: &Arc<AtomicBool>) {
    while !shutdown.load(Ordering::SeqCst) {
        if !is_pattern(state, NAME) {
            return;
        }
        frame(matrix);
        let _ = matrix.flush();
        sleep(FRAME_DELAY);
    }
}
