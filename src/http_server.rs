use anyhow::Result;
use axum::{
    extract::{Json, State},
    http::StatusCode,
    middleware,
    response::{IntoResponse, Response},
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tower_http::cors::CorsLayer;
use tracing::{info, debug, error};

use crate::auth::{AuthConfig, auth_middleware};
use crate::tcl_tools::{TclToolBox, TclExecuteRequest, TclToolAddRequest, TclToolRemoveRequest, TclToolListRequest, TclExecToolRequest};
use crate::tcl_executor::TclExecutor;
use crate::namespace::ToolPath;
use crate::tcl_runtime::RuntimeConfig;

#[derive(Clone)]
pub struct HttpMcpServer {
    tool_box: TclToolBox,
    privileged: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct McpRequest {
    pub method: String,
    pub params: Option<Value>,
    pub id: Option<Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct McpResponse {
    pub result: Option<Value>,
    pub error: Option<McpError>,
    pub id: Option<Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct McpError {
    pub code: i32,
    pub message: String,
    pub data: Option<Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct McpToolInfo {
    pub name: String,
    pub description: Option<String>,
    #[serde(rename = "inputSchema")]
    pub input_schema: Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct McpListToolsResult {
    pub tools: Vec<McpToolInfo>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct McpCallToolParams {
    pub name: String,
    pub arguments: Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct McpCallToolResult {
    pub content: Vec<McpContent>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum McpContent {
    #[serde(rename = "text")]
    Text { text: String },
}

impl HttpMcpServer {
    pub fn new(privileged: bool) -> Self {
        let executor = TclExecutor::spawn(privileged);
        let tool_box = TclToolBox::new(executor);
        
        Self { tool_box, privileged }
    }
    
    pub fn new_with_runtime(privileged: bool, runtime_config: RuntimeConfig) -> Result<Self, String> {
        let executor = TclExecutor::spawn_with_runtime(privileged, runtime_config)?;
        let tool_box = TclToolBox::new(executor);
        
        Ok(Self { tool_box, privileged })
    }
    
    pub async fn initialize_persistence(&self) -> Result<()> {
        match self.tool_box.initialize_persistence().await {
            Ok(message) => {
                info!("{}", message);
                Ok(())
            }
            Err(e) => {
                tracing::warn!("Failed to initialize persistence: {}", e);
                Err(e)
            }
        }
    }
    
    pub fn router(self) -> Router {
        let auth_config = AuthConfig::new();
        
        Router::new()
            .route("/", get(health_check))
            .route("/health", get(health_check))
            .route("/mcp", post(handle_mcp_request))
            .route("/initialize", post(handle_initialize))
            .route("/tools/list", get(handle_tools_list))
            .route("/tools/call", post(handle_tools_call))
            .route("/auth/generate-key", post(generate_api_key_endpoint))
            .layer(middleware::from_fn_with_state(auth_config.clone(), auth_middleware))
            .layer(CorsLayer::permissive())
            .with_state(self)
            .with_state(auth_config)
    }
    
    async fn handle_initialize(&self) -> Result<Value, McpError> {
        info!("MCP initialize called");
        Ok(json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {
                "tools": {}
            },
            "serverInfo": {
                "name": "tcl-mcp-server",
                "version": "1.0.0"
            }
        }))
    }
    
    async fn handle_tools_list(&self) -> Result<McpListToolsResult, McpError> {
        debug!("MCP tools/list called (privileged: {})", self.privileged);
        
        let mut tools = vec![];
        
        // Add system tools with MCP-compatible names
        let mut system_tools = vec![
            (ToolPath::bin("tcl_execute"), "Execute a TCL script and return the result", json!({
                "$schema": "https://json-schema.org/draft/2020-12/schema",
                "type": "object",
                "properties": {
                    "script": {
                        "type": "string",
                        "description": "TCL script to execute"
                    }
                },
                "required": ["script"]
            })),
            (ToolPath::bin("tcl_tool_list"), "List all available TCL tools", json!({
                "$schema": "https://json-schema.org/draft/2020-12/schema",
                "type": "object",
                "properties": {
                    "namespace": {
                        "type": "string",
                        "description": "Filter tools by namespace (optional)"
                    },
                    "filter": {
                        "type": "string",
                        "description": "Filter tools by name pattern (optional)"
                    }
                }
            })),
            (ToolPath::docs("molt_book"), "Access Molt TCL interpreter documentation and examples", json!({
                "$schema": "https://json-schema.org/draft/2020-12/schema",
                "type": "object",
                "properties": {
                    "topic": {
                        "type": "string",
                        "description": "Documentation topic: 'overview', 'commands', 'examples', 'links', or 'basic_syntax'",
                        "enum": ["overview", "commands", "examples", "links", "basic_syntax"]
                    }
                },
                "required": ["topic"]
            })),
            (ToolPath::bin("exec_tool"), "Execute a tool by its path with parameters", json!({
                "$schema": "https://json-schema.org/draft/2020-12/schema",
                "type": "object",
                "properties": {
                    "tool_path": {
                        "type": "string",
                        "description": "Full path to the tool (e.g., '/bin/list_dir')"
                    },
                    "params": {
                        "type": "object",
                        "description": "Parameters to pass to the tool",
                        "default": {}
                    }
                },
                "required": ["tool_path"]
            })),
            (ToolPath::bin("discover_tools"), "Discover and index tools from the filesystem", json!({
                "$schema": "https://json-schema.org/draft/2020-12/schema",
                "type": "object",
                "properties": {}
            })),
        ];
        
        // Add privileged tools only if in privileged mode
        if self.privileged {
            system_tools.push((ToolPath::sbin("tcl_tool_add"), "Add a new TCL tool to the available tools (PRIVILEGED)", json!({
                "$schema": "https://json-schema.org/draft/2020-12/schema",
                "type": "object",
                "properties": {
                    "user": {
                        "type": "string",
                        "description": "User namespace"
                    },
                    "package": {
                        "type": "string",
                        "description": "Package name"
                    },
                    "name": {
                        "type": "string",
                        "description": "Name of the new tool"
                    },
                    "version": {
                        "type": "string",
                        "description": "Version of the tool (defaults to 'latest')",
                        "default": "latest"
                    },
                    "description": {
                        "type": "string",
                        "description": "Description of what the tool does"
                    },
                    "script": {
                        "type": "string",
                        "description": "TCL script that implements the tool"
                    },
                    "parameters": {
                        "type": "array",
                        "description": "Parameters that the tool accepts",
                        "items": {
                            "type": "object",
                            "properties": {
                                "name": { "type": "string" },
                                "description": { "type": "string" },
                                "required": { "type": "boolean" },
                                "type_name": { "type": "string" }
                            },
                            "required": ["name", "description", "required", "type_name"]
                        }
                    }
                },
                "required": ["user", "package", "name", "description", "script"]
            })));
            system_tools.push((ToolPath::sbin("tcl_tool_remove"), "Remove a TCL tool from the available tools (PRIVILEGED)", json!({
                "$schema": "https://json-schema.org/draft/2020-12/schema",
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Full tool path (e.g., '/alice/utils/reverse_string:1.0')"
                    }
                },
                "required": ["path"]
            })));
        }
        
        for (path, description, schema) in system_tools {
            tools.push(McpToolInfo {
                name: path.to_mcp_name(),
                description: Some(format!("{} [{}]", description, path)),
                input_schema: schema,
            });
        }
        
        // Get custom tools
        let custom_tools = self.tool_box.get_tool_definitions().await.map_err(|e| {
            McpError {
                code: -32603,
                message: format!("Failed to get tool definitions: {}", e),
                data: None,
            }
        })?;
        
        // Add custom tools to the list
        for tool_def in custom_tools {
            let mut properties = serde_json::Map::new();
            let mut required = Vec::new();
            
            for param in &tool_def.parameters {
                let json_type = match param.type_name.to_lowercase().as_str() {
                    "string" | "str" | "text" => "string",
                    "number" | "float" | "double" | "real" => "number",
                    "integer" | "int" | "long" => "integer", 
                    "boolean" | "bool" => "boolean",
                    "array" | "list" => "array",
                    "object" | "dict" | "map" => "object",
                    "null" | "nil" | "none" => "null",
                    _ => "string"
                };
                
                properties.insert(
                    param.name.clone(),
                    json!({
                        "type": json_type,
                        "description": param.description,
                    }),
                );
                
                if param.required {
                    required.push(param.name.clone());
                }
            }
            
            let mut schema_obj = serde_json::Map::new();
            schema_obj.insert("$schema".to_string(), json!("https://json-schema.org/draft/2020-12/schema"));
            schema_obj.insert("type".to_string(), json!("object"));
            schema_obj.insert("properties".to_string(), json!(properties));
            
            if !required.is_empty() {
                schema_obj.insert("required".to_string(), json!(required));
            }
            
            let input_schema = serde_json::Value::Object(schema_obj);
            
            tools.push(McpToolInfo {
                name: tool_def.path.to_mcp_name(),
                description: Some(format!("{} [{}]", tool_def.description, tool_def.path)),
                input_schema,
            });
        }
        
        Ok(McpListToolsResult { tools })
    }
    
    async fn handle_tools_call(&self, params: McpCallToolParams) -> Result<McpCallToolResult, McpError> {
        info!("Calling tool: {} (privileged: {})", params.name, self.privileged);
        
        let result = match params.name.as_str() {
            "bin___tcl_execute" => {
                let request: TclExecuteRequest = serde_json::from_value(params.arguments)
                    .map_err(|e| McpError {
                        code: -32602,
                        message: format!("Invalid parameters: {}", e),
                        data: None,
                    })?;
                self.tool_box.tcl_execute(request).await
            }
            "sbin___tcl_tool_add" => {
                if !self.privileged {
                    return Err(McpError {
                        code: -32603,
                        message: "Tool management requires --privileged mode".to_string(),
                        data: None,
                    });
                }
                let request: TclToolAddRequest = serde_json::from_value(params.arguments)
                    .map_err(|e| McpError {
                        code: -32602,
                        message: format!("Invalid parameters: {}", e),
                        data: None,
                    })?;
                self.tool_box.tcl_tool_add(request).await
            }
            "sbin___tcl_tool_remove" => {
                if !self.privileged {
                    return Err(McpError {
                        code: -32603,
                        message: "Tool management requires --privileged mode".to_string(),
                        data: None,
                    });
                }
                let request: TclToolRemoveRequest = serde_json::from_value(params.arguments)
                    .map_err(|e| McpError {
                        code: -32602,
                        message: format!("Invalid parameters: {}", e),
                        data: None,
                    })?;
                self.tool_box.tcl_tool_remove(request).await
            }
            "bin___tcl_tool_list" => {
                let request: TclToolListRequest = serde_json::from_value(params.arguments)
                    .map_err(|e| McpError {
                        code: -32602,
                        message: format!("Invalid parameters: {}", e),
                        data: None,
                    })?;
                self.tool_box.tcl_tool_list(request).await
            }
            "bin___exec_tool" => {
                let request: TclExecToolRequest = serde_json::from_value(params.arguments)
                    .map_err(|e| McpError {
                        code: -32602,
                        message: format!("Invalid parameters: {}", e),
                        data: None,
                    })?;
                self.tool_box.exec_tool(request).await
            }
            "bin___discover_tools" => {
                self.tool_box.discover_tools().await
            }
            "docs___molt_book" => {
                let topic = params.arguments.get("topic")
                    .and_then(|v| v.as_str())
                    .unwrap_or("overview");
                
                match topic {
                    "overview" => Ok(format!(r#"# Molt TCL Interpreter Overview

## What is Molt?
Molt is a TCL (Tool Command Language) interpreter implemented in Rust. It provides a memory-safe, 
embeddable scripting language with familiar TCL syntax.

## Key Features
- Memory-safe implementation in Rust
- Compatible with core TCL commands
- Embeddable in Rust applications
- Thread-safe design
- Standard TCL control structures and data types

## Documentation Links
- Molt Book: https://wduquette.github.io/molt/
- GitHub Repository: https://github.com/wduquette/molt
- Source Documentation: https://github.com/wduquette/molt/tree/master/molt-book/src

Use 'basic_syntax', 'commands', 'examples', or 'links' for more specific information."#)),
                    "basic_syntax" => Ok(format!(r#"# TCL Basic Syntax

## Variables
```tcl
set name "Alice"
set age 30
puts "Hello, $name! You are $age years old."
```

## Lists
```tcl
set fruits [list apple banana cherry]
set first [lindex $fruits 0]  ;# apple
set length [llength $fruits]  ;# 3
```

## Control Structures
```tcl
# If statement
if {{$age >= 18}} {{
    puts "Adult"
}} else {{
    puts "Minor"
}}

# For loop
for {{set i 0}} {{$i < 5}} {{incr i}} {{
    puts "Count: $i"
}}

# Foreach loop
foreach fruit $fruits {{
    puts "Fruit: $fruit"
}}
```

## Procedures
```tcl
proc greet {{name}} {{
    return "Hello, $name!"
}}

set message [greet "World"]
puts $message
```"#)),
                    "commands" => Ok(format!(r#"# Common TCL Commands in Molt

## String Operations
- `string length $str` - Get string length
- `string index $str $idx` - Get character at index
- `string range $str $start $end` - Extract substring
- `string toupper $str` - Convert to uppercase
- `string tolower $str` - Convert to lowercase

## List Operations
- `list $item1 $item2 ...` - Create list
- `lindex $list $index` - Get list element
- `llength $list` - Get list length
- `lappend listVar $item` - Append to list
- `lrange $list $start $end` - Extract sublist

## Math and Logic
- `expr $expression` - Evaluate mathematical expression
- `incr varName ?increment?` - Increment variable
- `+ - * / %` - Arithmetic operators
- `== != < > <= >=` - Comparison operators
- `&& || !` - Logical operators

## Control Flow
- `if {{condition}} {{...}} else {{...}}` - Conditional
- `for {{init}} {{condition}} {{update}} {{...}}` - For loop
- `foreach var $list {{...}}` - Iterate over list
- `while {{condition}} {{...}}` - While loop
- `break` / `continue` - Loop control

## I/O and Variables
- `puts $string` - Print to stdout
- `set varName $value` - Set variable
- `unset varName` - Delete variable
- `global varName` - Access global variable"#)),
                    "examples" => Ok(format!(r#"# TCL Examples

## Example 1: Calculator
```tcl
proc calculate {{op a b}} {{
    switch $op {{
        "+" {{ return [expr {{$a + $b}}] }}
        "-" {{ return [expr {{$a - $b}}] }}
        "*" {{ return [expr {{$a * $b}}] }}
        "/" {{ 
            if {{$b == 0}} {{
                error "Division by zero"
            }}
            return [expr {{$a / $b}}] 
        }}
        default {{ error "Unknown operation: $op" }}
    }}
}}

puts [calculate + 5 3]    ;# 8
puts [calculate * 4 7]    ;# 28
```

## Example 2: List Processing
```tcl
set numbers [list 1 2 3 4 5]
set sum 0

foreach num $numbers {{
    set sum [expr {{$sum + $num}}]
}}

puts "Sum: $sum"  ;# Sum: 15

# Find maximum
set max [lindex $numbers 0]
foreach num $numbers {{
    if {{$num > $max}} {{
        set max $num
    }}
}}
puts "Max: $max"  ;# Max: 5
```

## Example 3: String Processing
```tcl
proc word_count {{text}} {{
    set words [split $text]
    return [llength $words]
}}

proc reverse_string {{str}} {{
    set result ""
    set len [string length $str]
    for {{set i [expr {{$len - 1}}]}} {{$i >= 0}} {{incr i -1}} {{
        append result [string index $str $i]
    }}
    return $result
}}

puts [word_count "Hello world from TCL"]  ;# 4
puts [reverse_string "hello"]              ;# olleh
```"#)),
                    "links" => Ok(format!(r#"# Molt TCL Documentation Links

## Official Documentation
- **Molt Book**: https://wduquette.github.io/molt/
  Complete guide to the Molt TCL interpreter
  
- **GitHub Repository**: https://github.com/wduquette/molt
  Source code, examples, and issue tracking
  
- **Book Source**: https://github.com/wduquette/molt/tree/master/molt-book/src
  Markdown source files for the Molt Book

## Specific Sections
- **Getting Started**: https://wduquette.github.io/molt/user/getting_started.html
- **Language Reference**: https://wduquette.github.io/molt/ref/
- **Embedding Guide**: https://wduquette.github.io/molt/embed/
- **API Documentation**: https://docs.rs/molt/

## TCL Language Resources
- **TCL/Tk Official**: https://www.tcl.tk/
- **TCL Tutorial**: https://www.tcl.tk/man/tcl8.6/tutorial/
- **TCL Commands**: https://www.tcl.tk/man/tcl8.6/TclCmd/

## Example Code
- **Molt Examples**: https://github.com/wduquette/molt/tree/master/examples
- **Test Suite**: https://github.com/wduquette/molt/tree/master/tests

Note: Molt implements a subset of full TCL but covers the core language features.
For Molt-specific capabilities and limitations, refer to the Molt Book."#)),
                    _ => Err(anyhow::anyhow!("Unknown documentation topic: {}. Available topics: overview, basic_syntax, commands, examples, links", topic))
                }
            }
            mcp_name => {
                self.tool_box.execute_custom_tool(mcp_name, params.arguments).await
            }
        };
        
        match result {
            Ok(text) => Ok(McpCallToolResult {
                content: vec![McpContent::Text { text }],
            }),
            Err(e) => Err(McpError {
                code: -32603,
                message: e.to_string(),
                data: None,
            }),
        }
    }
}

// HTTP handlers
async fn health_check() -> impl IntoResponse {
    json!({
        "status": "ok",
        "service": "tcl-mcp-server",
        "version": "1.0.0"
    })
}

async fn handle_mcp_request(
    State(server): State<HttpMcpServer>,
    Json(request): Json<McpRequest>,
) -> Result<Json<McpResponse>, Response> {
    debug!("Received MCP request: {:?}", request);
    
    let result = match request.method.as_str() {
        "initialize" => server.handle_initialize().await,
        "tools/list" => server.handle_tools_list().await.map(|r| serde_json::to_value(r).unwrap()),
        "tools/call" => {
            if let Some(params) = request.params {
                let call_params: McpCallToolParams = serde_json::from_value(params)
                    .map_err(|e| McpError {
                        code: -32602,
                        message: format!("Invalid parameters: {}", e),
                        data: None,
                    })?;
                server.handle_tools_call(call_params).await.map(|r| serde_json::to_value(r).unwrap())
            } else {
                Err(McpError {
                    code: -32602,
                    message: "Missing parameters".to_string(),
                    data: None,
                })
            }
        }
        _ => Err(McpError {
            code: -32601,
            message: format!("Method not found: {}", request.method),
            data: None,
        }),
    };
    
    let response = match result {
        Ok(result) => McpResponse {
            result: Some(result),
            error: None,
            id: request.id,
        },
        Err(error) => McpResponse {
            result: None,
            error: Some(error),
            id: request.id,
        },
    };
    
    Ok(Json(response))
}

async fn handle_initialize(State(server): State<HttpMcpServer>) -> impl IntoResponse {
    match server.handle_initialize().await {
        Ok(result) => (StatusCode::OK, Json(result)),
        Err(error) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({
            "error": error.message
        }))),
    }
}

async fn handle_tools_list(State(server): State<HttpMcpServer>) -> impl IntoResponse {
    match server.handle_tools_list().await {
        Ok(result) => (StatusCode::OK, Json(result)),
        Err(error) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({
            "error": error.message
        }))),
    }
}

async fn handle_tools_call(
    State(server): State<HttpMcpServer>,
    Json(params): Json<McpCallToolParams>,
) -> impl IntoResponse {
    match server.handle_tools_call(params).await {
        Ok(result) => (StatusCode::OK, Json(result)),
        Err(error) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({
            "error": error.message
        }))),
    }
}

// API key generation endpoint (unprotected for initial setup)
async fn generate_api_key_endpoint() -> impl IntoResponse {
    let new_key = crate::auth::generate_api_key();
    let key_hash = crate::auth::hash_api_key(&new_key);
    
    (StatusCode::OK, Json(json!({
        "api_key": new_key,
        "hash": key_hash,
        "instructions": {
            "step_1": "Set TCL_MCP_API_KEY environment variable to the api_key value",
            "step_2": "Restart the server",
            "step_3": "Use the api_key in Authorization header: 'Bearer <api_key>' or 'X-API-Key: <api_key>'"
        },
        "note": "Store the api_key securely. The hash is for verification purposes only."
    })))
}