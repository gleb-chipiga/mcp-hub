//! Binary entry point for `mcp-hub`.

use anyhow::{Context, Result};

mod app;
mod config;
mod error;
mod hub;
mod runtime;
mod telemetry;

/// Uses `mimalloc` as the global allocator when the default feature is enabled.
#[cfg(feature = "mimalloc")]
#[global_allocator]
static GLOBAL_ALLOCATOR: mimalloc::MiMalloc = mimalloc::MiMalloc;

/// Boots tracing, builds the Tokio runtime explicitly, and runs the application entrypoint.
fn main() -> Result<()> {
    let _tracing = telemetry::init_tracing("info,mcp_hub=debug");
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("failed to build tokio runtime for mcp-hub")?;

    runtime
        .block_on(app::run())
        .context("mcp-hub terminated with an error")
}
