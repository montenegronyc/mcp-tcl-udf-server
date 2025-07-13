#!/bin/bash
# Test runner for bin__exec_tool functionality

set -e

echo "=== Running bin__exec_tool Tests ==="
echo

# Colors for output
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Test results
PASSED=0
FAILED=0

# Function to run a test
run_test() {
    local test_name=$1
    local test_command=$2
    
    echo -n "Running $test_name... "
    
    if eval "$test_command" > /dev/null 2>&1; then
        echo -e "${GREEN}PASSED${NC}"
        ((PASSED++))
    else
        echo -e "${RED}FAILED${NC}"
        ((FAILED++))
        # Show error output
        eval "$test_command" 2>&1 | sed 's/^/  /'
    fi
}

# Check if we're in the right directory
if [ ! -f "Cargo.toml" ]; then
    echo -e "${RED}Error: Must run from tcl-mcp root directory${NC}"
    exit 1
fi

# 1. Run Rust unit tests
echo -e "${YELLOW}Running Rust unit tests...${NC}"
run_test "Unit tests" "cargo test --test test_bin_exec_tool"
echo

# 2. Run Rust integration tests
echo -e "${YELLOW}Running Rust integration tests...${NC}"
run_test "Integration tests" "cargo test --test test_bin_exec_tool_integration"
echo

# 3. Run TCL tests
echo -e "${YELLOW}Running TCL tests...${NC}"
if command -v tclsh &> /dev/null; then
    run_test "TCL script tests" "tclsh tests/test_bin_exec_tool.tcl"
else
    echo -e "${YELLOW}Warning: tclsh not found, skipping TCL tests${NC}"
fi
echo

# 4. Run Python MCP tests (if Python is available)
echo -e "${YELLOW}Running Python MCP tests...${NC}"
if command -v python3 &> /dev/null; then
    # Check if pytest is installed
    if python3 -m pytest --version &> /dev/null; then
        run_test "Python MCP tests" "python3 -m pytest tests/test_bin_exec_tool_mcp.py -v"
    else
        echo -e "${YELLOW}Warning: pytest not installed, trying direct execution${NC}"
        run_test "Python MCP tests" "python3 tests/test_bin_exec_tool_mcp.py"
    fi
else
    echo -e "${YELLOW}Warning: Python3 not found, skipping MCP tests${NC}"
fi
echo

# 5. Run example validation
echo -e "${YELLOW}Validating examples...${NC}"
if [ -f "tests/examples/bin_exec_tool_examples.md" ]; then
    echo -e "${GREEN}Example documentation found${NC}"
    ((PASSED++))
else
    echo -e "${RED}Example documentation missing${NC}"
    ((FAILED++))
fi
echo

# Summary
echo "=== Test Summary ==="
echo -e "Passed: ${GREEN}$PASSED${NC}"
echo -e "Failed: ${RED}$FAILED${NC}"

if [ $FAILED -eq 0 ]; then
    echo -e "\n${GREEN}All tests passed!${NC}"
    exit 0
else
    echo -e "\n${RED}Some tests failed!${NC}"
    exit 1
fi