//! Boolean axes. Locked character movement uses this so gameplay can freeze a world axis
//! without pulling in another math crate.

use crate::Vec3;

/// One boolean per world axis. `true` means that axis is locked.
///
/// The default is all `false`: nothing is locked, so a character can move on X, Y, and Z.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct BVec3 {
    pub x: bool,
    pub y: bool,
    pub z: bool,
}

impl BVec3 {
    pub const FALSE: Self = Self { x: false, y: false, z: false };

    pub const fn new(x: bool, y: bool, z: bool) -> Self {
        Self { x, y, z }
    }

    /// Zeroes each locked component. Unlocked components are copied through.
    pub fn clear(self, value: Vec3) -> Vec3 {
        Vec3::new(
            if self.x { 0.0 } else { value.x },
            if self.y { 0.0 } else { value.y },
            if self.z { 0.0 } else { value.z },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_locks_nothing_and_clear_zeroes_only_the_locked_axes() {
        let open = BVec3::default();
        assert_eq!(open, BVec3::FALSE);
        assert_eq!(open.clear(Vec3::new(1.0, 2.0, 3.0)), Vec3::new(1.0, 2.0, 3.0));
        assert_eq!(BVec3::new(false, true, false).clear(Vec3::new(1.0, 2.0, 3.0)), Vec3::new(1.0, 0.0, 3.0));
    }
}
