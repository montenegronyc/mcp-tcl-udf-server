# Security Setup Guide

This guide explains how to secure your TCL MCP HTTP server with API key authentication.

## Overview

The server now includes API key authentication to prevent unauthorized access. When enabled, all endpoints (except health checks) require a valid API key.

## Quick Setup

### 1. Generate an API Key

**Method 1: Using the endpoint (one-time setup)**
```bash
# For local development
curl -X POST http://localhost:3000/auth/generate-key

# For deployed server (before setting up auth)
curl -X POST https://your-app.vercel.app/auth/generate-key
```

**Method 2: Generate manually**
```bash
# Generate a random 64-character hex key
openssl rand -hex 32
```

### 2. Set Environment Variables

**For Local Development:**
```bash
export TCL_MCP_API_KEY="your-generated-api-key-here"
export TCL_MCP_REQUIRE_AUTH="true"
```

**For Vercel Deployment:**
```bash
# Set the secret in Vercel
vercel env add TCL_MCP_API_KEY production
# Paste your API key when prompted

# Or use the dashboard:
# Go to your Vercel project > Settings > Environment Variables
# Add: TCL_MCP_API_KEY = your-api-key-here
```

### 3. Use the API Key

Include the API key in your requests using one of these methods:

**Method 1: Authorization Header**
```bash
curl -H "Authorization: Bearer your-api-key" \
  https://your-app.vercel.app/tools/list
```

**Method 2: X-API-Key Header**
```bash
curl -H "X-API-Key: your-api-key" \
  https://your-app.vercel.app/tools/list
```

## Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `TCL_MCP_API_KEY` | Your secret API key | `""` (empty, auth disabled) |
| `TCL_MCP_REQUIRE_AUTH` | Enable/disable auth | `true` if API key is set |

## Security Features

### 1. **API Key Authentication**
- 64-character hex keys (256-bit security)
- Constant-time comparison to prevent timing attacks
- Supports both `Authorization: Bearer` and `X-API-Key` headers

### 2. **Protected Endpoints**
- All MCP endpoints require authentication
- Health check endpoints (`/`, `/health`) are always accessible
- `/auth/generate-key` is unprotected for initial setup

### 3. **Configurable Security**
- Auth can be disabled by not setting `TCL_MCP_API_KEY`
- Fine-grained control via environment variables

## Usage Examples

### JavaScript/Node.js
```javascript
const apiKey = process.env.TCL_MCP_API_KEY;

const response = await fetch('https://your-app.vercel.app/tools/call', {
  method: 'POST',
  headers: {
    'Content-Type': 'application/json',
    'Authorization': `Bearer ${apiKey}`
  },
  body: JSON.stringify({
    name: 'bin___tcl_execute',
    arguments: {
      script: 'expr {2 + 2}'
    }
  })
});
```

### Python
```python
import os
import requests

api_key = os.environ['TCL_MCP_API_KEY']

response = requests.post(
    'https://your-app.vercel.app/tools/call',
    headers={
        'Authorization': f'Bearer {api_key}',
        'Content-Type': 'application/json'
    },
    json={
        'name': 'bin___tcl_execute',
        'arguments': {
            'script': 'set x 5; expr {$x * 2}'
        }
    }
)
```

### cURL
```bash
# Set your API key
export API_KEY="your-api-key-here"

# Make authenticated requests
curl -H "Authorization: Bearer $API_KEY" \
  -H "Content-Type: application/json" \
  -X POST https://your-app.vercel.app/tools/call \
  -d '{
    "name": "bin___tcl_execute",
    "arguments": {
      "script": "puts \"Hello, Secure World!\""
    }
  }'
```

## Deployment Steps

### Initial Deployment (Without Auth)
1. Deploy to Vercel: `vercel`
2. Generate API key: `curl -X POST https://your-app.vercel.app/auth/generate-key`
3. Save the returned `api_key` securely

### Secure the Deployment
1. Add API key to Vercel:
   ```bash
   vercel env add TCL_MCP_API_KEY production
   ```
2. Redeploy: `vercel --prod`
3. Test authentication:
   ```bash
   # Should fail without key
   curl https://your-app.vercel.app/tools/list
   
   # Should succeed with key
   curl -H "Authorization: Bearer YOUR_KEY" \
     https://your-app.vercel.app/tools/list
   ```

## Error Responses

### Missing API Key
```json
{
  "error": "Authentication required",
  "message": "API key required. Provide via 'Authorization: Bearer <key>' or 'X-API-Key: <key>' header"
}
```

### Invalid API Key
```json
{
  "error": "Invalid API key",
  "message": "The provided API key is invalid or expired"
}
```

## Best Practices

### 1. **Key Management**
- Generate long, random keys (64+ characters)
- Store keys securely (environment variables, not code)
- Use different keys for different environments
- Rotate keys periodically

### 2. **Network Security**
- Always use HTTPS in production
- Consider IP whitelisting for additional security
- Monitor access logs for suspicious activity

### 3. **Application Security**
- Use restricted mode unless you need privileged features
- Validate all input data
- Monitor resource usage

## Troubleshooting

### Authentication Not Working
1. Check if `TCL_MCP_API_KEY` is set
2. Verify the key matches exactly (no extra spaces)
3. Check the header format: `Authorization: Bearer <key>`
4. Ensure HTTPS is used in production

### Generate New API Key
If you lose your API key:
1. Temporarily disable auth: `vercel env rm TCL_MCP_API_KEY`
2. Redeploy: `vercel --prod`
3. Generate new key: `curl -X POST https://your-app.vercel.app/auth/generate-key`
4. Set new key: `vercel env add TCL_MCP_API_KEY production`
5. Redeploy: `vercel --prod`

## Security Considerations

### Threat Model
- **Protects against**: Unauthorized API access, script injection by strangers
- **Does not protect against**: Compromised keys, application-level vulnerabilities
- **Recommendation**: Use additional layers (WAF, rate limiting, monitoring)

### Production Checklist
- [ ] API key is set and secure
- [ ] HTTPS is enabled
- [ ] Restricted mode is enabled (`TCL_MCP_PRIVILEGED=false`)
- [ ] Access logs are monitored
- [ ] Rate limiting is configured (if needed)
- [ ] Backup authentication method is available

## Advanced Security

For additional security, consider:

1. **IP Whitelisting**: Restrict access to specific IP ranges
2. **Rate Limiting**: Prevent abuse with request limits
3. **Web Application Firewall (WAF)**: Filter malicious requests
4. **Monitoring**: Set up alerts for suspicious activity
5. **Key Rotation**: Regular API key updates

The current implementation provides a solid foundation for securing your TCL MCP server while maintaining ease of use.