//! Renderer-wide sample count. Not a camera field and not a component.
//!
//! [`MsaaSamples::One`] is the default. [`MsaaSamples::Four`] is optional. There is no per-camera
//! or per-material count. Texture filtering, orthographic pixel snap, and integer-resolution
//! presentation stay independent of this setting.
//!
//! Four samples draw the same camera combinations as one sample. A size the device limits reject
//! skips that camera and keeps the selected count. It does not draw the camera at one sample.

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
/// camera at one sample. Load, depth presentation, offscreen targets, and internal resolution are
/// drawn. This reason is only an image that could not be allocated.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MsaaSkip {
    /// The 4× color or depth image could not be allocated at this size.
    ///
    /// The count stays 4×. The camera is not promoted to a one-sample scene.
    ImagesUnavailable,
}

impl MsaaSkip {
    /// Diagnostic text for this camera. Stable enough for a test to match.
    pub fn message(self, camera_id: u32) -> String {
        let why = match self {
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

/// Configuration gate for one camera.
///
/// Returns `None` for every combination the one-sample path draws, at both sample counts.
/// [`MsaaSkip::ImagesUnavailable`] is a device failure and is not decided here. `Load` with an
/// internal resolution is rejected before a pass exists, by presentation fit, not by this function.
pub fn msaa4_camera_skip(
    samples: MsaaSamples,
    fit: PresentationFit,
    output: CameraOutput,
    clear: CameraClear,
    presentation: CameraPresentation,
) -> Option<MsaaSkip> {
    let _ = (samples, fit, output, clear, presentation);
    None
}
