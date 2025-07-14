# HTTP/Vercel Deployment Guide

This guide explains how to deploy the TCL MCP Server as an HTTP API on Vercel.

## What Was Modified

The codebase has been extended to support HTTP deployment while maintaining the original stdio-based MCP functionality:

1. **Added HTTP Server Implementation** (`src/http_server.rs`):
   - HTTP REST API endpoints that mirror the MCP protocol
   - CORS support for web applications
   - JSON-based request/response handling

2. **Added HTTP Binary** (`src/main_http.rs`):
   - Standalone HTTP server for local development
   - Configurable host and port
   - Same TCL runtime and privilege configurations

3. **Added Vercel Support** (`api/index.rs`):
   - Serverless function entry point
   - Vercel Runtime integration
   - Request/response conversion between Vercel and Axum

## Available Endpoints

### Health Check
- `GET /` - Basic health check
- `GET /health` - Detailed health status

### MCP Protocol
- `POST /mcp` - Generic MCP request handler (JSON-RPC format)
- `POST /initialize` - Initialize MCP server
- `GET /tools/list` - List available tools
- `POST /tools/call` - Execute a tool

## Local Development

### HTTP Server
```bash
# Build the HTTP server
cargo build --release --bin tcl-mcp-server-http

# Run in restricted mode (default)
./target/release/tcl-mcp-server-http

# Run in privileged mode
./target/release/tcl-mcp-server-http --privileged

# Custom port and runtime
./target/release/tcl-mcp-server-http --port 8080 --runtime molt --privileged
```

### Test the HTTP API
```bash
# Health check
curl http://localhost:3000/health

# List tools
curl http://localhost:3000/tools/list

# Execute TCL script
curl -X POST http://localhost:3000/tools/call \
  -H "Content-Type: application/json" \
  -d '{
    "name": "bin___tcl_execute",
    "arguments": {
      "script": "expr {2 + 2}"
    }
  }'

# MCP protocol format
curl -X POST http://localhost:3000/mcp \
  -H "Content-Type: application/json" \
  -d '{
    "method": "tools/call",
    "params": {
      "name": "bin___tcl_execute",
      "arguments": {
        "script": "set x 5; expr {$x * 2}"
      }
    },
    "id": 1
  }'
```

## Vercel Deployment

### Prerequisites
- Vercel CLI installed: `npm i -g vercel`
- Vercel account

### Deploy
```bash
# Initial deployment (without auth)
vercel

# Generate API key after first deployment
curl -X POST https://your-app.vercel.app/auth/generate-key

# Add API key to Vercel
vercel env add TCL_MCP_API_KEY production

# Redeploy with authentication
vercel --prod
```

### Configuration
The `vercel.json` configuration sets:
- Runtime: `vercel-rust@4.0.0`
- Environment: Molt runtime, restricted mode, authentication enabled
- Routes: All traffic goes to `/api/index`

### Environment Variables
- `TCL_MCP_RUNTIME`: `molt` or `tcl` (default: `molt`)
- `TCL_MCP_PRIVILEGED`: `true` or `false` (default: `false`)
- `TCL_MCP_API_KEY`: Your secret API key for authentication
- `TCL_MCP_REQUIRE_AUTH`: `true` or `false` (default: `true` if API key is set)
- `RUST_LOG`: Log level (default: `info`)

### Security Setup
See [SECURITY_SETUP.md](SECURITY_SETUP.md) for complete authentication setup instructions.

## API Usage Examples

### JavaScript/Node.js
```javascript
const apiKey = process.env.TCL_MCP_API_KEY;

const response = await fetch('https://your-vercel-app.vercel.app/tools/call', {
  method: 'POST',
  headers: {
    'Content-Type': 'application/json',
    'Authorization': `Bearer ${apiKey}`
  },
  body: JSON.stringify({
    name: 'bin___tcl_execute',
    arguments: {
      script: 'set greeting "Hello"; puts "$greeting, World!"'
    }
  })
});

const result = await response.json();
console.log(result.content[0].text);
```

### Python
```python
import os
import requests

api_key = os.environ['TCL_MCP_API_KEY']

response = requests.post(
    'https://your-vercel-app.vercel.app/tools/call',
    headers={
        'Authorization': f'Bearer {api_key}',
        'Content-Type': 'application/json'
    },
    json={
        'name': 'bin___tcl_execute',
        'arguments': {
            'script': 'set numbers [list 1 2 3 4 5]; expr {[join $numbers " + "]}'
        }
    }
)

result = response.json()
print(result['content'][0]['text'])
```

### cURL
```bash
# Set your API key
export API_KEY="your-api-key-here"

# Execute TCL script
curl -X POST https://your-vercel-app.vercel.app/tools/call \
  -H "Authorization: Bearer $API_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "bin___tcl_execute",
    "arguments": {
      "script": "proc factorial {n} { if {$n <= 1} { return 1 } else { return [expr {$n * [factorial [expr {$n - 1}]]}] } }; factorial 5"
    }
  }'
```

## Security Considerations

### Default Security Settings
- **Restricted Mode**: Tool management disabled by default
- **Molt Runtime**: Safe TCL subset, no file I/O or system commands
- **CORS**: Configured for web access but can be restricted

### Enabling Privileged Mode
Only enable privileged mode if you need:
- Dynamic tool creation (`sbin___tcl_tool_add`)
- Tool removal (`sbin___tcl_tool_remove`)
- Full TCL runtime features

Set `TCL_MCP_PRIVILEGED=true` in Vercel environment variables.

### Production Recommendations
- Use restricted mode for public deployments
- Implement authentication/authorization if needed
- Monitor usage and implement rate limiting
- Consider running in a container for additional isolation

## Troubleshooting

### Common Issues
1. **Build Errors**: Ensure all dependencies are correctly specified in `Cargo.toml`
2. **Runtime Errors**: Check logs with `RUST_LOG=debug`
3. **CORS Issues**: Verify CORS configuration in `http_server.rs`
4. **Tool Not Found**: Ensure tool names use MCP format (e.g., `bin___tcl_execute`)

### Debugging
```bash
# Local debugging with verbose logs
RUST_LOG=debug ./target/release/tcl-mcp-server-http

# Check Vercel logs
vercel logs
```

## Differences from stdio MCP

| Feature | stdio MCP | HTTP API |
|---------|-----------|----------|
| **Protocol** | JSON-RPC over stdio | HTTP REST + JSON-RPC |
| **Deployment** | Desktop/CLI apps | Web services/serverless |
| **Authentication** | Process-based | HTTP-based (can add auth) |
| **Scaling** | Single instance | Serverless auto-scaling |
| **CORS** | N/A | Built-in support |
| **Monitoring** | Process monitoring | HTTP monitoring |

The HTTP API maintains full compatibility with MCP tool definitions and execution semantics.