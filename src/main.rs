use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::sleep;
use std::time::Duration;

use rand::Rng;
use rs_ws281x::{ChannelBuilder, Controller, ControllerBuilder, StripType};

const NUM_PIXELS: i32 = 512;
const BRIGHTNESS: u8 = 13;
const FRAME_DELAY: Duration = Duration::from_millis(50);

fn build_controller() -> rs_ws281x::Result<Controller> {
    ControllerBuilder::new()
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
        .build()
}

fn clear(controller: &mut Controller) -> rs_ws281x::Result<()> {
    for ch in [0, 1] {
        for led in controller.leds_mut(ch) {
            *led = [0, 0, 0, 0];
        }
    }
    controller.render()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut controller = build_controller()?;

    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    ctrlc::set_handler(move || r.store(false, Ordering::SeqCst))?;

    let mut rng = rand::thread_rng();

    while running.load(Ordering::SeqCst) {
        for ch in [0, 1] {
            for led in controller.leds_mut(ch) {
                let red = rng.gen_range(30..=150);
                let green = rng.gen_range(30..=150);
                let blue = rng.gen_range(30..=150);
                *led = [blue, green, red, 0];
            }
        }
        controller.render()?;
        sleep(FRAME_DELAY);
    }

    clear(&mut controller)?;
    Ok(())
}
