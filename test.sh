#!/bin/bash
# Test the TCL MCP server

echo "Testing TCL MCP Server..."

# Test 1: Simple expression
echo '{"id": 1, "method": "tools/call", "params": {"name": "tcl_execute", "arguments": {"script": "expr 2 + 2"}}}' | ./target/debug/tcl-mcp-server | tail -1

# Test 2: String output
echo '{"id": 2, "method": "tools/call", "params": {"name": "tcl_execute", "arguments": {"script": "puts \"Hello from TCL\""}}}' | ./target/debug/tcl-mcp-server | tail -1

# Test 3: List tools
echo '{"id": 3, "method": "tools/list", "params": {}}' | ./target/debug/tcl-mcp-server | tail -1