#![allow(dead_code)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::missing_panics_doc)]
#![allow(clippy::must_use_candidate)]
#![allow(clippy::should_implement_trait)]
#![allow(clippy::return_self_not_must_use)]
#![allow(clippy::module_name_repetitions)]

pub mod app;
pub mod cli;
pub mod config;
pub mod document;
pub mod editor;
pub mod errors;
pub mod estimates;
pub mod export;
pub mod gcode;
pub mod geometry;
pub mod history;
pub mod import;
pub mod layers;
pub mod materials;
pub mod mcp;
pub mod messages;
pub mod objects;
pub mod operations;
pub mod preview;
pub mod profiles;
pub mod storage;
pub mod ui;
