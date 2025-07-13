use anyhow::Result;
use clap::Parser;
use tracing::info;

mod server;
mod tcl_tools;
mod tcl_executor;
mod tcl_runtime;
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
    
    /// Select TCL runtime implementation
    #[arg(
        long, 
        value_name = "RUNTIME",
        help = "TCL runtime to use (molt|tcl). Can also be set via TCL_MCP_RUNTIME environment variable"
    )]
    runtime: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    // Determine runtime configuration
    let env_runtime = std::env::var("TCL_MCP_RUNTIME").ok();
    let runtime_config = match tcl_runtime::RuntimeConfig::from_args_and_env(
        args.runtime.as_deref(),
        env_runtime.as_deref(),
    ) {
        Ok(config) => config,
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    };

    // Show available runtimes if requested runtime is not available  
    let requested_available = runtime_config.runtime_type
        .as_ref()
        .map(|rt| rt.is_available())
        .unwrap_or(true);
    if !requested_available {
        let available = tcl_runtime::RuntimeConfig::available_runtimes();
        let available_names: Vec<&str> = available.iter().map(|r| r.as_str()).collect();
        eprintln!(
            "Warning: {} runtime not available. Available runtimes: {}",
            runtime_config.runtime_type.as_ref().map(|rt| rt.as_str()).unwrap_or("unknown"),
            available_names.join(", ")
        );
    }

    if args.privileged {
        info!("Starting TCL MCP Server in PRIVILEGED mode - full TCL access enabled");
    } else {
        info!("Starting TCL MCP Server in RESTRICTED mode - limited TCL access");
    }

    // Create and run the MCP server with privilege and runtime settings
    let server = match TclMcpServer::new_with_runtime(args.privileged, runtime_config) {
        Ok(server) => server,
        Err(e) => {
            eprintln!("Failed to create server: {}", e);
            std::process::exit(1);
        }
    };
    
    // Initialize persistence (load existing tools)
    if let Err(e) = server.initialize_persistence().await {
        tracing::warn!("Failed to initialize persistence: {}", e);
        // Continue without persistence rather than failing
    }
    
    // Handle stdio communication
    server.run_stdio().await?;

    Ok(())
}