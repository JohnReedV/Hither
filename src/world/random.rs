//! Small deterministic generator shared by procedural content.
pub(crate) struct Rng(pub(crate) u64);
impl Rng {
    pub(crate) fn unit(&mut self) -> f32 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.0 >> 32) as f32 / u32::MAX as f32
    }
    pub(crate) fn range(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.unit()
    }
}
