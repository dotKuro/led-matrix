use std::convert::Infallible;

use crate::matrix::{Color, Matrix};

pub struct DualMatrix<A, B> {
    pub a: A,
    pub b: B,
}

impl<A, B> Matrix for DualMatrix<A, B>
where
    A: Matrix,
    B: Matrix,
{
    type Error = Infallible;

    fn width(&self) -> usize {
        self.a.width()
    }

    fn height(&self) -> usize {
        self.a.height()
    }

    fn set(&mut self, x: usize, y: usize, color: Color) {
        self.a.set(x, y, color);
        self.b.set(x, y, color);
    }

    fn clear(&mut self) {
        self.a.clear();
        self.b.clear();
    }

    fn flush(&mut self) -> Result<(), Self::Error> {
        let _ = self.a.flush();
        let _ = self.b.flush();
        Ok(())
    }
}
