// Thread-local capture of `log` records for tests.
//
// Include this file from a `#[cfg(test)]` module. `begin` installs one process-wide capturing
// logger on first use and then drops records that belong to the calling thread. Other threads
// keep their own records, so parallel tests do not observe each other. The install runs once per
// process; call it before any other logger in that process.

use std::sync::{Mutex, Once};
use std::thread::ThreadId;

pub struct Record {
    pub level: log::Level,
    pub message: String,
}

struct Stored {
    thread: ThreadId,
    level: log::Level,
    message: String,
}

static INSTALL: Once = Once::new();
static RECORDS: Mutex<Vec<Stored>> = Mutex::new(Vec::new());

struct Capture;

impl log::Log for Capture {
    fn enabled(&self, _metadata: &log::Metadata<'_>) -> bool {
        true
    }

    fn log(&self, record: &log::Record<'_>) {
        if !self.enabled(record.metadata()) {
            return;
        }
        RECORDS.lock().expect("log capture lock").push(Stored {
            thread: std::thread::current().id(),
            level: record.level(),
            message: record.args().to_string(),
        });
    }

    fn flush(&self) {}
}

static CAPTURE: Capture = Capture;

pub fn begin() {
    INSTALL.call_once(|| {
        log::set_logger(&CAPTURE).expect("a logger is already installed in this test process");
        log::set_max_level(log::LevelFilter::Trace);
    });
    let id = std::thread::current().id();
    RECORDS.lock().expect("log capture lock").retain(|record| record.thread != id);
}

pub fn records() -> Vec<Record> {
    let id = std::thread::current().id();
    RECORDS
        .lock()
        .expect("log capture lock")
        .iter()
        .filter(|record| record.thread == id)
        .map(|record| Record { level: record.level, message: record.message.clone() })
        .collect()
}

pub fn messages() -> Vec<String> {
    records().into_iter().map(|record| record.message).collect()
}
