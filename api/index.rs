use serde_json::json;
use vercel_runtime::{run, Body, Error, Request, Response};

async fn handler(req: Request) -> Result<Response<Body>, Error> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    
    let path = req.uri().path();
    let method = req.method();
    
    // Simple response for now to test if the function works
    let response_data = json!({
        "status": "ok",
        "service": "tcl-mcp-server",
        "version": "1.0.0",
        "path": path,
        "method": method.to_string(),
        "message": "Vercel function is working"
    });
    
    Response::builder()
        .status(200)
        .header("content-type", "application/json")
        .header("access-control-allow-origin", "*")
        .header("access-control-allow-methods", "GET, POST, OPTIONS")
        .header("access-control-allow-headers", "Authorization, Content-Type, X-API-Key")
        .body(Body::from(response_data.to_string()))
        .map_err(|e| Error::from(format!("Failed to build response: {}", e)))
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    run(handler).await
}