//! Tiny xorshift PRNG so the crate needs no external randomness dependency.

#[derive(Clone)]
pub struct Rng {
    s: u32,
}

impl Rng {
    pub fn new(seed: u32) -> Self {
        Self {
            s: if seed == 0 { 0x9e37_79b9 } else { seed },
        }
    }

    pub fn from_time() -> Self {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let n = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos() as u64)
                .unwrap_or(0x1234_5678);
            Self::new((n ^ (n >> 32)) as u32)
        }
        #[cfg(target_arch = "wasm32")]
        {
            Self::new(0x5eed_1234)
        }
    }

    pub fn next_u32(&mut self) -> u32 {
        let mut x = self.s;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.s = x;
        x
    }

    /// Uniform in `[0, n)`. Returns 0 when `n <= 0`.
    pub fn below(&mut self, n: i32) -> i32 {
        if n <= 0 {
            return 0;
        }
        (self.next_u32() % n as u32) as i32
    }

    /// Uniform in `[a, b)`.
    pub fn range(&mut self, a: i32, b: i32) -> i32 {
        if b <= a {
            return a;
        }
        a + self.below(b - a)
    }

    pub fn f32(&mut self) -> f32 {
        (self.next_u32() >> 8) as f32 / 16_777_216.0
    }

    pub fn chance(&mut self, p: f32) -> bool {
        self.f32() < p
    }

    pub fn pick<T: Copy>(&mut self, items: &[T]) -> Option<T> {
        if items.is_empty() {
            return None;
        }
        Some(items[self.below(items.len() as i32) as usize])
    }

    pub fn shuffle<T>(&mut self, items: &mut [T]) {
        for i in (1..items.len()).rev() {
            let j = self.below(i as i32 + 1) as usize;
            items.swap(i, j);
        }
    }
}
