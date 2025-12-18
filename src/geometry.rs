use chromiumoxide::layout;
use serde::{Deserialize, Serialize};

#[derive(Copy, Clone, Serialize, Deserialize, Debug)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Into<Point> for layout::Point {
    fn into(self) -> Point {
        Point {
            x: self.x,
            y: self.y,
        }
    }
}

impl Into<layout::Point> for Point {
    fn into(self) -> layout::Point {
        layout::Point {
            x: self.x,
            y: self.y,
        }
    }
}
