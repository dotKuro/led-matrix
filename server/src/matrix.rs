#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Color {
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }
}

pub trait Matrix {
    type Error;

    fn width(&self) -> usize;
    fn height(&self) -> usize;
    fn set(&mut self, x: usize, y: usize, color: Color);
    fn clear(&mut self);
    fn flush(&mut self) -> Result<(), Self::Error>;
}
