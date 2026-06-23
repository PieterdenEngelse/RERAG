//! Path resolution and sandbox-gate helper.
//!
//! Historical home of `Paths` + `skip_systemctl`. The real bodies live
//! under `crate::platform::{linux,windows}`; this file is a thin
//! re-export so every existing `use crate::paths::{Paths, …}` call site
//! keeps working without an edit. The OS-specific implementations are
//! selected by `crate::platform::mod`.

pub use crate::platform::{skip_systemctl, Paths};
