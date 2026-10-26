//! Renderer-wide sample count. Not a camera field and not a component.
//!
//! [`MsaaSamples::One`] is the default. [`MsaaSamples::Four`] is optional. There is no per-camera
//! or per-material count. Texture filtering, orthographic pixel snap, and integer-resolution
//! presentation stay independent of this setting.
//!
//! Stage 1 draws 4× only for a direct shaded surface camera that clears color. Other combinations
//! are skipped. The skip is not a silent drop to one sample, and it does not change the selected
//! count. Load expansion, depth presentation, offscreen targets, and internal resolution at 4×
//! are later work.

use crate::{CameraClear, CameraOutput, CameraPresentation, PresentationFit};

/// How many samples the renderer asks the backend to keep for scene color and depth.
///
/// This is coverage for geometry edges. It does not change texture samplers, and it does not
/// enable alpha-to-coverage. A mask or cutout edge still discards a whole fragment.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum MsaaSamples {
    /// One sample. The historical renderer path.
    #[default]
    One,
    /// Four samples. The backend must support this for its color format and for `Depth32Float`.
    Four,
}

impl MsaaSamples {
    /// The numeric sample count stored on GPU images and pipelines.
    pub const fn count(self) -> u32 {
        match self {
            Self::One => 1,
            Self::Four => 4,
        }
    }
}

/// A 4× request that was not applied. The previously selected count stays in effect.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MsaaError {
    /// The color format, `Depth32Float`, or both do not support 4 samples.
    ///
    /// `color` and `depth` say which of those two checks passed.
    Unavailable { color: bool, depth: bool },
}

impl std::fmt::Display for MsaaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unavailable { color, depth } => write!(
                f,
                "4× MSAA is not supported (color samples: {color}, depth samples: {depth}); the previous sample count is unchanged"
            ),
        }
    }
}

impl std::error::Error for MsaaError {}

/// Why one camera was not drawn while 4× is selected.
///
/// The renderer logs the reason and leaves the selected count at 4×. It does not draw that
/// camera at one sample.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MsaaSkip {
    /// [`crate::CameraClear::Load`] needs a multisample expansion of the destination. Not in stage 1.
    Load,
    /// [`crate::CameraPresentation::Depth`] needs a nearest reverse-Z sample reduction. Not in stage 1.
    DepthPresentation,
    /// An offscreen target must be sampled after it has been resolved. Not in stage 1.
    Offscreen,
    /// Internal resolution must resolve into the one-sample private image before the integer blit.
    /// Not in stage 1. Letterbox-only presentation is included.
    InternalResolution,
    /// The 4× color or depth image could not be allocated at this size.
    ///
    /// The count stays 4×. The camera is not promoted to a one-sample scene.
    ImagesUnavailable,
}

impl MsaaSkip {
    /// Diagnostic text for this camera. Stable enough for a test to match.
    pub fn message(self, camera_id: u32) -> String {
        let why = match self {
            Self::Load => {
                "CameraClear::Load is not supported at 4× until destination samples can be expanded; the camera was not drawn"
            }
            Self::DepthPresentation => {
                "CameraPresentation::Depth is not supported at 4× until nearest reverse-Z samples can be shown; the camera was not drawn"
            }
            Self::Offscreen => {
                "offscreen targets are not supported at 4× until a later camera can sample the resolved image; the camera was not drawn"
            }
            Self::InternalResolution => {
                "internal resolution is not supported at 4× until the multisample image resolves before the integer blit; the camera was not drawn"
            }
            Self::ImagesUnavailable => {
                "4× images were not allocated; the camera was not drawn and was not replaced with 1×"
            }
        };
        format!("camera {camera_id} {why}")
    }
}

/// Decides the next selected count.
///
/// `Ok` is the count to store. `Err` means the caller must keep `current`. A failed 4× request
/// does not become [`MsaaSamples::One`].
///
/// `color_supports_4` and `depth_supports_4` come from the adapter's format features. This
/// function does not look at a GPU. One sample does not consult them.
pub fn select_msaa(
    current: MsaaSamples,
    requested: MsaaSamples,
    color_supports_4: bool,
    depth_supports_4: bool,
) -> Result<MsaaSamples, MsaaError> {
    match requested {
        MsaaSamples::One => Ok(MsaaSamples::One),
        MsaaSamples::Four if color_supports_4 && depth_supports_4 => Ok(MsaaSamples::Four),
        MsaaSamples::Four => {
            let _ = current;
            Err(MsaaError::Unavailable { color: color_supports_4, depth: depth_supports_4 })
        }
    }
}

/// Stage-1 gate for one camera while 4× is selected.
///
/// [`MsaaSamples::One`] returns `None`: the one-sample path draws every combination it already
/// drew. [`MsaaSamples::Four`] returns `None` only for a direct, shaded, surface camera that
/// clears color. [`MsaaSkip::ImagesUnavailable`] is a device failure and is not decided here.
///
/// When more than one limit applies, the order is presentation, then output, then fit, then clear.
pub fn msaa4_camera_skip(
    samples: MsaaSamples,
    fit: PresentationFit,
    output: CameraOutput,
    clear: CameraClear,
    presentation: CameraPresentation,
) -> Option<MsaaSkip> {
    if samples != MsaaSamples::Four {
        return None;
    }
    if presentation == CameraPresentation::Depth {
        return Some(MsaaSkip::DepthPresentation);
    }
    if matches!(output, CameraOutput::Target(_)) {
        return Some(MsaaSkip::Offscreen);
    }
    if !matches!(fit, PresentationFit::Direct) {
        return Some(MsaaSkip::InternalResolution);
    }
    if clear != CameraClear::Color {
        return Some(MsaaSkip::Load);
    }
    None
}
