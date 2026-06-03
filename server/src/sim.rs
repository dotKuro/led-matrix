use std::convert::Infallible;
use std::sync::{Arc, Mutex};

use tokio::sync::broadcast;

use crate::matrix::{Color, Matrix};

pub type Frame = Arc<Vec<u8>>;

/// Broadcaster + a "latest frame" cache so that a client connecting after the
/// last flush still receives the current state. Without the cache, image
/// display (which flushes only once) would appear blank to late subscribers.
#[derive(Clone)]
pub struct FrameBroadcast {
    sender: broadcast::Sender<Frame>,
    latest: Arc<Mutex<Option<Frame>>>,
}

impl FrameBroadcast {
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self {
            sender,
            latest: Arc::new(Mutex::new(None)),
        }
    }

    fn send(&self, frame: Frame) {
        // Hold the latest lock across both updates so subscribe() can't observe
        // a gap between "rx subscribed" and "latest set".
        let mut guard = self.latest.lock().unwrap();
        *guard = Some(frame.clone());
        let _ = self.sender.send(frame);
    }

    /// Subscribe for future frames and atomically read the latest frame, if any.
    pub fn subscribe(&self) -> (Option<Frame>, broadcast::Receiver<Frame>) {
        let guard = self.latest.lock().unwrap();
        let rx = self.sender.subscribe();
        let latest = guard.clone();
        (latest, rx)
    }
}

pub struct SimulatorMatrix {
    width: usize,
    height: usize,
    pixels: Vec<Color>,
    broadcast: FrameBroadcast,
}

impl SimulatorMatrix {
    pub fn new(width: usize, height: usize, broadcast: FrameBroadcast) -> Self {
        Self {
            width,
            height,
            pixels: vec![Color::new(0, 0, 0); width * height],
            broadcast,
        }
    }
}

impl Matrix for SimulatorMatrix {
    type Error = Infallible;

    fn width(&self) -> usize {
        self.width
    }

    fn height(&self) -> usize {
        self.height
    }

    fn set(&mut self, x: usize, y: usize, color: Color) {
        self.pixels[y * self.width + x] = color;
    }

    fn clear(&mut self) {
        let black = Color::new(0, 0, 0);
        for p in &mut self.pixels {
            *p = black;
        }
    }

    fn flush(&mut self) -> Result<(), Self::Error> {
        let mut bytes = Vec::with_capacity(self.pixels.len() * 3);
        for c in &self.pixels {
            bytes.push(c.r);
            bytes.push(c.g);
            bytes.push(c.b);
        }
        self.broadcast.send(Arc::new(bytes));
        Ok(())
    }
}
