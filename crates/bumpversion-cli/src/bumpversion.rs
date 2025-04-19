#![forbid(unsafe_code)]

mod common;
mod logging;
mod options;
mod verbose;

use clap::Parser;
use color_eyre::eyre;

#[tokio::main]
async fn main() -> eyre::Result<()> {
    color_eyre::install()?;

    let mut options = options::Options::parse();
    options::fix(&mut options);
    common::bumpversion(options).await
}
