use anyhow::Result;
use clap::Parser;
use tracing::info;

mod server;
mod tcl_tools;
mod tcl_executor;
mod namespace;
mod persistence;
mod tool_discovery;

use server::TclMcpServer;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
#[command(about = "TCL MCP Server - Execute TCL scripts via Model Context Protocol")]
struct Args {
    /// Enable privileged mode (full TCL language access and tool management)
    #[arg(long, help = "Enable privileged mode with full TCL access and tool management capabilities")]
    privileged: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    if args.privileged {
        info!("Starting TCL MCP Server in PRIVILEGED mode - full TCL access enabled");
    } else {
        info!("Starting TCL MCP Server in RESTRICTED mode - limited TCL access");
    }

    // Create and run the MCP server with privilege settings
    let server = TclMcpServer::new(args.privileged);
    
    // Initialize persistence (load existing tools)
    if let Err(e) = server.initialize_persistence().await {
        tracing::warn!("Failed to initialize persistence: {}", e);
        // Continue without persistence rather than failing
    }
    
    // Handle stdio communication
    server.run_stdio().await?;

    Ok(())
}