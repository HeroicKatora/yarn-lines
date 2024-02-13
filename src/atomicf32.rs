use core::sync::atomic::{AtomicU32, Ordering};

pub struct AtomicF32(AtomicU32);

impl AtomicF32 {
    pub const fn new() -> Self {
        // 0 is the right bit representation for 0.0
        let v = AtomicU32::new(0);
        AtomicF32(v)
    }

    pub fn fetch_add(&self, add: f32) -> f32 {
        let pre = self.0.fetch_update(
            Ordering::Relaxed,
            Ordering::Relaxed,
            |v| {
                let pre = Self::from_v(v);
                Some(Self::to_v(pre + add))
            })
            .unwrap_or_else(|x| x);

        Self::from_v(pre)
    }

    pub fn load(&self) -> f32 {
        Self::from_v(self.0.load(Ordering::Relaxed))
    }

    fn to_v(val: f32) -> u32 {
        u32::from_be_bytes(val.to_be_bytes())
    }

    fn from_v(val: u32) -> f32 {
        f32::from_be_bytes(val.to_be_bytes())
    }
}
