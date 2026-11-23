//! Shared primitives used by every module. Keep this dependency-free.

/// Integer grid position. `y` grows downward.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Hash)]
pub struct Vec2 {
    pub x: i32,
    pub y: i32,
}

impl Vec2 {
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    pub fn within(self, f: Field) -> bool {
        self.x >= 0 && self.y >= 0 && self.x < f.w && self.y < f.h
    }

    pub fn manhattan(self, o: Vec2) -> i32 {
        (self.x - o.x).abs() + (self.y - o.y).abs()
    }

    /// Chebyshev (king-move) distance - the natural metric for grid movement.
    pub fn cheb(self, o: Vec2) -> i32 {
        (self.x - o.x).abs().max((self.y - o.y).abs())
    }

    pub fn offset(self, d: Dir) -> Vec2 {
        let v = d.delta();
        Vec2::new(self.x + v.x, self.y + v.y)
    }

    pub fn clamp_to(self, f: Field) -> Vec2 {
        Vec2::new(self.x.clamp(0, f.w - 1), self.y.clamp(0, f.h - 1))
    }
}

impl std::ops::Add for Vec2 {
    type Output = Vec2;
    fn add(self, o: Vec2) -> Vec2 {
        Vec2::new(self.x + o.x, self.y + o.y)
    }
}

impl std::ops::Sub for Vec2 {
    type Output = Vec2;
    fn sub(self, o: Vec2) -> Vec2 {
        Vec2::new(self.x - o.x, self.y - o.y)
    }
}

/// Continuous position / velocity in grid cells.
///
/// Bullets, enemies and players all move on a sub-cell plane and are snapped to
/// whole cells only when drawn. That is what makes dodging feel smooth instead
/// of steppy at a 60 ms tick.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct Vec2f {
    pub x: f32,
    pub y: f32,
}

impl Vec2f {
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    pub const fn splat(v: f32) -> Self {
        Self { x: v, y: v }
    }

    pub fn from_cell(p: Vec2) -> Self {
        Self::new(p.x as f32, p.y as f32)
    }

    pub fn len(self) -> f32 {
        (self.x * self.x + self.y * self.y).sqrt()
    }

    pub fn dist(self, o: Vec2f) -> f32 {
        (self - o).len()
    }

    /// Unit vector, or a safe fallback if this is (near) zero length.
    pub fn norm(self) -> Vec2f {
        let l = self.len();
        if l < 1e-4 {
            Vec2f::new(1.0, 0.0)
        } else {
            Vec2f::new(self.x / l, self.y / l)
        }
    }

    /// Left-hand perpendicular (rotate 90 degrees).
    pub fn perp(self) -> Vec2f {
        Vec2f::new(-self.y, self.x)
    }

    pub fn dot(self, o: Vec2f) -> f32 {
        self.x * o.x + self.y * o.y
    }

    pub fn scale(self, k: f32) -> Vec2f {
        Vec2f::new(self.x * k, self.y * k)
    }

    pub fn clamp_to(self, f: Field) -> Vec2f {
        Vec2f::new(
            self.x.clamp(0.0, (f.w - 1) as f32),
            self.y.clamp(0.0, (f.h - 1) as f32),
        )
    }

    /// Snap to the nearest whole grid cell.
    pub fn cell(self) -> Vec2 {
        Vec2::new(self.x.round() as i32, self.y.round() as i32)
    }

    /// Snap to the nearest cell, clamped into the field (for drawing/sampling).
    pub fn cell_in(self, f: Field) -> Vec2 {
        Vec2::new(
            (self.x.round() as i32).clamp(0, f.w - 1),
            (self.y.round() as i32).clamp(0, f.h - 1),
        )
    }
}

impl std::ops::Add for Vec2f {
    type Output = Vec2f;
    fn add(self, o: Vec2f) -> Vec2f {
        Vec2f::new(self.x + o.x, self.y + o.y)
    }
}

impl std::ops::Sub for Vec2f {
    type Output = Vec2f;
    fn sub(self, o: Vec2f) -> Vec2f {
        Vec2f::new(self.x - o.x, self.y - o.y)
    }
}

impl std::ops::Mul<f32> for Vec2f {
    type Output = Vec2f;
    fn mul(self, k: f32) -> Vec2f {
        self.scale(k)
    }
}

/// Cardinal direction of a single keypress.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Dir {
    Up,
    Down,
    Left,
    Right,
}

impl Dir {
    pub fn delta(self) -> Vec2 {
        match self {
            Dir::Up => Vec2::new(0, -1),
            Dir::Down => Vec2::new(0, 1),
            Dir::Left => Vec2::new(-1, 0),
            Dir::Right => Vec2::new(1, 0),
        }
    }

    /// Glyph used for particles / trails.
    pub fn glyph(self) -> char {
        match self {
            Dir::Up => '\u{25b2}',
            Dir::Down => '\u{25bc}',
            Dir::Left => '\u{25c0}',
            Dir::Right => '\u{25b6}',
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Side {
    One,
    Two,
}

impl Side {
    pub fn idx(self) -> usize {
        match self {
            Side::One => 0,
            Side::Two => 1,
        }
    }
}

/// Playable area in grid cells.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Field {
    pub w: i32,
    pub h: i32,
}

impl Field {
    pub fn new(w: i32, h: i32) -> Self {
        Self {
            w: w.max(16),
            h: h.max(8),
        }
    }

    pub fn center(&self) -> Vec2 {
        Vec2::new(self.w / 2, self.h / 2)
    }
}

/// Bresenham line between two cells, endpoints included, no duplicates.
pub fn line(a: Vec2, b: Vec2) -> Vec<Vec2> {
    let mut out = Vec::new();
    let dx = (b.x - a.x).abs();
    let dy = -(b.y - a.y).abs();
    let sx = if a.x < b.x { 1 } else { -1 };
    let sy = if a.y < b.y { 1 } else { -1 };
    let mut err = dx + dy;
    let (mut x, mut y) = (a.x, a.y);
    loop {
        out.push(Vec2::new(x, y));
        if x == b.x && y == b.y {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            err += dx;
            y += sy;
        }
        if out.len() > 4096 {
            break;
        }
    }
    out
}
