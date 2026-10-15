//! Platform- and GPU-independent runtime primitives.
//!
//! Nothing here knows about windows or rendering; the platform layer feeds [`Input`] and
//! [`Time`], and gameplay code reads them. [`Actions`] maps those devices onto named controls.

mod actions;
mod color;
mod input;
mod time;

pub use actions::{Actions, ButtonSource};
pub use color::Color;
pub use input::{Input, InputEvent, Key, MouseButton};
pub use time::{FixedTimestep, Time};
