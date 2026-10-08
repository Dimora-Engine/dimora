//! Platform- and GPU-independent runtime primitives.
//!
//! Nothing here knows about windows or rendering; the platform layer feeds [`Input`] and
//! [`Time`], and gameplay code reads them.

mod color;
mod input;
mod time;

pub use color::Color;
pub use input::{Input, Key, MouseButton};
pub use time::{FixedTimestep, Time};
