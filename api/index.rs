use serde_json::json;
use vercel_runtime::{run, Body, Error, Request, RequestExt, Response};

use tcl_mcp_server::http_server::HttpMcpServer;
use tcl_mcp_server::tcl_runtime::RuntimeConfig;

async fn handler(req: Request) -> Result<Response<Body>, Error> {
    let path = req.uri().path();
    let method = req.method();
    
    // Determine runtime configuration
    let env_runtime = std::env::var("TCL_MCP_RUNTIME").ok();
    let runtime_config = RuntimeConfig::from_args_and_env(
        None,
        env_runtime.as_deref(),
    ).unwrap_or_else(|_| RuntimeConfig::default());
    
    // For Vercel, use restricted mode by default for security
    let privileged = std::env::var("TCL_MCP_PRIVILEGED")
        .map(|v| v.to_lowercase() == "true")
        .unwrap_or(false);
    
    // Create server instance
    let server = HttpMcpServer::new_with_runtime(privileged, runtime_config)
        .map_err(|e| Error::from(format!("Failed to create server: {}", e)))?;
    
    // Initialize persistence (load existing tools)
    if let Err(e) = server.initialize_persistence().await {
        tracing::warn!("Failed to initialize persistence: {}", e);
        // Continue without persistence rather than failing
    }
    
    // Create router
    let app = server.router();
    
    // Convert Vercel request to Axum request
    let (parts, body) = req.into_parts();
    let axum_request = axum::extract::Request::from_parts(parts, axum::body::Body::from(body));
    
    // Handle the request using tower::Service
    match tower::ServiceExt::oneshot(app, axum_request).await {
        Ok(response) => {
            let (parts, body) = response.into_parts();
            let body_bytes = axum::body::to_bytes(body, usize::MAX).await
                .map_err(|e| Error::from(format!("Failed to read response body: {}", e)))?;
            
            let mut builder = Response::builder()
                .status(parts.status);
            
            // Copy headers
            for (key, value) in parts.headers {
                if let Some(key) = key {
                    builder = builder.header(key, value);
                }
            }
            
            builder
                .body(Body::from(body_bytes))
                .map_err(|e| Error::from(format!("Failed to build response: {}", e)))
        }
        Err(e) => {
            tracing::error!("Request handling error: {}", e);
            Response::builder()
                .status(500)
                .header("content-type", "application/json")
                .body(Body::from(json!({
                    "error": "Internal server error",
                    "message": e.to_string()
                }).to_string()))
                .map_err(|e| Error::from(format!("Failed to build error response: {}", e)))
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    
    run(handler).await
}