use crate::matrix::{Color, Matrix};

#[derive(PartialEq, Eq)]
pub enum StepResult {
    Changed,
    Static,
    Oscillator,
}

pub struct Grid {
    pub width: usize,
    pub height: usize,
    cells: Vec<bool>,
    next: Vec<bool>,
    previous: Vec<bool>,
}

impl Grid {
    pub fn new(width: usize, height: usize) -> Self {
        let n = width * height;
        Self {
            width,
            height,
            cells: vec![false; n],
            next: vec![false; n],
            previous: vec![false; n],
        }
    }

    pub fn idx(&self, x: usize, y: usize) -> usize {
        y * self.width + x
    }

    pub fn cells_mut(&mut self) -> &mut [bool] {
        &mut self.cells
    }

    fn count_neighbors(&self, x: usize, y: usize) -> u8 {
        let mut count = 0;
        let w = self.width as isize;
        let h = self.height as isize;
        for dy in [-1isize, 0, 1] {
            for dx in [-1isize, 0, 1] {
                if dx == 0 && dy == 0 {
                    continue;
                }
                let nx = (x as isize + dx).rem_euclid(w) as usize;
                let ny = (y as isize + dy).rem_euclid(h) as usize;
                if self.cells[self.idx(nx, ny)] {
                    count += 1;
                }
            }
        }
        count
    }

    pub fn step(&mut self) -> StepResult {
        for y in 0..self.height {
            for x in 0..self.width {
                let alive = self.cells[self.idx(x, y)];
                let n = self.count_neighbors(x, y);
                let i = self.idx(x, y);
                self.next[i] = if alive { n == 2 || n == 3 } else { n == 3 };
            }
        }
        let result = if self.next == self.cells {
            StepResult::Static
        } else if self.next == self.previous {
            StepResult::Oscillator
        } else {
            StepResult::Changed
        };
        std::mem::swap(&mut self.previous, &mut self.cells);
        std::mem::swap(&mut self.cells, &mut self.next);
        result
    }

    pub fn draw<M: Matrix>(&self, matrix: &mut M, color: Color) {
        matrix.clear();
        for y in 0..self.height {
            for x in 0..self.width {
                if self.cells[self.idx(x, y)] {
                    matrix.set(x, y, color);
                }
            }
        }
    }
}
