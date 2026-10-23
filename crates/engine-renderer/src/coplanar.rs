//! Opt-in depth bias for coplanar sprites.
//!
//! Ordinary sprites and meshes keep their real spatial depth. [`CoplanarPriority`] is a separate
//! component. It is not a sort key, not a batch key, and not a replacement for [`crate::RenderOrder`].

use core::fmt;

/// Highest stored priority. Level 0 is already an opt-in step toward the camera.
pub const MAX_COPLANAR_PRIORITY: u8 = 7;

/// World-unit thickness used when a world has no [`CoplanarSettings`].
///
/// Production tests on Apple M5 / Metal with reverse-Z passed this value, and passed `1/1024` and
/// `1/64`, at the same inside-pull and outside-pull checks. Level 7 at `1/64` reaches four times as
/// far as level 7 at this value. A world that inserts [`CoplanarSettings`] replaces it.
pub const CANDIDATE_COPLANAR_THICKNESS: f32 = 1.0 / 256.0;

/// Why a priority or a thickness was refused.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CoplanarError {
    /// The level was above [`MAX_COPLANAR_PRIORITY`].
    PriorityOutOfRange { level: u8 },
    /// The thickness was NaN or infinite.
    ThicknessNotFinite { thickness: f32 },
    /// The thickness was finite and not strictly positive.
    ThicknessNotPositive { thickness: f32 },
}

impl fmt::Display for CoplanarError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CoplanarError::PriorityOutOfRange { level } => {
                write!(f, "priority {level} is outside 0..={MAX_COPLANAR_PRIORITY}")
            }
            CoplanarError::ThicknessNotFinite { thickness } => {
                write!(f, "thickness {thickness} is not finite")
            }
            CoplanarError::ThicknessNotPositive { thickness } => {
                write!(f, "thickness {thickness} must be greater than 0")
            }
        }
    }
}

impl std::error::Error for CoplanarError {}

/// Opt-in ordering for one sprite that shares a surface with other opted-in artwork.
///
/// Absence of this component is ordinary spatial depth. Level 0 is the first step toward the
/// camera, `(1 + level)` times the project thickness, and is not "no priority". Levels above
/// [`MAX_COPLANAR_PRIORITY`] are rejected by [`Self::new`] and are not clamped during extraction.
///
/// The value does not change blend-versus-blend order. That stays [`crate::RenderOrder`] and
/// camera-space depth. Meshes do not read this component.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CoplanarPriority(pub u8);

impl CoplanarPriority {
    /// A priority in `0..=7`.
    pub const fn new(level: u8) -> Result<Self, CoplanarError> {
        if level > MAX_COPLANAR_PRIORITY {
            Err(CoplanarError::PriorityOutOfRange { level })
        } else {
            Ok(Self(level))
        }
    }

    /// The stored level when it is inside `0..=7`.
    pub const fn checked(self) -> Result<u8, CoplanarError> {
        if self.0 > MAX_COPLANAR_PRIORITY {
            Err(CoplanarError::PriorityOutOfRange { level: self.0 })
        } else {
            Ok(self.0)
        }
    }
}

/// Project-wide step between two adjacent priorities, in world units.
///
/// One value for the whole world. A sprite's level selects a multiple of it. There is no
/// per-material or per-camera thickness.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CoplanarSettings {
    pub thickness: f32,
}

impl CoplanarSettings {
    /// A finite thickness strictly greater than zero.
    pub fn new(thickness: f32) -> Result<Self, CoplanarError> {
        let settings = Self { thickness };
        settings.checked()?;
        Ok(settings)
    }

    /// The stored thickness when it can be used as a world-unit step.
    pub fn checked(self) -> Result<f32, CoplanarError> {
        if !self.thickness.is_finite() {
            Err(CoplanarError::ThicknessNotFinite { thickness: self.thickness })
        } else if self.thickness <= 0.0 {
            Err(CoplanarError::ThicknessNotPositive { thickness: self.thickness })
        } else {
            Ok(self.thickness)
        }
    }
}

/// Thickness uploaded for this frame.
///
/// No resource uses [`CANDIDATE_COPLANAR_THICKNESS`]. An explicit value that fails
/// [`CoplanarSettings::checked`] is logged and becomes `0`, so opted-in sprites keep their real
/// depth instead of a substituted candidate. `0` does not move a sprite.
pub fn resolve_coplanar_thickness(settings: Option<CoplanarSettings>) -> f32 {
    match settings {
        None => CANDIDATE_COPLANAR_THICKNESS,
        Some(settings) => match settings.checked() {
            Ok(thickness) => thickness,
            Err(error) => {
                log::warn!("coplanar thickness {error}; opted-in sprites keep their real depth");
                0.0
            }
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn priority_accepts_zero_through_seven_and_refuses_the_next() {
        assert_eq!(CoplanarPriority::new(0).unwrap().0, 0);
        assert_eq!(CoplanarPriority::new(7).unwrap().checked().unwrap(), 7);
        assert_eq!(CoplanarPriority::new(8), Err(CoplanarError::PriorityOutOfRange { level: 8 }));
        assert_eq!(CoplanarPriority(9).checked(), Err(CoplanarError::PriorityOutOfRange { level: 9 }));
    }

    #[test]
    fn thickness_must_be_finite_and_positive() {
        assert_eq!(CoplanarSettings::new(1.0 / 256.0).unwrap().thickness, CANDIDATE_COPLANAR_THICKNESS);
        assert_eq!(CoplanarSettings::new(1.0 / 1024.0).unwrap().checked().unwrap(), 1.0 / 1024.0);
        assert!(matches!(CoplanarSettings::new(0.0), Err(CoplanarError::ThicknessNotPositive { .. })));
        assert!(matches!(CoplanarSettings::new(-0.1), Err(CoplanarError::ThicknessNotPositive { .. })));
        assert!(matches!(
            CoplanarSettings { thickness: f32::NAN }.checked(),
            Err(CoplanarError::ThicknessNotFinite { .. })
        ));
        assert!(matches!(
            CoplanarSettings { thickness: f32::INFINITY }.checked(),
            Err(CoplanarError::ThicknessNotFinite { .. })
        ));
    }

    #[test]
    fn a_missing_resource_uses_the_candidate_and_an_invalid_one_disables_displacement() {
        assert_eq!(resolve_coplanar_thickness(None), CANDIDATE_COPLANAR_THICKNESS);
        assert_eq!(resolve_coplanar_thickness(Some(CoplanarSettings { thickness: 1.0 / 64.0 })), 1.0 / 64.0);
        assert_eq!(resolve_coplanar_thickness(Some(CoplanarSettings { thickness: f32::NAN })), 0.0);
    }
}
