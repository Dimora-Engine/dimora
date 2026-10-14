//! The process logger. An application that installs its own `log` implementation first is left
//! alone, including the filter it chose.

use std::fmt::Display;

/// Installs a stderr logger at `Info` when no logger is installed yet.
///
/// A logger that is already installed stays installed, and its max level is not changed.
/// [`crate::Game::run`] calls this. Tests and applications that want a different logger call
/// [`log::set_logger`] before `run`.
pub fn install_stderr_logger() {
    if log::set_logger(&STDERR).is_ok() {
        log::set_max_level(log::LevelFilter::Info);
    }
}

pub(crate) fn log_window_init_failure(err: &dyn Display) {
    log::error!("window initialization failed: {err}");
}

pub(crate) fn log_render_failure(err: &dyn Display) {
    log::error!("render failed: {err}");
}

struct StderrLogger;

impl log::Log for StderrLogger {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        metadata.level() <= log::Level::Info
    }

    fn log(&self, record: &log::Record<'_>) {
        if self.enabled(record.metadata()) {
            eprintln!("{}: {}", record.level(), record.args());
        }
    }

    fn flush(&self) {
        use std::io::Write;
        let _ = std::io::stderr().flush();
    }
}

static STDERR: StderrLogger = StderrLogger;

#[cfg(test)]
mod tests {
    use super::*;
    use engine_renderer_wgpu::RenderError;

    mod capture {
        include!("../../log_capture.rs");
    }

    #[test]
    fn window_and_render_failures_log_their_cause() {
        capture::begin();
        log_window_init_failure(&RenderError::Surface("adapter rejected the surface".into()));
        log_render_failure(&RenderError::Device("device lost".into()));
        let messages = capture::messages();
        assert_eq!(messages.len(), 2, "{messages:?}");
        assert!(messages[0].contains("window initialization failed"));
        assert!(messages[0].contains("adapter rejected the surface"));
        assert_eq!(capture::records()[0].level, log::Level::Error);
        assert!(messages[1].contains("render failed"));
        assert!(messages[1].contains("device lost"));
    }

    #[test]
    fn an_application_logger_and_its_filter_stay_in_place() {
        capture::begin();
        let level = log::max_level();
        install_stderr_logger();
        assert_eq!(log::max_level(), level, "installing the default logger must not change the filter");
        log::warn!("application logger still receives records");
        let messages = capture::messages();
        assert!(
            messages.iter().any(|message| message.contains("application logger still receives records")),
            "{messages:?}"
        );
    }
}
