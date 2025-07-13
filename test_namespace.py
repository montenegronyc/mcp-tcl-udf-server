#!/usr/bin/env python3
"""
Test script for TCL MCP Server with Namespace Support
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
    print("Starting TCL MCP Server namespace test...")
    
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
        
        # Test 2: List tools (should show namespace paths)
        print("\n2. Testing tools/list...")
        response = send_request(proc, {
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/list",
            "params": {}
        })
        
        if response and 'result' in response:
            print("\nAvailable tools:")
            for tool in response['result']['tools']:
                print(f"  - {tool['name']} : {tool.get('description', 'No description')}")
        
        # Test 3: Execute system tool from /bin
        print("\n3. Testing bin___tcl_execute...")
        response = send_request(proc, {
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": {
                "name": "bin___tcl_execute",
                "arguments": {
                    "script": "expr 2 + 2"
                }
            }
        })
        
        # Test 4: Add a custom tool with namespace
        print("\n4. Testing sbin___tcl_tool_add...")
        response = send_request(proc, {
            "jsonrpc": "2.0",
            "id": 4,
            "method": "tools/call",
            "params": {
                "name": "sbin___tcl_tool_add",
                "arguments": {
                    "user": "alice",
                    "package": "utils",
                    "name": "reverse_string",
                    "version": "1.0",
                    "description": "Reverse a string",
                    "script": "return [string reverse $text]",
                    "parameters": [{
                        "name": "text",
                        "description": "Text to reverse",
                        "required": True,
                        "type_name": "string"
                    }]
                }
            }
        })
        
        # Test 5: Add another tool in different user namespace
        print("\n5. Adding tool to bob's namespace...")
        response = send_request(proc, {
            "jsonrpc": "2.0",
            "id": 5,
            "method": "tools/call",
            "params": {
                "name": "sbin___tcl_tool_add",
                "arguments": {
                    "user": "bob",
                    "package": "math",
                    "name": "multiply",
                    "version": "latest",
                    "description": "Multiply two numbers",
                    "script": "expr $a * $b",
                    "parameters": [
                        {
                            "name": "a",
                            "description": "First number",
                            "required": True,
                            "type_name": "number"
                        },
                        {
                            "name": "b",
                            "description": "Second number",
                            "required": True,
                            "type_name": "number"
                        }
                    ]
                }
            }
        })
        
        # Test 6: List tools with namespace filter
        print("\n6. Testing tcl_tool_list with namespace filter...")
        response = send_request(proc, {
            "jsonrpc": "2.0",
            "id": 6,
            "method": "tools/call",
            "params": {
                "name": "bin___tcl_tool_list",
                "arguments": {
                    "namespace": "alice"
                }
            }
        })
        
        # Test 7: Call custom tool
        print("\n7. Testing custom tool user_alice__utils___reverse_string__v1_0...")
        response = send_request(proc, {
            "jsonrpc": "2.0",
            "id": 7,
            "method": "tools/call",
            "params": {
                "name": "user_alice__utils___reverse_string__v1_0",
                "arguments": {
                    "text": "Hello World"
                }
            }
        })
        
        # Test 8: List all tools to see full namespace structure
        print("\n8. Listing all tools...")
        response = send_request(proc, {
            "jsonrpc": "2.0",
            "id": 8,
            "method": "tools/call",
            "params": {
                "name": "bin___tcl_tool_list",
                "arguments": {}
            }
        })
        
        # Test 9: Remove custom tool
        print("\n9. Testing sbin___tcl_tool_remove...")
        response = send_request(proc, {
            "jsonrpc": "2.0",
            "id": 9,
            "method": "tools/call",
            "params": {
                "name": "sbin___tcl_tool_remove",
                "arguments": {
                    "path": "/alice/utils/reverse_string:1.0"
                }
            }
        })
        
        # Test 10: Try to remove system tool (should fail)
        print("\n10. Testing removal of system tool (should fail)...")
        response = send_request(proc, {
            "jsonrpc": "2.0",
            "id": 10,
            "method": "tools/call",
            "params": {
                "name": "sbin___tcl_tool_remove",
                "arguments": {
                    "path": "/bin/tcl_execute"
                }
            }
        })
        
        print("\nAll namespace tests completed!")
        
    finally:
        # Cleanup
        proc.terminate()
        proc.wait()

if __name__ == "__main__":
    main()