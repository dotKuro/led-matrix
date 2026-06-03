use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::sleep;
use std::time::Duration;

use super::{DisplayState, is_image};
use crate::matrix::{Color, Matrix};

const IDLE_DELAY: Duration = Duration::from_millis(100);

pub fn run<M: Matrix>(
    matrix: &mut M,
    pixels: &[Color],
    state: &DisplayState,
    shutdown: &Arc<AtomicBool>,
) {
    let w = matrix.width();
    let h = matrix.height();
    if pixels.len() != w * h {
        return;
    }

    // Draw once. The image sits until the state changes (different image, a
    // pattern starts, or stop).
    for y in 0..h {
        for x in 0..w {
            matrix.set(x, y, pixels[y * w + x]);
        }
    }
    let _ = matrix.flush();

    while !shutdown.load(Ordering::SeqCst) {
        if !is_image(state, pixels) {
            return;
        }
        sleep(IDLE_DELAY);
    }
}
