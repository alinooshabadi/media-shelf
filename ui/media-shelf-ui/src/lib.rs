//! Media Shelf desktop UI on CUI.
//!
//! Layer 1 (`ffi`): raw bindings generated from the vendored CUI v1.4.0 header.
//! Layer 2 (`ui`): small Rust wrapper — owned option structs, string handling,
//! begin/end scopes, and the `cui_app_run` entry point.
//! Layer 3 (`app`, `library`, `series`, …): the actual screens, ported from
//! the egui implementation over the unchanged `media-shelf` core.
//!
//! String contract: every CUI component copies its text into per-frame memory
//! (`cui__frame_strdup`), so `&str` borrowed for the call is always safe.
//! Editable buffers are Rust-owned byte vectors with a guaranteed NUL
//! terminator; `cap` includes the terminator, per CUI's edit code.

#![allow(clippy::all)]

#[allow(non_camel_case_types, non_snake_case, non_upper_case_globals, dead_code, missing_docs)]
pub mod ffi;
pub mod ui;

pub mod app;
pub mod library;
pub mod series;

pub use ui::*;
