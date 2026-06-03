use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::sleep;
use std::time::Duration;

use chrono::{Local, Timelike};

use super::{DisplayState, is_pattern, params_for, parse_hex_color};
use crate::matrix::{Color, Matrix};

pub const NAME: &str = "clock";
const FRAME_DELAY: Duration = Duration::from_millis(200);

const DEFAULT_COLOR: Color = Color::new(220, 220, 220);
const DEFAULT_COLOR_HEX: &str = "#dcdcdc";

const BORDER_INSET: usize = 1;

pub fn info() -> serde_json::Value {
    serde_json::json!({
        "name": NAME,
        "inputs": [
            { "key": "color", "label": "Color", "type": "color", "default": DEFAULT_COLOR_HEX },
        ],
    })
}

// 5x7 pixel font for digits 0..9. Each row is 5 bits, MSB is leftmost column.
const DIGIT_FONT: [[u8; 7]; 10] = [
    // 0
    [0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110],
    // 1
    [0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
    // 2
    [0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b01000, 0b11111],
    // 3
    [0b01110, 0b10001, 0b00001, 0b00110, 0b00001, 0b10001, 0b01110],
    // 4
    [0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010],
    // 5
    [0b11111, 0b10000, 0b10000, 0b11110, 0b00001, 0b10001, 0b01110],
    // 6
    [0b01110, 0b10001, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110],
    // 7
    [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000],
    // 8
    [0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110],
    // 9
    [0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b10001, 0b01110],
];

const DIGIT_W: usize = 5;
const DIGIT_H: usize = 7;

fn draw_digit<M: Matrix>(matrix: &mut M, digit: u8, x: usize, y: usize, color: Color) {
    let glyph = &DIGIT_FONT[digit as usize];
    for row in 0..DIGIT_H {
        for col in 0..DIGIT_W {
            let bit = (glyph[row] >> (DIGIT_W - 1 - col)) & 1;
            if bit == 1 {
                matrix.set(x + col, y + row, color);
            }
        }
    }
}

/// Walk the perimeter clockwise from (inset, inset). Visits the inset rectangle
/// in the order: top row left→right, right col top→bottom (excluding the corner
/// already visited), bottom row right→left, left col bottom→top.
fn border_position(i: usize, w: usize, h: usize, inset: usize) -> (usize, usize) {
    let inner_w = w - 2 * inset;
    let inner_h = h - 2 * inset;
    let top_end = inner_w;
    let right_end = top_end + inner_h - 1;
    let bottom_end = right_end + inner_w - 1;
    if i < top_end {
        (inset + i, inset)
    } else if i < right_end {
        (w - 1 - inset, inset + 1 + (i - top_end))
    } else if i < bottom_end {
        (w - 2 - inset - (i - right_end), h - 1 - inset)
    } else {
        (inset, h - 2 - inset - (i - bottom_end))
    }
}

fn draw_border<M: Matrix>(matrix: &mut M, minute: u32, second: u32, color: Color) {
    let w = matrix.width();
    let h = matrix.height();
    let inner_w = w - 2 * BORDER_INSET;
    let inner_h = h - 2 * BORDER_INSET;
    let perimeter = 2 * inner_w + 2 * inner_h - 4;

    // 2-minute fill/drain cycle. Even minutes add segments in order; the
    // following odd minute removes them in the same FIFO order. The visible
    // range is [trailing, leading) along the perimeter. At cycle boundaries
    // there's no big jump — only the rounding remainder (~2 cells).
    // Ceiling division so the first second's increment is 2 cells (like every
    // other second), not 1. Floor-division left a 1-cell sub-tick on the
    // boundary which looked awkward.
    let t = ((minute as usize) % 2) * 60 + (second as usize);
    let leading = (t.min(60) * perimeter + 59) / 60;
    let trailing = (t.saturating_sub(60) * perimeter + 59) / 60;

    for i in trailing..leading {
        let (x, y) = border_position(i, w, h, BORDER_INSET);
        matrix.set(x, y, color);
    }
}

fn current_color(state: &DisplayState) -> Color {
    params_for(state, NAME)
        .and_then(|p| p.get("color").and_then(|v| v.as_str()).map(str::to_string))
        .and_then(|hex| parse_hex_color(&hex))
        .unwrap_or(DEFAULT_COLOR)
}

pub fn run<M: Matrix>(matrix: &mut M, state: &DisplayState, shutdown: &Arc<AtomicBool>) {
    // Layout: HH : MM
    //   digit (5) + gap (1) + digit (5) + gap (1) + colon (1) + gap (1) + digit (5) + gap (1) + digit (5)
    //   = 25 columns total. Centered in 32 leaves a 3/4 col gutter.
    let w = matrix.width();
    let h = matrix.height();
    // 4 digits (5 each) + inter-digit gap (1) within each pair + 2-px gap between HH and MM = 24
    let total_w = DIGIT_W * 4 + 2 + 2;
    let start_x = (w - total_w) / 2;
    let start_y = (h - DIGIT_H) / 2;

    while !shutdown.load(Ordering::SeqCst) {
        if !is_pattern(state, NAME) {
            return;
        }

        let color = current_color(state);
        let now = Local::now();
        let hours = now.hour() as u8;
        let minutes = now.minute() as u8;
        let seconds = now.second();

        matrix.clear();

        let h_tens = hours / 10;
        let h_ones = hours % 10;
        let m_tens = minutes / 10;
        let m_ones = minutes % 10;

        let mut x = start_x;
        draw_digit(matrix, h_tens, x, start_y, color);
        x += DIGIT_W + 1;
        draw_digit(matrix, h_ones, x, start_y, color);
        x += DIGIT_W + 2;
        draw_digit(matrix, m_tens, x, start_y, color);
        x += DIGIT_W + 1;
        draw_digit(matrix, m_ones, x, start_y, color);

        draw_border(matrix, minutes as u32, seconds, color);

        let _ = matrix.flush();
        sleep(FRAME_DELAY);
    }
}
