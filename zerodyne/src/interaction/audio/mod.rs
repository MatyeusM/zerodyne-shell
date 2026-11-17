//! Audio interaction handlers, ported from the Go daemon's
//! `internal/interaction/audio` package. Public types are re-exported here so
//! callers keep using `crate::interaction::audio::VolumeHandler` unchanged.

mod common;
mod defaults;
mod switch;
mod volume;

pub use defaults::{InputDefaultHandler, OutputDefaultHandler};
pub use switch::OutputSwitchHandler;
pub use volume::{InputMuteHandler, VolumeHandler};
