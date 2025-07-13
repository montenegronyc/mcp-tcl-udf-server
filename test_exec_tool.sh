#!/bin/bash
# Test script for bin__exec_tool functionality

echo "Testing bin__exec_tool implementation..."
echo

# Build the project first
echo "Building project..."
cargo build
if [ $? -ne 0 ]; then
    echo "Build failed!"
    exit 1
fi

# Start the server in the background
echo "Starting TCL MCP server..."
./target/debug/tcl-mcp-server --privileged > server.log 2>&1 &
SERVER_PID=$!

# Give server time to start
sleep 2

# Test 1: Discover tools
echo "Test 1: Discovering tools..."
echo '{"jsonrpc":"2.0","method":"tools/call","params":{"name":"bin___discover_tools","arguments":{}},"id":1}' | nc -N localhost 8080

echo
echo "Test 2: List tools in bin namespace..."
echo '{"jsonrpc":"2.0","method":"tools/call","params":{"name":"bin___tcl_tool_list","arguments":{"namespace":"bin"}},"id":2}' | nc -N localhost 8080

echo
echo "Test 3: Execute hello_world tool..."
echo '{"jsonrpc":"2.0","method":"tools/call","params":{"name":"bin___exec_tool","arguments":{"tool_path":"/bin/hello_world","params":{"name":"TCL MCP"}}},"id":3}' | nc -N localhost 8080

echo
echo "Test 4: Execute list_dir tool..."
echo '{"jsonrpc":"2.0","method":"tools/call","params":{"name":"bin___exec_tool","arguments":{"tool_path":"/bin/list_dir","params":{"path":"./tools/bin"}}},"id":4}' | nc -N localhost 8080

# Clean up
echo
echo "Stopping server..."
kill $SERVER_PID 2>/dev/null

echo "Tests complete!"