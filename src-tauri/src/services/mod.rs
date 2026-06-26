//! Business logic, Tauri-agnostic and unit-testable. Commands (the IPC surface)
//! stay thin and delegate here.

pub mod entries;
pub mod generator;
pub mod local_channel;
pub mod vault;
