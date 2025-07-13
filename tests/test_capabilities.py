#!/usr/bin/env python3
"""
Test suite for TCL MCP server capability reporting functionality.

Tests the enhanced MCP protocol extensions for runtime capability communication.
"""

import json
import subprocess
import unittest
import time
import sys
import os
from typing import Dict, Any, Optional

class TclMcpCapabilityTest(unittest.TestCase):
    """Test cases for TCL MCP capability reporting."""
    
    def setUp(self):
        """Set up test environment."""
        self.server_path = "./target/debug/tcl-mcp-server"
        self.process = None
        self.request_id = 1
        
        # Check if server binary exists
        if not os.path.exists(self.server_path):
            self.skipTest("TCL MCP server binary not found")
    
    def tearDown(self):
        """Clean up test environment."""
        self.stop_server()
    
    def start_server(self, privileged: bool = False):
        """Start the MCP server for testing."""
        cmd = [self.server_path]
        if privileged:
            cmd.append("--privileged")
        
        self.process = subprocess.Popen(
            cmd,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            bufsize=0
        )
        
        # Wait for server to start
        time.sleep(0.1)
    
    def stop_server(self):
        """Stop the MCP server."""
        if self.process:
            self.process.terminate()
            self.process.wait()
            self.process = None
    
    def send_request(self, method: str, params: Optional[Dict[str, Any]] = None) -> Dict[str, Any]:
        """Send JSON-RPC request and get response."""
        if not self.process:
            raise RuntimeError("Server not started")
        
        request = {
            "jsonrpc": "2.0",
            "id": self.request_id,
            "method": method
        }
        
        if params is not None:
            request["params"] = params
        
        self.request_id += 1
        
        # Send request
        request_json = json.dumps(request) + "\n"
        self.process.stdin.write(request_json)
        self.process.stdin.flush()
        
        # Read response
        response_line = self.process.stdout.readline()
        if not response_line:
            raise RuntimeError("No response from server")
        
        return json.loads(response_line.strip())
    
    def test_enhanced_initialize_response(self):
        """Test that initialize response includes TCL capability information."""
        self.start_server()
        
        response = self.send_request("initialize", {
            "protocolVersion": "2024-11-05",
            "capabilities": {"tools": {}},
            "clientInfo": {"name": "test-client", "version": "1.0.0"}
        })
        
        self.assertIn("result", response)
        result = response["result"]
        
        # Check basic MCP structure
        self.assertIn("protocolVersion", result)
        self.assertIn("capabilities", result)
        self.assertIn("serverInfo", result)
        
        # Check enhanced capabilities structure
        capabilities = result["capabilities"]
        self.assertIn("tcl", capabilities)
        
        tcl_caps = capabilities["tcl"]
        self.assertIn("runtime", tcl_caps)
        self.assertIn("commands", tcl_caps)
        self.assertIn("extensions", tcl_caps)
        
        # Check runtime information
        runtime = tcl_caps["runtime"]
        self.assertIn("type", runtime)
        self.assertIn("version", runtime)
        self.assertIn("features", runtime)
        self.assertIn("limitations", runtime)
        self.assertIn("safety_level", runtime)
        
        # Check server info enhancements
        server_info = result["serverInfo"]
        self.assertIn("tcl_runtime", server_info)
        self.assertIn("build_features", server_info)
        self.assertIn("safety_mode", server_info)
    
    def test_tcl_capabilities_method(self):
        """Test the new tcl/capabilities MCP method."""
        self.start_server()
        self.send_request("initialize")  # Initialize first
        
        response = self.send_request("tcl/capabilities")
        
        self.assertIn("result", response)
        caps = response["result"]
        
        # Check structure
        required_keys = ["runtime", "features", "limitations", "safety", "commands"]
        for key in required_keys:
            self.assertIn(key, caps)
        
        # Check runtime info
        runtime = caps["runtime"]
        self.assertIn("type", runtime)
        self.assertIn("version", runtime)
        self.assertIn("name", runtime)
        self.assertIn("implementation", runtime)
        self.assertIn("thread_safe", runtime)
        self.assertIn("memory_safe", runtime)
        
        # Check features
        features = caps["features"]
        feature_categories = [
            "core_commands", "string_operations", "list_operations",
            "math_operations", "control_structures", "procedures", "variables"
        ]
        for category in feature_categories:
            self.assertIn(category, features)
            self.assertIsInstance(features[category], list)
        
        # Check command info
        commands = caps["commands"]
        self.assertIn("total_available", commands)
        self.assertIn("safe", commands)
        self.assertIn("restricted", commands)
        self.assertIn("unsafe", commands)
        self.assertIn("unavailable", commands)
        
        # Verify command counts make sense
        total = commands["total_available"]
        safe_count = len(commands["safe"])
        restricted_count = len(commands["restricted"])
        unsafe_count = len(commands["unsafe"])
        unavailable_count = len(commands["unavailable"])
        
        self.assertGreater(total, 0)
        self.assertGreaterEqual(safe_count, 0)
        self.assertGreaterEqual(restricted_count, 0)
        self.assertGreaterEqual(unsafe_count, 0)
        self.assertGreaterEqual(unavailable_count, 0)
    
    def test_tcl_commands_method(self):
        """Test the new tcl/commands MCP method."""
        self.start_server()
        self.send_request("initialize")
        
        # Test basic command query
        response = self.send_request("tcl/commands")
        
        self.assertIn("result", response)
        result = response["result"]
        
        self.assertIn("commands", result)
        self.assertIn("summary", result)
        
        commands = result["commands"]
        summary = result["summary"]
        
        # Check summary structure
        summary_keys = ["total", "safe", "restricted", "unsafe", "unavailable"]
        for key in summary_keys:
            self.assertIn(key, summary)
            self.assertIsInstance(summary[key], int)
        
        # Check command structure
        if commands:
            cmd = commands[0]
            cmd_keys = ["name", "safety", "category", "description", "available"]
            for key in cmd_keys:
                self.assertIn(key, cmd)
    
    def test_tcl_commands_filtering(self):
        """Test tcl/commands method with filtering parameters."""
        self.start_server()
        self.send_request("initialize")
        
        # Test safety filtering
        for safety_filter in ["safe", "restricted", "unsafe", "unavailable"]:
            response = self.send_request("tcl/commands", {"filter": safety_filter})
            self.assertIn("result", response)
            
            commands = response["result"]["commands"]
            summary = response["result"]["summary"]
            
            # Verify all returned commands match the filter
            for cmd in commands:
                self.assertEqual(cmd["safety"].lower(), safety_filter)
        
        # Test category filtering
        for category in ["string", "list", "system"]:
            response = self.send_request("tcl/commands", {"category": category})
            self.assertIn("result", response)
            
            commands = response["result"]["commands"]
            
            # Verify all returned commands match the category
            for cmd in commands:
                self.assertEqual(cmd["category"], category)
    
    def test_enhanced_tools_list_metadata(self):
        """Test that tools/list includes enhanced metadata."""
        self.start_server()
        self.send_request("initialize")
        
        response = self.send_request("tools/list")
        
        self.assertIn("result", response)
        tools = response["result"]["tools"]
        
        self.assertGreater(len(tools), 0)
        
        # Check that tools have metadata
        for tool in tools:
            self.assertIn("name", tool)
            self.assertIn("description", tool)
            self.assertIn("inputSchema", tool)
            
            # Check for enhanced metadata
            if "metadata" in tool:
                metadata = tool["metadata"]
                self.assertIn("runtime", metadata)
                self.assertIn("safety_level", metadata)
                self.assertIn("available_commands", metadata)
                self.assertIn("limitations", metadata)
    
    def test_privileged_vs_restricted_capabilities(self):
        """Test that capabilities differ between privileged and restricted modes."""
        # Test restricted mode
        self.start_server(privileged=False)
        self.send_request("initialize")
        
        restricted_caps = self.send_request("tcl/capabilities")
        self.assertIn("result", restricted_caps)
        
        restricted_safety = restricted_caps["result"]["safety"]["level"]
        restricted_tools = self.send_request("tools/list")["result"]["tools"]
        
        self.stop_server()
        
        # Test privileged mode
        self.start_server(privileged=True)
        self.send_request("initialize")
        
        privileged_caps = self.send_request("tcl/capabilities")
        self.assertIn("result", privileged_caps)
        
        privileged_safety = privileged_caps["result"]["safety"]["level"]
        privileged_tools = self.send_request("tools/list")["result"]["tools"]
        
        # Verify differences
        self.assertEqual(restricted_safety, "restricted")
        self.assertEqual(privileged_safety, "privileged")
        
        # Privileged mode should have more tools (sbin tools)
        restricted_tool_names = {tool["name"] for tool in restricted_tools}
        privileged_tool_names = {tool["name"] for tool in privileged_tools}
        
        self.assertGreater(len(privileged_tool_names), len(restricted_tool_names))
        self.assertTrue(restricted_tool_names.issubset(privileged_tool_names))
    
    def test_runtime_specific_capabilities(self):
        """Test that capabilities are specific to the active runtime."""
        self.start_server()
        self.send_request("initialize")
        
        response = self.send_request("tcl/capabilities")
        caps = response["result"]
        
        runtime_type = caps["runtime"]["type"]
        
        # Test Molt-specific capabilities
        if runtime_type == "molt":
            self.assertEqual(caps["runtime"]["implementation"], "rust")
            self.assertTrue(caps["runtime"]["memory_safe"])
            self.assertFalse(caps["runtime"]["thread_safe"])
            
            # Molt should have file I/O limitations
            limitations = caps["limitations"]
            self.assertIsNotNone(limitations.get("file_io"))
            self.assertIsNotNone(limitations.get("exec"))
            
            # Check for specific unavailable commands
            unavailable = caps["commands"]["unavailable"]
            self.assertIn("exec", unavailable)
            self.assertIn("open", unavailable)
        
        # Test would be similar for official TCL runtime
        elif runtime_type == "tcl":
            self.assertEqual(caps["runtime"]["implementation"], "c")
            self.assertFalse(caps["runtime"]["memory_safe"])
            self.assertTrue(caps["runtime"]["thread_safe"])
    
    def test_capability_consistency(self):
        """Test that capability information is consistent across methods."""
        self.start_server()
        init_response = self.send_request("initialize")
        caps_response = self.send_request("tcl/capabilities")
        
        # Get runtime info from both responses
        init_tcl = init_response["result"]["capabilities"]["tcl"]["runtime"]
        caps_runtime = caps_response["result"]["runtime"]
        
        # Verify consistency
        self.assertEqual(init_tcl["type"], caps_runtime["type"])
        self.assertEqual(init_tcl["safety_level"], caps_runtime["type"])
        
        # Get server info
        server_info = init_response["result"]["serverInfo"]
        
        # Verify server info matches capabilities
        expected_runtime = f"{caps_runtime['name']} {caps_runtime['version']}"
        self.assertEqual(server_info["tcl_runtime"], expected_runtime)

class TclCapabilityIntegrationTest(unittest.TestCase):
    """Integration tests for capability-aware TCL execution."""
    
    def setUp(self):
        """Set up integration test environment."""
        self.server_path = "./target/debug/tcl-mcp-server"
        if not os.path.exists(self.server_path):
            self.skipTest("TCL MCP server binary not found")
    
    def test_capability_aware_code_generation(self):
        """Test generating code based on capability information."""
        # This would be a more complex test that demonstrates how
        # an LLM client would use capability information to generate
        # appropriate TCL code
        pass

def run_capability_tests():
    """Run all capability tests."""
    loader = unittest.TestLoader()
    suite = unittest.TestSuite()
    
    # Add test classes
    suite.addTests(loader.loadTestsFromTestCase(TclMcpCapabilityTest))
    suite.addTests(loader.loadTestsFromTestCase(TclCapabilityIntegrationTest))
    
    # Run tests
    runner = unittest.TextTestRunner(verbosity=2)
    result = runner.run(suite)
    
    return result.wasSuccessful()

if __name__ == "__main__":
    success = run_capability_tests()
    sys.exit(0 if success else 1)