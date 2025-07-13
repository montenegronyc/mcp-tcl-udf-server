# TCL MCP Server with Namespace Support

A Model Context Protocol (MCP) server that provides TCL (Tool Command Language) execution capabilities with namespace-based tool management and versioning.

This currently uses the Molt engine which provides a limited subset of TCL that is slightly safer than an official TCL integration.

Coming soon: Build-time support for a full TCL interpreter.

(Caution this is "vibe coded" and allows for autonomous AI development and execution on target systems. Use with caution)

## Overview

This server provides TCL script execution through MCP with a Unix-like namespace system for organizing tools:
- `/bin/` - System tools (read-only)
- `/sbin/` - System administration tools (privileged)
- `/docs/` - Documentation tools (read-only)
- `/<user>/<package>/<tool>:<version>` - User tools with versioning

## Features

- **Namespace Organization**: Unix-like path structure for tool organization
- **Version Support**: Tools can have specific versions or use "latest"
- **Protected System Tools**: System tools in `/bin` and `/sbin` cannot be removed
- **User Tool Management**: Users can create tools in their own namespaces
- **Built-in Documentation**: Access Molt TCL interpreter docs and examples via `docs___molt_book`
- **MCP-Compatible Naming**: Internal namespace paths are converted to MCP-compatible names
- **Thread-Safe Architecture**: Handles TCL's non-thread-safe interpreter safely

## Namespace System

### Tool Paths
Tools are organized using a path-like structure:
- `/bin/tcl_execute` - Execute TCL scripts (system tool)
- `/bin/tcl_tool_list` - List available tools
- `/docs/molt_book` - Access Molt TCL documentation
- `/sbin/tcl_tool_add` - Add new tools (admin)
- `/sbin/tcl_tool_remove` - Remove tools (admin)
- `/alice/utils/reverse_string:1.0` - User tool with version
- `/bob/math/calculate:latest` - User tool with latest version

### MCP Name Mapping
Since MCP doesn't support forward slashes in tool names, paths are converted:
- `/bin/tcl_execute` → `bin___tcl_execute`
- `/docs/molt_book` → `docs___molt_book`
- `/sbin/tcl_tool_add` → `sbin___tcl_tool_add`
- `/alice/utils/reverse_string:1.0` → `user_alice__utils___reverse_string__v1_0`
- `/bob/math/calculate:latest` → `user_bob__math___calculate`

## Installation

```bash
cargo build --release
```

## Usage

### Running the Server

#### Restricted Mode (Default)
```bash
cargo run
# or
./target/release/tcl-mcp-server
```

In restricted mode:
- Only `tcl_execute`, `tcl_tool_list`, and `docs___molt_book` tools are available  
- Tool management commands (`tcl_tool_add`, `tcl_tool_remove`) are disabled
- Provides safer TCL execution environment

#### Privileged Mode
```bash
cargo run -- --privileged
# or  
./target/release/tcl-mcp-server --privileged
# or use the generated wrapper (recommended for MCP integration)
./target/release/tcl-mcp-server-admin
```

In privileged mode:
- All tools are available including tool management
- Full TCL language access via Molt interpreter
- Enables dynamic tool creation and removal
- **Use with caution** - provides full TCL access

#### Admin Wrapper Script
The build process automatically generates `tcl-mcp-server-admin` wrapper script that enables privileged mode. This is useful for MCP integration since Claude's MCP configuration doesn't support command-line arguments:

```bash
# Generated automatically during build
./target/release/tcl-mcp-server-admin  # Equivalent to: tcl-mcp-server --privileged
./target/debug/tcl-mcp-server-admin    # Debug version
```

#### Command Line Options
```bash
./target/release/tcl-mcp-server --help
```

```
TCL MCP Server - Execute TCL scripts via Model Context Protocol

Usage: tcl-mcp-server [OPTIONS]

Options:
      --privileged  Enable privileged mode with full TCL access and tool management capabilities
  -h, --help        Print help
  -V, --version     Print version
```

### System Tools

#### 1. bin___tcl_execute *(Available in both modes)*
Execute a TCL script (path: `/bin/tcl_execute`)

```json
{
  "tool": "bin___tcl_execute",
  "parameters": {
    "script": "expr {2 + 2}"
  }
}
```

#### 2. bin___tcl_tool_list *(Available in both modes)*
List all available TCL tools (path: `/bin/tcl_tool_list`)

```json
{
  "tool": "bin___tcl_tool_list",
  "parameters": {
    "namespace": "alice",  // optional filter
    "filter": "utils*"     // optional name pattern
  }
}
```

#### 3. sbin___tcl_tool_add *(Privileged mode only)*
Add a new tool to a user namespace (path: `/sbin/tcl_tool_add`)

```json
{
  "tool": "sbin___tcl_tool_add",
  "parameters": {
    "user": "alice",
    "package": "utils",
    "name": "reverse_string",
    "version": "1.0",
    "description": "Reverse a string",
    "script": "return [string reverse $text]",
    "parameters": [{
      "name": "text",
      "description": "Text to reverse",
      "required": true,
      "type_name": "string"
    }]
  }
}
```

#### 4. sbin___tcl_tool_remove *(Privileged mode only)*
Remove a user tool (path: `/sbin/tcl_tool_remove`)

```json
{
  "tool": "sbin___tcl_tool_remove",
  "parameters": {
    "path": "/alice/utils/reverse_string:1.0"
  }
}
```

#### 5. docs___molt_book *(Available in both modes)*
Access Molt TCL interpreter documentation and examples (path: `/docs/molt_book`)

```json
{
  "tool": "docs___molt_book",
  "parameters": {
    "topic": "overview"  // Available: overview, basic_syntax, commands, examples, links
  }
}
```

**Available topics:**
- `overview` - Introduction to Molt TCL interpreter
- `basic_syntax` - TCL syntax fundamentals (variables, lists, control structures)
- `commands` - Common TCL commands reference
- `examples` - Practical TCL code examples
- `links` - Links to official Molt documentation and resources

### Custom Tool Example

1. **Add a tool to your namespace:**
```json
{
  "tool": "sbin___tcl_tool_add",
  "parameters": {
    "user": "myuser",
    "package": "text",
    "name": "word_count",
    "version": "1.0",
    "description": "Count words in text",
    "script": "return [llength [split $text]]",
    "parameters": [{
      "name": "text",
      "description": "Text to count words in",
      "required": true,
      "type_name": "string"
    }]
  }
}
```

2. **Use the tool (MCP name):**
```json
{
  "tool": "user_myuser__text___word_count__v1_0",
  "parameters": {
    "text": "Hello world from TCL"
  }
}
```

Result: `4`

## Architecture

```
┌─────────────┐     ┌──────────────┐     ┌─────────────┐
│  MCP Client ├────►│  MCP Server  ├────►│TCL Executor │
│  (stdio)    │     │  (jsonrpc)   │     │  (thread)   │
└─────────────┘     └──────────────┘     └─────────────┘
                          │                      │
                          │  Namespace Manager   │
                          │  - Path mapping      │
                          │  - Version control   │
                          │  - Access control    │
                          └──────────────────────┘
```

## Security Considerations

### Privileged vs Restricted Mode

**Restricted Mode (Default - Recommended)**:
- Only `tcl_execute` and `tcl_tool_list` tools are available
- Tool management commands are disabled
- Safer for production environments
- Limits potential attack surface

**Privileged Mode**:
- ⚠️ **Use with caution** - provides full TCL interpreter access
- Enables dynamic tool creation and removal
- Should only be used in trusted environments
- Consider additional sandboxing for untrusted input

### General Security

- System tools in `/bin` and `/sbin` cannot be removed
- Only `/sbin` tools can manage the tool registry (privileged mode only)
- User tools are isolated in user namespaces
- TCL execution is handled by the Molt interpreter (memory-safe Rust implementation)
- Consider running in a container or sandbox for production use
- Always validate input when accepting TCL scripts from untrusted sources

## MCP Configuration

### Claude Desktop Integration

Add to your Claude Desktop `settings.json`:

**Restricted Mode (Recommended):**
```json
{
  "mcpServers": {
    "tcl": {
      "command": "/path/to/tcl-mcp/target/release/tcl-mcp-server"
    }
  }
}
```

**Privileged Mode (Admin):**
```json
{
  "mcpServers": {
    "tcl-admin": {
      "command": "/path/to/tcl-mcp/target/release/tcl-mcp-server-admin"
    }
  }
}
```

### Claude Code MCP Integration

```bash
# Add restricted server
claude mcp add tcl /path/to/tcl-mcp/target/release/tcl-mcp-server

# Add privileged server
claude mcp add tcl-admin /path/to/tcl-mcp/target/release/tcl-mcp-server-admin
```

## Testing

Two test scripts are provided:

```bash
# Basic functionality test
python3 test_mcp.py

# Namespace functionality test
python3 test_namespace.py
```

## Building a Container Image

```dockerfile
FROM rust:1.70 as builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
# Copy both the main binary and admin wrapper
COPY --from=builder /app/target/release/tcl-mcp-server /usr/bin/
COPY --from=builder /app/target/release/tcl-mcp-server-admin /usr/bin/
# Default to restricted mode
CMD ["/usr/bin/tcl-mcp-server"]
```

For privileged mode container:
```dockerfile
CMD ["/usr/bin/tcl-mcp-server-admin"]
```

## LLM Compatibility

The namespace system is designed to be LLM-friendly:
- Tool names follow a consistent pattern
- Namespace paths provide semantic organization
- Version information is preserved in the MCP name
- Descriptions include the full path for clarity

When tools are listed, they include both the MCP name and the namespace path:
```
"name": "user_alice__utils___reverse_string__v1_0",
"description": "Reverse a string [/alice/utils/reverse_string:1.0]"
```

## License

MIT
