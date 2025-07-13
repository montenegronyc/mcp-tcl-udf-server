#!/usr/bin/env python3
"""
Integration tests for bin__exec_tool through MCP protocol.
Tests the complete flow from MCP client to TCL execution.
"""

import asyncio
import json
import subprocess
import sys
import time
from typing import Dict, Any, Optional, List
import pytest


class MCPTestClient:
    """Test client for MCP server communication."""
    
    def __init__(self, privileged: bool = True):
        self.privileged = privileged
        self.process: Optional[subprocess.Popen] = None
        self.message_id = 0
        
    async def start(self):
        """Start the MCP server process."""
        cmd = ["cargo", "run", "--"]
        if self.privileged:
            cmd.append("--privileged")
            
        self.process = subprocess.Popen(
            cmd,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            bufsize=0
        )
        
        # Wait for server to be ready
        await asyncio.sleep(1)
        
        # Initialize MCP connection
        response = await self.send_request("initialize", {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {
                "name": "test-client",
                "version": "1.0.0"
            }
        })
        
        assert response["result"]["protocolVersion"] == "2024-11-05"
        
    async def stop(self):
        """Stop the MCP server process."""
        if self.process:
            self.process.terminate()
            self.process.wait(timeout=5)
            
    async def send_request(self, method: str, params: Dict[str, Any]) -> Dict[str, Any]:
        """Send a JSON-RPC request and wait for response."""
        self.message_id += 1
        request = {
            "jsonrpc": "2.0",
            "id": self.message_id,
            "method": method,
            "params": params
        }
        
        # Send request
        request_str = json.dumps(request) + "\n"
        self.process.stdin.write(request_str)
        self.process.stdin.flush()
        
        # Read response
        response_str = self.process.stdout.readline()
        return json.loads(response_str)
        
    async def add_tool(self, user: str, package: str, name: str, 
                      description: str, script: str, 
                      parameters: List[Dict[str, Any]]) -> str:
        """Add a custom tool."""
        response = await self.send_request("tools/call", {
            "name": "mcp__tcl__sbin___tcl_tool_add",
            "arguments": {
                "user": user,
                "package": package,
                "name": name,
                "description": description,
                "script": script,
                "parameters": parameters
            }
        })
        
        if "error" in response:
            raise Exception(f"Failed to add tool: {response['error']}")
            
        return response["result"]["content"][0]["text"]
        
    async def exec_tool(self, tool_path: str, arguments: Dict[str, Any]) -> str:
        """Execute a tool using bin__exec_tool."""
        response = await self.send_request("tools/call", {
            "name": "mcp__tcl__bin___exec_tool",
            "arguments": {
                "tool_path": tool_path,
                "arguments": arguments
            }
        })
        
        if "error" in response:
            raise Exception(f"Tool execution failed: {response['error']}")
            
        return response["result"]["content"][0]["text"]
        
    async def list_tools(self, namespace: Optional[str] = None, 
                        filter_pattern: Optional[str] = None) -> List[str]:
        """List available tools."""
        args = {}
        if namespace:
            args["namespace"] = namespace
        if filter_pattern:
            args["filter"] = filter_pattern
            
        response = await self.send_request("tools/call", {
            "name": "mcp__tcl__bin___tcl_tool_list",
            "arguments": args
        })
        
        if "error" in response:
            raise Exception(f"Failed to list tools: {response['error']}")
            
        # Parse tool list from response
        text = response["result"]["content"][0]["text"]
        return [line.strip() for line in text.split("\n") if line.strip()]


class TestBinExecTool:
    """Test cases for bin__exec_tool functionality."""
    
    @pytest.fixture
    async def client(self):
        """Create and start test client."""
        client = MCPTestClient(privileged=True)
        await client.start()
        yield client
        await client.stop()
        
    @pytest.mark.asyncio
    async def test_basic_tool_execution(self, client):
        """Test basic tool creation and execution."""
        # Add a simple tool
        result = await client.add_tool(
            user="test",
            package="basic",
            name="echo",
            description="Echo tool",
            script='return "Echo: $message"',
            parameters=[{
                "name": "message",
                "description": "Message to echo",
                "required": True,
                "type_name": "string"
            }]
        )
        
        assert "added successfully" in result
        
        # Execute the tool
        result = await client.exec_tool(
            "/test/basic/echo:latest",
            {"message": "Hello MCP"}
        )
        
        assert result == "Echo: Hello MCP"
        
    @pytest.mark.asyncio
    async def test_missing_required_parameter(self, client):
        """Test error handling for missing required parameters."""
        # Add tool with required parameters
        await client.add_tool(
            user="test",
            package="params",
            name="required",
            description="Tool with required params",
            script='return "$param1 and $param2"',
            parameters=[
                {
                    "name": "param1",
                    "description": "First parameter",
                    "required": True,
                    "type_name": "string"
                },
                {
                    "name": "param2",
                    "description": "Second parameter",
                    "required": True,
                    "type_name": "string"
                }
            ]
        )
        
        # Try to execute without all required parameters
        with pytest.raises(Exception) as exc_info:
            await client.exec_tool(
                "/test/params/required:latest",
                {"param1": "value1"}  # Missing param2
            )
            
        assert "Missing required parameter" in str(exc_info.value)
        
    @pytest.mark.asyncio
    async def test_optional_parameters(self, client):
        """Test handling of optional parameters."""
        # Add tool with optional parameter
        await client.add_tool(
            user="test",
            package="params",
            name="optional",
            description="Tool with optional param",
            script='''
                if {[info exists optional]} {
                    return "With optional: $optional"
                } else {
                    return "Without optional"
                }
            ''',
            parameters=[{
                "name": "optional",
                "description": "Optional parameter",
                "required": False,
                "type_name": "string"
            }]
        )
        
        # Execute without optional parameter
        result = await client.exec_tool(
            "/test/params/optional:latest",
            {}
        )
        assert result == "Without optional"
        
        # Execute with optional parameter
        result = await client.exec_tool(
            "/test/params/optional:latest",
            {"optional": "provided"}
        )
        assert result == "With optional: provided"
        
    @pytest.mark.asyncio
    async def test_complex_tcl_operations(self, client):
        """Test complex TCL script execution."""
        # Add factorial calculator
        await client.add_tool(
            user="test",
            package="math",
            name="factorial",
            description="Calculate factorial",
            script='''
                proc factorial {n} {
                    if {$n <= 1} {
                        return 1
                    }
                    return [expr {$n * [factorial [expr {$n - 1}]]}]
                }
                return [factorial $number]
            ''',
            parameters=[{
                "name": "number",
                "description": "Number to calculate factorial of",
                "required": True,
                "type_name": "integer"
            }]
        )
        
        # Test factorial calculations
        test_cases = [(0, 1), (1, 1), (5, 120), (7, 5040)]
        
        for input_val, expected in test_cases:
            result = await client.exec_tool(
                "/test/math/factorial:latest",
                {"number": input_val}
            )
            assert int(result) == expected
            
    @pytest.mark.asyncio
    async def test_tool_not_found(self, client):
        """Test error handling for non-existent tools."""
        with pytest.raises(Exception) as exc_info:
            await client.exec_tool(
                "/test/nonexistent/tool:1.0",
                {}
            )
            
        assert "not found" in str(exc_info.value)
        
    @pytest.mark.asyncio
    async def test_special_characters(self, client):
        """Test handling of special characters in parameters."""
        # Add echo tool
        await client.add_tool(
            user="test",
            package="special",
            name="echo",
            description="Echo with special chars",
            script='return "Got: $input"',
            parameters=[{
                "name": "input",
                "description": "Input string",
                "required": True,
                "type_name": "string"
            }]
        )
        
        # Test various special characters
        test_inputs = [
            'hello "world"',
            'test\\nline',
            '$variable',
            '{braces}',
            '[brackets]',
            'mixed "quotes" and \\backslash'
        ]
        
        for test_input in test_inputs:
            result = await client.exec_tool(
                "/test/special/echo:latest",
                {"input": test_input}
            )
            assert result == f"Got: {test_input}"
            
    @pytest.mark.asyncio
    async def test_concurrent_execution(self, client):
        """Test concurrent tool execution."""
        # Add a counter tool
        await client.add_tool(
            user="test",
            package="concurrent",
            name="task",
            description="Concurrent task",
            script='return "Task $task_id completed at [clock milliseconds]"',
            parameters=[{
                "name": "task_id",
                "description": "Task identifier",
                "required": True,
                "type_name": "string"
            }]
        )
        
        # Execute multiple tools concurrently
        tasks = []
        for i in range(10):
            task = client.exec_tool(
                "/test/concurrent/task:latest",
                {"task_id": f"task_{i}"}
            )
            tasks.append(task)
            
        results = await asyncio.gather(*tasks)
        
        # Verify all tasks completed
        for i, result in enumerate(results):
            assert f"Task task_{i} completed" in result
            
    @pytest.mark.asyncio
    async def test_tool_discovery(self, client):
        """Test tool listing and discovery."""
        # Add multiple tools
        tools = [
            ("util", "string_reverse", "Reverse a string"),
            ("util", "string_upper", "Convert to uppercase"),
            ("math", "add", "Add two numbers"),
            ("math", "multiply", "Multiply two numbers")
        ]
        
        for package, name, description in tools:
            await client.add_tool(
                user="test",
                package=package,
                name=name,
                description=description,
                script=f'return "{name} executed"',
                parameters=[]
            )
            
        # List all tools in test namespace
        all_tools = await client.list_tools(namespace="test")
        assert len([t for t in all_tools if t.startswith("/test/")]) >= 4
        
        # Filter tools by pattern
        string_tools = await client.list_tools(namespace="test", filter_pattern="string")
        string_count = len([t for t in string_tools if "string" in t])
        assert string_count == 2
        
    @pytest.mark.asyncio
    async def test_privilege_mode(self):
        """Test privilege mode restrictions."""
        # Create non-privileged client
        client = MCPTestClient(privileged=False)
        await client.start()
        
        try:
            # List tools - should not include sbin tools
            response = await client.send_request("tools/list", {})
            tools = response["result"]["tools"]
            
            # Verify no sbin tools are exposed
            for tool in tools:
                assert "sbin" not in tool["name"]
                
            # Verify bin__exec_tool is available
            tool_names = [t["name"] for t in tools]
            assert "mcp__tcl__bin___exec_tool" in tool_names
            
        finally:
            await client.stop()


if __name__ == "__main__":
    # Run tests
    pytest.main([__file__, "-v"])