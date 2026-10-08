//! The bridge API the Flutter front end calls. Every function is a coarse
//! operation; every type is a plain DTO. Business behaviour stays in the core.

pub mod common;
pub mod dto;
pub mod inspect;
pub mod integrate;
pub mod library;
pub mod settings;
pub mod system;
pub mod updates;
