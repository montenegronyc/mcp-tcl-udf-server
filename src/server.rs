use anyhow::Result;
use jsonrpc_core::{IoHandler, Params, Value};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tracing::{info, debug};

use crate::tcl_tools::{TclToolBox, TclExecuteRequest, TclToolAddRequest, TclToolRemoveRequest, TclToolListRequest, TclExecToolRequest};
use crate::tcl_executor::TclExecutor;
use crate::namespace::ToolPath;

#[derive(Clone)]
pub struct TclMcpServer {
    tool_box: TclToolBox,
    handler: IoHandler,
}

#[derive(Debug, Serialize, Deserialize)]
struct McpToolInfo {
    name: String,
    description: Option<String>,
    #[serde(rename = "inputSchema")]
    input_schema: Value,
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

impl TclMcpServer {
    pub fn new(privileged: bool) -> Self {
        // Spawn the TCL executor with privilege settings
        let executor = TclExecutor::spawn(privileged);
        let tool_box = TclToolBox::new(executor);
        let mut handler = IoHandler::new();
        
        // Register MCP methods
        handler.add_sync_method("initialize", move |_params: Params| {
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
        });
        
        let tb = tool_box.clone();
        let is_privileged = privileged;
        handler.add_sync_method("tools/list", move |_params: Params| {
            debug!("MCP tools/list called (privileged: {})", is_privileged);
            let tb = tb.clone();
            
            // Don't use async block here since we're in a sync context
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
                if is_privileged {
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
                
            // Get custom tools synchronously - this should be fast
            let custom_tools = match std::thread::spawn(move || {
                let rt = tokio::runtime::Runtime::new().unwrap();
                rt.block_on(tb.get_tool_definitions())
            }).join() {
                Ok(result) => result,
                Err(_) => {
                    return Err(jsonrpc_core::Error::internal_error());
                }
            };
            
            // Add custom tools to the list
            if let Ok(tool_defs) = custom_tools {
                for tool_def in tool_defs {
                    // Build input schema for custom tool
                    let mut properties = serde_json::Map::new();
                    let mut required = Vec::new();
                    
                    for param in &tool_def.parameters {
                        // Validate and normalize JSON Schema type
                        let json_type = match param.type_name.to_lowercase().as_str() {
                            "string" | "str" | "text" => "string",
                            "number" | "float" | "double" | "real" => "number",
                            "integer" | "int" | "long" => "integer", 
                            "boolean" | "bool" => "boolean",
                            "array" | "list" => "array",
                            "object" | "dict" | "map" => "object",
                            "null" | "nil" | "none" => "null",
                            // Default to string for unknown types to maintain compatibility
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
                    
                    // Build the schema object, only including "required" if it's not empty
                    let mut schema_obj = serde_json::Map::new();
                    schema_obj.insert("$schema".to_string(), json!("https://json-schema.org/draft/2020-12/schema"));
                    schema_obj.insert("type".to_string(), json!("object"));
                    schema_obj.insert("properties".to_string(), json!(properties));
                    
                    // Only add "required" array if there are required parameters
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
            }
            
            Ok(json!(McpListToolsResult { tools }))
        });
        
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
                // Check if it's a system tool by MCP name
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
                        // Handle documentation request
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
        
        Self { tool_box, handler }
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
        info!("Starting TCL MCP server on stdio");
        
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