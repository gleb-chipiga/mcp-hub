//! Application bootstrap and top-level run loop for `mcp-hub`.

use std::path::PathBuf;

use anyhow::{Context, Result, anyhow};
use rmcp::{ServiceExt, transport::stdio};

use crate::{config::HubConfig, hub::HubServer};

/// Loads configuration, starts the hub server, and waits for the outward MCP session to end.
pub(crate) async fn run() -> Result<()> {
    let config_path = resolve_config_path()?;
    let config = HubConfig::load(&config_path)
        .await
        .with_context(|| format!("failed to load hub config from '{}'", config_path.display()))?;
    let hub = HubServer::from_config(config)
        .await
        .context("failed startup validation while building hub runtime from config")?;
    let service = hub
        .clone()
        .serve(stdio())
        .await
        .context("failed to initialize outward MCP server over stdio")?;

    let result = service.waiting().await;
    hub.shutdown().await;
    result.context("outward MCP server task terminated with an error")?;

    Ok(())
}

/// Resolves the configuration path from the first CLI arg or `MCP_HUB_CONFIG`.
fn resolve_config_path() -> Result<PathBuf> {
    std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("MCP_HUB_CONFIG").map(PathBuf::from))
        .ok_or_else(|| {
            anyhow!(
                "missing hub config path; pass it as the first CLI argument or set MCP_HUB_CONFIG"
            )
        })
}
