use anyhow::Result;
use jsonrpc_core::{IoHandler, Params, Value};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tracing::{info, debug};

use crate::tcl_tools::{TclToolBox, TclExecuteRequest, TclToolAddRequest, TclToolRemoveRequest, TclToolListRequest, TclExecToolRequest};
use crate::tcl_executor::TclExecutor;
use crate::namespace::ToolPath;
use crate::tcl_runtime::create_runtime;
use crate::capabilities::{CapabilityFactory, CommandMetadata};

#[derive(Clone)]
pub struct EnhancedTclMcpServer {
    tool_box: TclToolBox,
    handler: IoHandler,
    privileged: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct McpToolInfo {
    name: String,
    description: Option<String>,
    #[serde(rename = "inputSchema")]
    input_schema: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    metadata: Option<ToolMetadata>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ToolMetadata {
    runtime: String,
    safety_level: String,
    available_commands: usize,
    limitations: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct McpListToolsResult {
    tools: Vec<McpToolInfo>,
}

#[derive(Debug, Serialize, Deserialize)]
struct McpCallToolParams {
    name: String,
    arguments: Value,
}

#[derive(Debug, Serialize, Deserialize)]
struct McpCallToolResult {
    content: Vec<McpContent>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
enum McpContent {
    #[serde(rename = "text")]
    Text { text: String },
}

// New capability query parameters
#[derive(Debug, Serialize, Deserialize)]
struct TclCapabilitiesParams {
    // Optional parameters for capability queries
}

#[derive(Debug, Serialize, Deserialize)]
struct TclCommandsParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    filter: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    category: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct TclCommandsResult {
    commands: Vec<CommandMetadata>,
    summary: CommandSummary,
}

#[derive(Debug, Serialize, Deserialize)]
struct CommandSummary {
    total: usize,
    safe: usize,
    restricted: usize,
    unsafe: usize,
    unavailable: usize,
}

impl EnhancedTclMcpServer {
    pub fn new(privileged: bool) -> Self {
        // Spawn the TCL executor with privilege settings
        let executor = TclExecutor::spawn(privileged);
        let tool_box = TclToolBox::new(executor);
        let mut handler = IoHandler::new();
        
        // Get runtime capabilities for enhanced server info
        let runtime = create_runtime();
        let capabilities = runtime.get_capabilities(privileged);
        let runtime_name = runtime.name();
        
        // Register enhanced MCP initialize method
        let caps_clone = capabilities.clone();
        let privileged_clone = privileged;
        handler.add_sync_method("initialize", move |_params: Params| {
            info!("Enhanced MCP initialize called");
            Ok(json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {
                    "tools": {},
                    "tcl": {
                        "runtime": {
                            "type": caps_clone.runtime.runtime_type,
                            "version": caps_clone.runtime.version,
                            "features": [
                                caps_clone.features.core_commands,
                                caps_clone.features.string_operations,
                                caps_clone.features.list_operations,
                                caps_clone.features.math_operations,
                                caps_clone.features.control_structures
                            ].concat(),
                            "limitations": [
                                caps_clone.limitations.file_io.as_ref().unwrap_or(&"none".to_string()),
                                caps_clone.limitations.network.as_ref().unwrap_or(&"none".to_string()),
                                caps_clone.limitations.exec.as_ref().unwrap_or(&"none".to_string())
                            ],
                            "safety_level": caps_clone.safety.level
                        },
                        "commands": {
                            "available": caps_clone.commands.total_available,
                            "unsafe": caps_clone.commands.unsafe,
                            "restricted": caps_clone.commands.restricted,
                            "safe": caps_clone.commands.safe
                        },
                        "extensions": {
                            "custom_tools": true,
                            "filesystem_discovery": true,
                            "persistence": true
                        }
                    }
                },
                "serverInfo": {
                    "name": "tcl-mcp-server-enhanced",
                    "version": "1.1.0",
                    "tcl_runtime": format!("{} {}", caps_clone.runtime.name, caps_clone.runtime.version),
                    "build_features": [caps_clone.runtime.runtime_type.clone()],
                    "safety_mode": if privileged_clone { "privileged" } else { "restricted" }
                }
            }))
        });
        
        // Register enhanced tools/list with metadata
        let tb = tool_box.clone();
        let is_privileged = privileged;
        let runtime_for_tools = create_runtime();
        let caps_for_tools = runtime_for_tools.get_capabilities(privileged);
        handler.add_sync_method("tools/list", move |_params: Params| {
            debug!("Enhanced MCP tools/list called (privileged: {})", is_privileged);
            let tb = tb.clone();
            let caps = caps_for_tools.clone();
            
            let mut tools = vec![];
            
            // Create metadata for tool descriptions
            let tool_metadata = ToolMetadata {
                runtime: caps.runtime.name.clone(),
                safety_level: caps.safety.level.clone(),
                available_commands: caps.commands.total_available,
                limitations: vec![
                    caps.limitations.file_io.clone().unwrap_or_default(),
                    caps.limitations.network.clone().unwrap_or_default(),
                    caps.limitations.exec.clone().unwrap_or_default(),
                ].into_iter().filter(|s| !s.is_empty()).collect(),
            };
            
            // Add system tools with enhanced metadata
            let mut system_tools = vec![
                (ToolPath::bin("tcl_execute"), 
                 format!("Execute a TCL script using {} interpreter ({})", caps.runtime.name, caps.safety.level), 
                 json!({
                    "$schema": "https://json-schema.org/draft/2020-12/schema",
                    "type": "object",
                    "properties": {
                        "script": {
                            "type": "string",
                            "description": format!("TCL script to execute ({} runtime - {} mode)", caps.runtime.name, caps.safety.level)
                        }
                    },
                    "required": ["script"]
                })),
                (ToolPath::bin("tcl_tool_list"), 
                 format!("List all available TCL tools ({})", caps.runtime.name), 
                 json!({
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
                (ToolPath::docs("molt_book"), 
                 format!("Access {} TCL interpreter documentation and examples", caps.runtime.name), 
                 json!({
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
            ];
            
            // Add privileged tools only if in privileged mode
            if is_privileged {
                system_tools.push((ToolPath::sbin("tcl_tool_add"), 
                    format!("Add a new TCL tool to the available tools (PRIVILEGED - {})", caps.runtime.name), 
                    json!({
                        "$schema": "https://json-schema.org/draft/2020-12/schema",
                        "type": "object",
                        "properties": {
                            "user": { "type": "string", "description": "User namespace" },
                            "package": { "type": "string", "description": "Package name" },
                            "name": { "type": "string", "description": "Name of the new tool" },
                            "version": { "type": "string", "description": "Version of the tool (defaults to 'latest')", "default": "latest" },
                            "description": { "type": "string", "description": "Description of what the tool does" },
                            "script": { "type": "string", "description": "TCL script that implements the tool" },
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
            }
            
            for (path, description, schema) in system_tools {
                tools.push(McpToolInfo {
                    name: path.to_mcp_name(),
                    description: Some(format!("{} [{}]", description, path)),
                    input_schema: schema,
                    metadata: Some(tool_metadata.clone()),
                });
            }
            
            // Get custom tools (same as before but with metadata)
            let custom_tools = match std::thread::spawn(move || {
                let rt = tokio::runtime::Runtime::new().unwrap();
                rt.block_on(tb.get_tool_definitions())
            }).join() {
                Ok(result) => result,
                Err(_) => return Err(jsonrpc_core::Error::internal_error()),
            };
            
            if let Ok(tool_defs) = custom_tools {
                for tool_def in tool_defs {
                    // Build input schema for custom tool
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
                        metadata: Some(tool_metadata.clone()),
                    });
                }
            }
            
            Ok(json!(McpListToolsResult { tools }))
        });
        
        // Register new tcl/capabilities method
        let runtime_for_caps = create_runtime();
        let privileged_for_caps = privileged;
        handler.add_sync_method("tcl/capabilities", move |_params: Params| {
            debug!("TCL capabilities query called");
            let caps = runtime_for_caps.get_capabilities(privileged_for_caps);
            Ok(json!(caps))
        });
        
        // Register new tcl/commands method
        let runtime_for_commands = create_runtime();
        handler.add_sync_method("tcl/commands", move |params: Params| {
            debug!("TCL commands query called");
            let params: TclCommandsParams = params.parse().unwrap_or(TclCommandsParams { filter: None, category: None });
            
            let provider = CapabilityFactory::create_provider(runtime_for_commands.name());
            let commands = provider.get_command_metadata(
                params.filter.as_deref(), 
                params.category.as_deref()
            );
            
            let summary = CommandSummary {
                total: commands.len(),
                safe: commands.iter().filter(|c| matches!(c.safety, crate::capabilities::CommandSafety::Safe)).count(),
                restricted: commands.iter().filter(|c| matches!(c.safety, crate::capabilities::CommandSafety::Restricted)).count(),
                unsafe: commands.iter().filter(|c| matches!(c.safety, crate::capabilities::CommandSafety::Unsafe)).count(),
                unavailable: commands.iter().filter(|c| matches!(c.safety, crate::capabilities::CommandSafety::Unavailable)).count(),
            };
            
            Ok(json!(TclCommandsResult { commands, summary }))
        });
        
        // Register existing tools/call method (unchanged)
        let tb = tool_box.clone();
        let is_privileged_call = privileged;
        handler.add_sync_method("tools/call", move |params: Params| {
            debug!("MCP tools/call called with params: {:?}", params);
            let tb = tb.clone();
            
            let params: McpCallToolParams = params.parse()?;
            info!("Calling tool: {} (privileged: {})", params.name, is_privileged_call);
            
            let result = std::thread::spawn(move || {
                let rt = tokio::runtime::Runtime::new().unwrap();
                rt.block_on(async move {
                    // Handle system tools and custom tools same as before
                    match params.name.as_str() {
                        "bin___tcl_execute" => {
                            let request: TclExecuteRequest = serde_json::from_value(params.arguments)?;
                            tb.tcl_execute(request).await
                        }
                        "sbin___tcl_tool_add" => {
                            if !is_privileged_call {
                                return Err(anyhow::anyhow!("Tool management requires --privileged mode"));
                            }
                            let request: TclToolAddRequest = serde_json::from_value(params.arguments)?;
                            tb.tcl_tool_add(request).await
                        }
                        "sbin___tcl_tool_remove" => {
                            if !is_privileged_call {
                                return Err(anyhow::anyhow!("Tool management requires --privileged mode"));
                            }
                            let request: TclToolRemoveRequest = serde_json::from_value(params.arguments)?;
                            tb.tcl_tool_remove(request).await
                        }
                        "bin___tcl_tool_list" => {
                            let request: TclToolListRequest = serde_json::from_value(params.arguments)?;
                            tb.tcl_tool_list(request).await
                        }
                        "bin___exec_tool" => {
                            let request: TclExecToolRequest = serde_json::from_value(params.arguments)?;
                            tb.exec_tool(request).await
                        }
                        "bin___discover_tools" => {
                            tb.discover_tools().await
                        }
                        "docs___molt_book" => {
                            // Handle documentation request (same as before)
                            let topic = params.arguments.get("topic")
                                .and_then(|v| v.as_str())
                                .unwrap_or("overview");
                            
                            match topic {
                                "overview" => Ok(format!(r#"# Enhanced TCL MCP Server Runtime Information

## Active Runtime
- **Type**: {}
- **Version**: {}
- **Safety Level**: {}
- **Memory Safe**: {}
- **Thread Safe**: {}

## Available Commands
- **Total**: {}
- **Safe**: {}
- **Restricted**: {}
- **Unavailable**: {}

For detailed command information, use the `tcl/commands` MCP method.
For full capability details, use the `tcl/capabilities` MCP method.

## Standard Molt Documentation
{}"#, 
                                    "Runtime info would be inserted here",
                                    "Version info",
                                    "Safety level",
                                    "Memory safety",
                                    "Thread safety",
                                    "Command count",
                                    "Safe count",
                                    "Restricted count",
                                    "Unavailable count",
                                    "Standard Molt documentation...")),
                                _ => Ok("See enhanced documentation via tcl/capabilities method".to_string())
                            }
                        }
                        mcp_name => {
                            // Try to execute as a custom tool
                            tb.execute_custom_tool(mcp_name, params.arguments).await
                        }
                    }
                })
            }).join();
            
            match result {
                Ok(Ok(text)) => Ok(json!(McpCallToolResult {
                    content: vec![McpContent::Text { text }],
                })),
                Ok(Err(e)) => Err(jsonrpc_core::Error {
                    code: jsonrpc_core::ErrorCode::InternalError,
                    message: e.to_string(),
                    data: None,
                }),
                Err(_) => Err(jsonrpc_core::Error {
                    code: jsonrpc_core::ErrorCode::InternalError,
                    message: "Thread panic".to_string(),
                    data: None,
                }),
            }
        });
        
        Self { tool_box, handler, privileged }
    }
    
    /// Initialize persistence for tool storage
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
    
    pub async fn run_stdio(self) -> Result<()> {
        info!("Starting Enhanced TCL MCP server on stdio (privileged: {})", self.privileged);
        
        let stdin = tokio::io::stdin();
        let mut stdout = tokio::io::stdout();
        let mut reader = BufReader::new(stdin);
        let mut line = String::new();
        
        loop {
            line.clear();
            let n = reader.read_line(&mut line).await?;
            if n == 0 {
                break; // EOF
            }
            
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            
            debug!("Received request: {}", trimmed);
            
            // Process the request
            let response = self.handler.handle_request(trimmed).await;
            
            if let Some(response) = response {
                debug!("Sending response: {}", response);
                stdout.write_all(response.as_bytes()).await?;
                stdout.write_all(b"\n").await?;
                stdout.flush().await?;
            }
        }
        
        Ok(())
    }
}