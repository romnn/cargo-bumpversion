//! `cargo-bumpversion` subcommand integration for bumpversion CLI.
//!
//! Skips leading `cargo` arguments and invokes the bumpversion logic.
#![forbid(unsafe_code)]

mod common;
mod logging;
mod options;
mod verbose;

use clap::Parser;
use color_eyre::eyre;

/// Main entry point for `cargo-bumpversion`.
///
/// Parses arguments after `cargo bumpversion` and delegates to common logic.
#[tokio::main]
async fn main() -> eyre::Result<()> {
    color_eyre::install()?;

    let bin_name = env!("CARGO_BIN_NAME");
    let bin_name = bin_name.strip_prefix("cargo-").unwrap_or(bin_name);

    let args: Vec<String> = std::env::args_os()
        // skip executable name
        .skip(1)
        // skip our own cargo-* command name
        .skip_while(|arg| {
            let arg = arg.as_os_str();
            arg == bin_name || arg == "cargo"
        })
        .map(|s| s.to_string_lossy().to_string())
        .collect();

    let mut options = options::Options::parse_from(args);
    options::fix(&mut options);
    common::bumpversion(options).await
}
