#!/usr/bin/env python3
"""
Test script for TCL MCP Server with correct tool names
"""

import json
import subprocess
import sys
import time

def send_request(proc, request):
    """Send a JSON-RPC request and get response"""
    request_str = json.dumps(request)
    print(f"→ Request: {request_str}")
    
    proc.stdin.write(request_str + '\n')
    proc.stdin.flush()
    
    response_str = proc.stdout.readline()
    print(f"← Response: {response_str}")
    
    try:
        return json.loads(response_str)
    except json.JSONDecodeError as e:
        print(f"Failed to parse response: {e}")
        return None

def main():
    print("Starting TCL MCP Server test with correct tool names...")
    
    # Start the server (privileged mode for tool management tests)
    proc = subprocess.Popen(
        ['./target/debug/tcl-mcp-server-admin'],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        bufsize=1
    )
    
    # Give server time to start
    time.sleep(2)
    
    try:
        # Test 1: Initialize
        print("\n1. Testing initialize...")
        response = send_request(proc, {
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {}
        })
        
        # Test 2: List tools
        print("\n2. Testing tools/list...")
        response = send_request(proc, {
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/list",
            "params": {}
        })
        
        # Test 3: Execute TCL script (correct tool name)
        print("\n3. Testing bin___tcl_execute...")
        response = send_request(proc, {
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": {
                "name": "bin___tcl_execute",
                "arguments": {
                    "script": "puts \"Hello from TCL!\"; expr {2 + 2}"
                }
            }
        })
        
        # Test 4: Test bin___tcl_tool_list 
        print("\n4. Testing bin___tcl_tool_list...")
        response = send_request(proc, {
            "jsonrpc": "2.0",
            "id": 4,
            "method": "tools/call",
            "params": {
                "name": "bin___tcl_tool_list",
                "arguments": {}
            }
        })
        
        # Test 5: Test docs___molt_book
        print("\n5. Testing docs___molt_book...")
        response = send_request(proc, {
            "jsonrpc": "2.0",
            "id": 5,
            "method": "tools/call",
            "params": {
                "name": "docs___molt_book",
                "arguments": {
                    "topic": "basic_syntax"
                }
            }
        })
        
        # Test 6: Add a test tool (privileged mode)
        print("\n6. Testing sbin___tcl_tool_add...")
        response = send_request(proc, {
            "jsonrpc": "2.0",
            "id": 6,
            "method": "tools/call",
            "params": {
                "name": "sbin___tcl_tool_add",
                "arguments": {
                    "user": "testuser",
                    "package": "utils",
                    "name": "greet_test",
                    "version": "1.0",
                    "description": "Test greeting tool",
                    "script": "return \"Hello, $name!\"",
                    "parameters": [{
                        "name": "name",
                        "description": "Person's name",
                        "required": True,
                        "type_name": "string"
                    }]
                }
            }
        })
        
        # Test 7: Use bin___exec_tool to execute the new tool
        print("\n7. Testing bin___exec_tool...")
        response = send_request(proc, {
            "jsonrpc": "2.0",
            "id": 7,
            "method": "tools/call",
            "params": {
                "name": "bin___exec_tool",
                "arguments": {
                    "tool_path": "/testuser/utils/greet_test:1.0",
                    "params": {
                        "name": "World"
                    }
                }
            }
        })
        
        print("\nAll tests completed!")
        
    finally:
        # Cleanup
        proc.terminate()
        proc.wait()

if __name__ == "__main__":
    main()