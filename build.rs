use std::env;
use std::fs;
use std::path::Path;

fn main() {
    let profile = env::var("PROFILE").unwrap();
    
    // Determine the target directory based on profile
    let target_dir = if profile == "release" {
        "target/release"
    } else {
        "target/debug"
    };
    
    // Create the wrapper script content
    let wrapper_content = format!(r#"#!/bin/bash
# TCL MCP Server Admin Wrapper
# Automatically enables privileged mode for tool management capabilities

# Get the directory where this script is located
SCRIPT_DIR="$(cd "$(dirname "${{BASH_SOURCE[0]}}")" && pwd)"

# Path to the main tcl-mcp-server binary
TCL_SERVER="${{SCRIPT_DIR}}/tcl-mcp-server"

# Check if the binary exists
if [ ! -f "$TCL_SERVER" ]; then
    echo "Error: tcl-mcp-server binary not found at: $TCL_SERVER" >&2
    echo "Please run 'cargo build{}' first" >&2
    exit 1
fi

# Execute the server with privileged mode enabled
exec "$TCL_SERVER" --privileged
"#, if profile == "release" { " --release" } else { "" });

    // Write the wrapper script to the target directory
    let wrapper_path = Path::new(target_dir).join("tcl-mcp-server-admin");
    fs::write(&wrapper_path, wrapper_content).expect("Failed to write wrapper script");
    
    // Make the wrapper executable on Unix systems
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&wrapper_path).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&wrapper_path, perms).expect("Failed to set wrapper permissions");
    }
    
    println!("cargo:rerun-if-changed=build.rs");
}