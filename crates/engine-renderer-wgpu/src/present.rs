//! Choosing a surface present mode from the modes a surface actually lists.
//!
//! VSync on asks for [`SurfacePresent::Fifo`]. VSync off asks for [`SurfacePresent::Immediate`],
//! then [`SurfacePresent::Mailbox`], then Fifo. The effective mode is whatever this selection
//! returns. A surface that cannot disable VSync therefore stays on Fifo.

use wgpu::PresentMode as WgpuPresent;

/// How the window surface queues frames. This is the engine's mode, not a wgpu type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SurfacePresent {
    /// Wait for vertical blank. No tearing.
    Fifo,
    /// Present as soon as the image is ready. May tear.
    Immediate,
    /// Replace the queued frame and still wait for blank. No tearing.
    Mailbox,
}

/// The mode to configure for this VSync request, given the modes the surface listed.
///
/// An empty list selects Fifo. That is the wgpu default, and it is the mode used until a
/// surface has reported its capabilities.
pub fn choose_surface_present(vsync: bool, available: &[SurfacePresent]) -> SurfacePresent {
    if vsync {
        if available.is_empty() || available.contains(&SurfacePresent::Fifo) {
            SurfacePresent::Fifo
        } else {
            available[0]
        }
    } else if available.contains(&SurfacePresent::Immediate) {
        SurfacePresent::Immediate
    } else if available.contains(&SurfacePresent::Mailbox) {
        SurfacePresent::Mailbox
    } else {
        SurfacePresent::Fifo
    }
}

/// Collapses wgpu's present-mode list to the three modes the engine selects among.
///
/// `AutoVsync` and `FifoRelaxed` count as Fifo. `AutoNoVsync` counts as Mailbox.
pub fn surface_presents_from_wgpu(modes: &[WgpuPresent]) -> Vec<SurfacePresent> {
    let mut out = Vec::new();
    for mode in modes {
        let present = match mode {
            WgpuPresent::Fifo | WgpuPresent::FifoRelaxed | WgpuPresent::AutoVsync => SurfacePresent::Fifo,
            WgpuPresent::Immediate => SurfacePresent::Immediate,
            WgpuPresent::Mailbox | WgpuPresent::AutoNoVsync => SurfacePresent::Mailbox,
        };
        if !out.contains(&present) {
            out.push(present);
        }
    }
    out
}

/// The wgpu mode to write into the surface configuration for an engine choice.
///
/// Prefers the exact variant. Fifo may be satisfied by `AutoVsync` or `FifoRelaxed` when the
/// surface did not list `Fifo` itself.
pub fn wgpu_present_for(choice: SurfacePresent, modes: &[WgpuPresent]) -> WgpuPresent {
    let preferred: &[WgpuPresent] = match choice {
        SurfacePresent::Fifo => &[WgpuPresent::Fifo, WgpuPresent::AutoVsync, WgpuPresent::FifoRelaxed],
        SurfacePresent::Immediate => &[WgpuPresent::Immediate],
        SurfacePresent::Mailbox => &[WgpuPresent::Mailbox, WgpuPresent::AutoNoVsync],
    };
    preferred.iter().copied().find(|mode| modes.contains(mode)).unwrap_or(preferred[0])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vsync_on_selects_fifo_and_off_falls_through_immediate_then_mailbox() {
        let all = [SurfacePresent::Fifo, SurfacePresent::Immediate, SurfacePresent::Mailbox];
        assert_eq!(choose_surface_present(true, &all), SurfacePresent::Fifo);
        assert_eq!(choose_surface_present(false, &all), SurfacePresent::Immediate);
        assert_eq!(
            choose_surface_present(false, &[SurfacePresent::Mailbox, SurfacePresent::Fifo]),
            SurfacePresent::Mailbox
        );
        assert_eq!(choose_surface_present(false, &[SurfacePresent::Fifo]), SurfacePresent::Fifo);
        assert_eq!(choose_surface_present(true, &[]), SurfacePresent::Fifo);
        assert_eq!(choose_surface_present(false, &[]), SurfacePresent::Fifo);
        assert_eq!(choose_surface_present(true, &[SurfacePresent::Immediate]), SurfacePresent::Immediate);
    }

    #[test]
    fn wgpu_modes_collapse_and_map_back_without_assuming_immediate_exists() {
        let listed = [WgpuPresent::AutoVsync, WgpuPresent::Mailbox];
        assert_eq!(surface_presents_from_wgpu(&listed), vec![SurfacePresent::Fifo, SurfacePresent::Mailbox]);
        assert_eq!(choose_surface_present(true, &surface_presents_from_wgpu(&listed)), SurfacePresent::Fifo);
        assert_eq!(wgpu_present_for(SurfacePresent::Fifo, &listed), WgpuPresent::AutoVsync);
        assert_eq!(choose_surface_present(false, &surface_presents_from_wgpu(&listed)), SurfacePresent::Mailbox);
        assert_eq!(wgpu_present_for(SurfacePresent::Mailbox, &listed), WgpuPresent::Mailbox);
        assert_eq!(wgpu_present_for(SurfacePresent::Immediate, &listed), WgpuPresent::Immediate);
    }
}
