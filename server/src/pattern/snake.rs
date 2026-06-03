use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::sleep;
use std::time::Duration;

use rand::Rng;
use rand::rngs::ThreadRng;

use super::{DisplayState, is_pattern};
use crate::matrix::{Color, Matrix};

pub const NAME: &str = "snake";
const FRAME_DELAY: Duration = Duration::from_millis(150);

pub fn info() -> serde_json::Value {
    serde_json::json!({
        "name": NAME,
        "inputs": [],
    })
}
const INITIAL_LENGTH: usize = 4;

const SNAKE_COLOR: Color = Color::new(0, 120, 0);
const APPLE_COLOR: Color = Color::new(120, 0, 0);

struct SnakeState {
    width: usize,
    height: usize,
    body: VecDeque<(usize, usize)>,
    apple: (usize, usize),
    rng: ThreadRng,
}

impl SnakeState {
    fn new(width: usize, height: usize) -> Self {
        let mut s = Self {
            width,
            height,
            body: VecDeque::new(),
            apple: (0, 0),
            rng: rand::thread_rng(),
        };
        s.reset();
        s
    }

    fn reset(&mut self) {
        self.body.clear();
        let cy = self.height / 2;
        let head_x = self.width / 2;
        for i in 0..INITIAL_LENGTH {
            self.body.push_back((head_x - i, cy));
        }
        self.place_apple();
    }

    fn place_apple(&mut self) {
        loop {
            let x = self.rng.gen_range(0..self.width);
            let y = self.rng.gen_range(0..self.height);
            if !self.body.iter().any(|&p| p == (x, y)) {
                self.apple = (x, y);
                return;
            }
        }
    }

    fn would_collide(&self, dir: (i32, i32)) -> bool {
        let &(hx, hy) = self.body.front().unwrap();
        let new_x = hx as i32 + dir.0;
        let new_y = hy as i32 + dir.1;
        if new_x < 0
            || new_x >= self.width as i32
            || new_y < 0
            || new_y >= self.height as i32
        {
            return true;
        }
        let new_head = (new_x as usize, new_y as usize);
        let will_eat = new_head == self.apple;
        // When not eating, the tail vacates its cell this step, so a
        // collision with the tail isn't a death.
        if will_eat {
            self.body.iter().any(|&p| p == new_head)
        } else {
            self.body
                .iter()
                .take(self.body.len() - 1)
                .any(|&p| p == new_head)
        }
    }

    fn step(&mut self) -> bool {
        let &(hx, hy) = self.body.front().unwrap();
        let (ax, ay) = self.apple;
        let dx = ax as i32 - hx as i32;
        let dy = ay as i32 - hy as i32;

        // Always close the x gap first, then y. If the preferred axis would
        // hit the body, fall back to the other.
        let primary = if dx != 0 {
            (dx.signum(), 0)
        } else {
            (0, dy.signum())
        };
        let secondary = if dx != 0 {
            (0, dy.signum())
        } else {
            (dx.signum(), 0)
        };

        let chosen = [primary, secondary]
            .into_iter()
            .filter(|&c| c != (0, 0))
            .find(|&c| !self.would_collide(c));

        let dir = match chosen {
            Some(d) => d,
            None => return false,
        };

        let new_head = (
            (hx as i32 + dir.0) as usize,
            (hy as i32 + dir.1) as usize,
        );
        let will_eat = new_head == self.apple;

        self.body.push_front(new_head);
        if will_eat {
            self.place_apple();
        } else {
            self.body.pop_back();
        }
        true
    }

    fn draw<M: Matrix>(&self, matrix: &mut M) {
        matrix.clear();
        for &(x, y) in &self.body {
            matrix.set(x, y, SNAKE_COLOR);
        }
        let (ax, ay) = self.apple;
        matrix.set(ax, ay, APPLE_COLOR);
    }
}

pub fn run<M: Matrix>(matrix: &mut M, state: &DisplayState, shutdown: &Arc<AtomicBool>) {
    let mut snake = SnakeState::new(matrix.width(), matrix.height());

    while !shutdown.load(Ordering::SeqCst) {
        if !is_pattern(state, NAME) {
            return;
        }

        if !snake.step() {
            snake.reset();
        }
        snake.draw(matrix);
        let _ = matrix.flush();
        sleep(FRAME_DELAY);
    }
}
