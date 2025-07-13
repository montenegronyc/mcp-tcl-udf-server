#!/usr/bin/env python3
"""
Test script for TCL MCP Server
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
    print("Starting TCL MCP Server test...")
    
    # Start the server
    proc = subprocess.Popen(
        ['cargo', 'run'],
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
        
        # Test 3: Execute TCL script
        print("\n3. Testing tcl_execute...")
        response = send_request(proc, {
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": {
                "name": "tcl_execute",
                "arguments": {
                    "script": "expr 2 + 2"
                }
            }
        })
        
        # Test 4: Add a custom tool
        print("\n4. Testing tcl_tool_add...")
        response = send_request(proc, {
            "jsonrpc": "2.0",
            "id": 4,
            "method": "tools/call",
            "params": {
                "name": "tcl_tool_add",
                "arguments": {
                    "name": "greet",
                    "description": "Greet a person by name",
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
        
        # Test 5: List tools again (should include custom tool)
        print("\n5. Testing tools/list after adding custom tool...")
        response = send_request(proc, {
            "jsonrpc": "2.0",
            "id": 5,
            "method": "tools/list",
            "params": {}
        })
        
        # Test 6: Call custom tool
        print("\n6. Testing custom tool...")
        response = send_request(proc, {
            "jsonrpc": "2.0",
            "id": 6,
            "method": "tools/call",
            "params": {
                "name": "greet",
                "arguments": {
                    "name": "World"
                }
            }
        })
        
        # Test 7: Test tcl_tool_list
        print("\n7. Testing tcl_tool_list...")
        response = send_request(proc, {
            "jsonrpc": "2.0",
            "id": 7,
            "method": "tools/call",
            "params": {
                "name": "tcl_tool_list",
                "arguments": {}
            }
        })
        
        # Test 8: Remove custom tool
        print("\n8. Testing tcl_tool_remove...")
        response = send_request(proc, {
            "jsonrpc": "2.0",
            "id": 8,
            "method": "tools/call",
            "params": {
                "name": "tcl_tool_remove",
                "arguments": {
                    "name": "greet"
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