//! Single Source of Truth for AirMedia application metadata.
//! Automatically synchronized from Cargo.toml at compile time.

pub const APP_NAME: &str = "AirMedia";
pub const APP_ID: &str = env!("CARGO_PKG_NAME");
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const APP_DESCRIPTION: &str = env!("CARGO_PKG_DESCRIPTION");
pub const APP_AUTHORS: &str = env!("CARGO_PKG_AUTHORS");
pub const APP_REPOSITORY: &str = env!("CARGO_PKG_REPOSITORY");
pub const APP_HOMEPAGE: &str = env!("CARGO_PKG_HOMEPAGE");
pub const APP_LICENSE: &str = env!("CARGO_PKG_LICENSE");
pub const APP_COPYRIGHT: &str = "Copyright (C) 2026 AirMedia Contributors";
pub const BUNDLE_ID: &str = "io.github.airmedia.player";
