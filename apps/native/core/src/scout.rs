//! Scout: a teammate that keeps watch, brings morning briefs, weekly tune-ups, and keeps notes.
//!
//! Port of Electron's `main/scout.ts`, `main/scout-folder.ts`, `main/scout-github.ts`,
//! `shared/scout.ts`, and `shared/scout-log.ts`.

pub mod folder;
pub mod github;
pub mod log;
pub mod store;
pub mod types;

#[cfg(test)]
mod unit_tests;
