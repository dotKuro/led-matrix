use rs_ws281x::{ChannelBuilder, Controller, ControllerBuilder, StripType, WS2811Error};

use crate::matrix::{Color, Matrix};

const NUM_PIXELS: i32 = 512;
// Driver-level scaler applied to every channel value before it hits the LED.
// Lives here (not in the upload handler or pattern code) so that the simulator
// always sees the raw colors; only the physical strip is dimmed.
const BRIGHTNESS: u8 = 5;

pub const MATRIX_WIDTH: usize = 32;
const STRIP_ROWS: usize = 8;
const STRIP_COUNT: usize = 4;
pub const MATRIX_HEIGHT: usize = STRIP_ROWS * STRIP_COUNT;
const STRIP_PIXELS: usize = MATRIX_WIDTH * STRIP_ROWS;

pub struct Ws281xMatrix {
    controller: Controller,
}

impl Ws281xMatrix {
    pub fn new() -> rs_ws281x::Result<Self> {
        let controller = ControllerBuilder::new()
            .freq(800_000)
            .dma(10)
            .channel(
                0,
                ChannelBuilder::new()
                    .pin(12)
                    .count(NUM_PIXELS)
                    .strip_type(StripType::Ws2812)
                    .brightness(BRIGHTNESS)
                    .build(),
            )
            .channel(
                1,
                ChannelBuilder::new()
                    .pin(13)
                    .count(NUM_PIXELS)
                    .strip_type(StripType::Ws2812)
                    .brightness(BRIGHTNESS)
                    .build(),
            )
            .build()?;
        Ok(Self { controller })
    }

    fn matrix_to_buffer(x: usize, y: usize) -> (usize, usize) {
        let strip = y / STRIP_ROWS;
        let ry = y % STRIP_ROWS;
        let cx = x;

        let led_in_strip = match strip {
            0 | 2 => {
                let c = MATRIX_WIDTH - 1 - cx;
                let row_offset = if c % 2 == 0 { STRIP_ROWS - 1 - ry } else { ry };
                c * STRIP_ROWS + row_offset
            }
            1 | 3 => {
                let row_offset = if cx % 2 == 0 { ry } else { STRIP_ROWS - 1 - ry };
                cx * STRIP_ROWS + row_offset
            }
            _ => panic!("y out of range: {y}"),
        };

        match strip {
            0 => (0, STRIP_PIXELS + led_in_strip),
            1 => (0, led_in_strip),
            2 => (1, STRIP_PIXELS + led_in_strip),
            3 => (1, led_in_strip),
            _ => unreachable!(),
        }
    }
}

impl Matrix for Ws281xMatrix {
    type Error = WS2811Error;

    fn width(&self) -> usize {
        MATRIX_WIDTH
    }

    fn height(&self) -> usize {
        MATRIX_HEIGHT
    }

    fn set(&mut self, x: usize, y: usize, color: Color) {
        let (channel, index) = Self::matrix_to_buffer(x, y);
        self.controller.leds_mut(channel)[index] = [color.b, color.g, color.r, 0];
    }

    fn clear(&mut self) {
        for ch in [0, 1] {
            for led in self.controller.leds_mut(ch) {
                *led = [0, 0, 0, 0];
            }
        }
    }

    fn flush(&mut self) -> Result<(), Self::Error> {
        self.controller.render()
    }
}
