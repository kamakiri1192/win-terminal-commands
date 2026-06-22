//! Shared library backing the `win-terminal-commands` binaries.
//!
//! Most commands are self-contained in `src/bin`, but `gzip` and `gunzip`
//! share their engine, so it lives here.

pub mod gzip;
