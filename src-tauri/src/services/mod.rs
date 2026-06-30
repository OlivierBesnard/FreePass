//! Business logic, Tauri-agnostic and unit-testable. Commands (the IPC surface)
//! stay thin and delegate here.

pub mod entries;
pub mod environments;
pub mod favicon;
pub mod generator;
pub mod local_channel;
pub mod projects;
pub mod vault;
