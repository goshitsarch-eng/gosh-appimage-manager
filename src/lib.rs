// Gosh AppImage Manager 3.0.0 — core library. Made by Gosh.
// GPL-3.0-or-later. Opening an AppImage never integrates or executes it.

pub mod controller;
pub mod desktop;
pub mod diagnostics;
pub mod drop_queue;
pub mod elf;
pub mod i18n;
pub mod inspector;
pub mod integration;
pub mod launch;
pub mod library;
pub mod limits;
pub mod network;
pub mod notifier;
pub mod process;
pub mod proctable;
pub mod registry;
pub mod removal;
pub mod safe_fs;
pub mod settings;
pub mod tasks;
pub mod trash;
pub mod types;
pub mod updates_service;
pub mod updates_sources;
pub mod url_guard;

pub mod cli;

#[cfg(feature = "gui")]
pub mod gui;
