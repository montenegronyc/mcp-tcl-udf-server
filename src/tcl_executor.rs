use anyhow::{Result, anyhow};
use molt::Interp;
use tokio::sync::{mpsc, oneshot};
use std::collections::HashMap;
use std::thread;

use crate::tcl_tools::{ToolDefinition, ParameterDefinition};
use crate::namespace::{ToolPath, Namespace};
use crate::persistence::FilePersistence;
use crate::tool_discovery::{ToolDiscovery, DiscoveredTool};

pub enum TclCommand {
    Execute {
        script: String,
        response: oneshot::Sender<Result<String>>,
    },
    AddTool {
        path: ToolPath,
        description: String,
        script: String,
        parameters: Vec<ParameterDefinition>,
        response: oneshot::Sender<Result<String>>,
    },
    RemoveTool {
        path: ToolPath,
        response: oneshot::Sender<Result<String>>,
    },
    ListTools {
        namespace: Option<String>,
        filter: Option<String>,
        response: oneshot::Sender<Result<Vec<String>>>,
    },
    ExecuteCustomTool {
        path: ToolPath,
        params: serde_json::Value,
        response: oneshot::Sender<Result<String>>,
    },
    GetToolDefinitions {
        response: oneshot::Sender<Vec<ToolDefinition>>,
    },
    InitializePersistence {
        response: oneshot::Sender<Result<String>>,
    },
    ExecTool {
        tool_path: String,
        params: serde_json::Value,
        response: oneshot::Sender<Result<String>>,
    },
    DiscoverTools {
        response: oneshot::Sender<Result<String>>,
    },
}

pub struct TclExecutor {
    interp: Interp,
    custom_tools: HashMap<ToolPath, ToolDefinition>,
    discovered_tools: HashMap<ToolPath, DiscoveredTool>,
    tool_discovery: ToolDiscovery,
    persistence: Option<FilePersistence>,
}

impl TclExecutor {
    pub fn new(privileged: bool) -> Self {
        let interp = Interp::new();
        
        // In non-privileged mode, we could disable certain commands here
        // For now, we'll just store the flag and use it during execution
        if !privileged {
            // TODO: Consider filtering dangerous commands like 'exec', 'file', etc.
            // For now, we rely on Molt's default safety features
        }
        
        Self {
            interp,
            custom_tools: HashMap::new(),
            discovered_tools: HashMap::new(),
            tool_discovery: ToolDiscovery::new(),
            persistence: None,
        }
    }
    
    pub fn spawn(privileged: bool) -> mpsc::Sender<TclCommand> {
        let (tx, mut rx) = mpsc::channel::<TclCommand>(100);
        
        // Spawn a dedicated thread for the TCL interpreter
        thread::spawn(move || {
            let mut executor = TclExecutor::new(privileged);
            
            // Create a single-threaded runtime for this thread
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("Failed to create Tokio runtime");
                
            runtime.block_on(async move {
                while let Some(cmd) = rx.recv().await {
                    match cmd {
                        TclCommand::Execute { script, response } => {
                            let result = executor.execute_script(&script);
                            let _ = response.send(result);
                        }
                        TclCommand::AddTool { path, description, script, parameters, response } => {
                            let result = executor.add_tool(path, description, script, parameters).await;
                            let _ = response.send(result);
                        }
                        TclCommand::RemoveTool { path, response } => {
                            let result = executor.remove_tool(&path).await;
                            let _ = response.send(result);
                        }
                        TclCommand::ListTools { namespace, filter, response } => {
                            let tools = executor.list_tools(namespace, filter);
                            let _ = response.send(Ok(tools));
                        }
                        TclCommand::ExecuteCustomTool { path, params, response } => {
                            let result = executor.execute_custom_tool(&path, params);
                            let _ = response.send(result);
                        }
                        TclCommand::GetToolDefinitions { response } => {
                            let tools = executor.get_tool_definitions();
                            let _ = response.send(tools);
                        }
                        TclCommand::InitializePersistence { response } => {
                            let result = executor.initialize_persistence().await;
                            let _ = response.send(result);
                        }
                        TclCommand::ExecTool { tool_path, params, response } => {
                            let result = executor.exec_tool(&tool_path, params).await;
                            let _ = response.send(result);
                        }
                        TclCommand::DiscoverTools { response } => {
                            let result = executor.discover_tools().await;
                            let _ = response.send(result);
                        }
                    }
                }
            });
        });
        
        tx
    }
    
    fn execute_script(&mut self, script: &str) -> Result<String> {
        match self.interp.eval(script) {
            Ok(value) => Ok(value.to_string()),
            Err(error) => Err(anyhow!("TCL execution error: {:?}", error)),
        }
    }
    
    async fn add_tool(&mut self, path: ToolPath, description: String, script: String, parameters: Vec<ParameterDefinition>) -> Result<String> {
        // Only allow adding tools to user namespace
        if !matches!(path.namespace, Namespace::User(_)) {
            return Err(anyhow!("Can only add tools to user namespace, not {}", path));
        }
        
        if self.custom_tools.contains_key(&path) {
            return Err(anyhow!("Tool '{}' already exists", path));
        }
        
        // Initialize persistence if not already initialized
        if self.persistence.is_none() {
            match FilePersistence::new().await {
                Ok(persistence) => {
                    // Load existing tools from storage
                    match persistence.list_tools(None).await {
                        Ok(stored_tools) => {
                            for tool in stored_tools {
                                if matches!(tool.path.namespace, Namespace::User(_)) {
                                    self.custom_tools.insert(tool.path.clone(), tool);
                                }
                            }
                            tracing::info!("Initialized persistence and loaded {} existing tools", self.custom_tools.len());
                        }
                        Err(e) => {
                            tracing::warn!("Failed to load existing tools: {}", e);
                        }
                    }
                    self.persistence = Some(persistence);
                }
                Err(e) => {
                    tracing::warn!("Failed to initialize persistence: {}", e);
                }
            }
        }
        
        let tool_def = ToolDefinition {
            path: path.clone(),
            description,
            script,
            parameters,
        };
        
        // Save to persistence if available
        let persisted = if let Some(ref mut persistence) = self.persistence {
            match persistence.save_tool(&tool_def).await {
                Ok(_) => true,
                Err(e) => {
                    tracing::warn!("Failed to persist tool: {}", e);
                    false
                }
            }
        } else {
            false
        };
        
        // Add to in-memory cache
        self.custom_tools.insert(path.clone(), tool_def);
        
        if persisted {
            Ok(format!("Tool '{}' added successfully and persisted", path))
        } else {
            Ok(format!("Tool '{}' added to memory (persistence unavailable)", path))
        }
    }
    
    async fn remove_tool(&mut self, path: &ToolPath) -> Result<String> {
        // Cannot remove system tools
        if path.is_system() {
            return Err(anyhow!("Cannot remove system tool '{}'", path));
        }
        
        // Remove from in-memory cache first
        let removed_from_memory = self.custom_tools.remove(path).is_some();
        
        // Remove from persistent storage
        let removed_from_storage = self.remove_tool_from_storage(path).await?;
        
        if removed_from_memory || removed_from_storage {
            Ok(format!("Tool '{}' removed successfully", path))
        } else {
            Err(anyhow!("Tool '{}' not found", path))
        }
    }
    
    fn list_tools(&self, namespace: Option<String>, filter: Option<String>) -> Vec<String> {
        let mut tools = Vec::new();
        
        // Add system tools
        let system_tools = vec![
            ToolPath::bin("tcl_execute"),
            ToolPath::sbin("tcl_tool_add"),
            ToolPath::sbin("tcl_tool_remove"),
            ToolPath::bin("tcl_tool_list"),
            ToolPath::bin("exec_tool"),
            ToolPath::bin("discover_tools"),
            ToolPath::docs("molt_book"),
        ];
        
        for tool in system_tools {
            if let Some(ref ns) = namespace {
                let matches = match (&tool.namespace, ns.as_str()) {
                    (Namespace::Bin, "bin") => true,
                    (Namespace::Sbin, "sbin") => true,
                    (Namespace::User(user_ns), filter_ns) if user_ns == filter_ns => true,
                    _ => false,
                };
                if !matches {
                    continue;
                }
            }
            
            let path_str = tool.to_string();
            if filter.as_ref().map(|f| path_str.contains(f)).unwrap_or(true) {
                tools.push(path_str);
            }
        }
        
        // Add custom tools
        for path in self.custom_tools.keys() {
            if let Some(ref ns) = namespace {
                let matches = match (&path.namespace, ns.as_str()) {
                    (Namespace::User(user_ns), filter_ns) if user_ns == filter_ns => true,
                    _ => false,
                };
                if !matches {
                    continue;
                }
            }
            
            let path_str = path.to_string();
            if filter.as_ref().map(|f| path_str.contains(f)).unwrap_or(true) {
                tools.push(path_str);
            }
        }
        
        // Add discovered tools
        for path in self.discovered_tools.keys() {
            if let Some(ref ns) = namespace {
                let matches = match (&path.namespace, ns.as_str()) {
                    (Namespace::Bin, "bin") => true,
                    (Namespace::Sbin, "sbin") => true,
                    (Namespace::Docs, "docs") => true,
                    (Namespace::User(user_ns), filter_ns) if user_ns == filter_ns => true,
                    _ => false,
                };
                if !matches {
                    continue;
                }
            }
            
            let path_str = path.to_string();
            if filter.as_ref().map(|f| path_str.contains(f)).unwrap_or(true) {
                tools.push(path_str);
            }
        }
        
        tools.sort();
        tools
    }
    
    fn execute_custom_tool(&mut self, path: &ToolPath, params: serde_json::Value) -> Result<String> {
        let tool = self.custom_tools.get(path)
            .ok_or_else(|| anyhow!("Tool '{}' not found", path))?
            .clone();
        
        let mut script = String::new();
        
        // Set parameters as TCL variables
        if let Some(params_obj) = params.as_object() {
            for param_def in &tool.parameters {
                if let Some(value) = params_obj.get(&param_def.name) {
                    let tcl_value = match value {
                        serde_json::Value::String(s) => format!("\"{}\"", s.replace("\"", "\\\"")),
                        _ => value.to_string(),
                    };
                    script.push_str(&format!("set {} {}\n", param_def.name, tcl_value));
                } else if param_def.required {
                    return Err(anyhow!("Missing required parameter: {}", param_def.name));
                }
            }
        }
        
        // Append the tool script
        script.push_str(&tool.script);
        
        self.execute_script(&script)
    }
    
    fn get_tool_definitions(&self) -> Vec<ToolDefinition> {
        let mut tools = Vec::new();
        
        // Add custom tools
        tools.extend(self.custom_tools.values().cloned());
        
        // Convert discovered tools to ToolDefinition format
        for discovered in self.discovered_tools.values() {
            let tool_def = ToolDefinition {
                path: discovered.path.clone(),
                description: discovered.description.clone(),
                script: format!("# Tool loaded from: {}", discovered.file_path.display()),
                parameters: discovered.parameters.clone(),
            };
            tools.push(tool_def);
        }
        
        tools
    }
    
    /// Initialize persistence and load existing tools
    async fn initialize_persistence(&mut self) -> Result<String> {
        if self.persistence.is_some() {
            return Ok("Persistence already initialized".to_string());
        }
        
        let persistence = FilePersistence::new().await?;
        
        // Load existing tools from storage
        let stored_tools = persistence.list_tools(None).await?;
        let loaded_count = stored_tools.len();
        
        // Add stored tools to in-memory cache
        for tool in stored_tools {
            // Only load user tools, system tools are hardcoded
            if matches!(tool.path.namespace, Namespace::User(_)) {
                self.custom_tools.insert(tool.path.clone(), tool);
            }
        }
        
        self.persistence = Some(persistence);
        
        Ok(format!("Persistence initialized. Loaded {} tools from storage.", loaded_count))
    }
    
    
    /// Remove tool from persistent storage
    async fn remove_tool_from_storage(&mut self, path: &ToolPath) -> Result<bool> {
        if let Some(ref mut persistence) = self.persistence {
            return persistence.delete_tool(path).await;
        }
        Ok(false)
    }
    
    /// Execute a tool from the filesystem or custom tools
    async fn exec_tool(&mut self, tool_path: &str, params: serde_json::Value) -> Result<String> {
        // Parse the tool path
        let path = ToolPath::parse(tool_path)?;
        
        // Check custom tools first (added via tcl_tool_add)
        if let Some(custom_tool) = self.custom_tools.get(&path) {
            // Create a script with parameter bindings
            let mut full_script = String::new();
            
            // Set parameters as TCL variables
            if let Some(params_obj) = params.as_object() {
                for param_def in &custom_tool.parameters {
                    if let Some(value) = params_obj.get(&param_def.name) {
                        let tcl_value = match value {
                            serde_json::Value::String(s) => format!("\"{}\"", s.replace("\"", "\\\"")),
                            _ => value.to_string(),
                        };
                        full_script.push_str(&format!("set {} {}\n", param_def.name, tcl_value));
                    } else if param_def.required {
                        return Err(anyhow!("Missing required parameter: {}", param_def.name));
                    }
                }
            }
            
            // Make params available as an array for the script
            full_script.push_str("array set params {}\n");
            if let Some(params_obj) = params.as_object() {
                for (key, value) in params_obj {
                    let tcl_value = match value {
                        serde_json::Value::String(s) => format!("\"{}\"", s.replace("\"", "\\\"")),
                        _ => value.to_string(),
                    };
                    full_script.push_str(&format!("set params({}) {}\n", key, tcl_value));
                }
            }
            
            // Append the tool script
            full_script.push_str(&custom_tool.script);
            
            return self.execute_script(&full_script);
        }
        
        // Check if it's a discovered tool
        if let Some(discovered_tool) = self.discovered_tools.get(&path) {
            // Read and execute the tool file
            let script_content = tokio::fs::read_to_string(&discovered_tool.file_path).await?;
            
            // Create a script with parameter bindings
            let mut full_script = String::new();
            
            // Set parameters as TCL variables
            if let Some(params_obj) = params.as_object() {
                for param_def in &discovered_tool.parameters {
                    if let Some(value) = params_obj.get(&param_def.name) {
                        let tcl_value = match value {
                            serde_json::Value::String(s) => format!("\"{}\"", s.replace("\"", "\\\"")),
                            _ => value.to_string(),
                        };
                        full_script.push_str(&format!("set {} {}\n", param_def.name, tcl_value));
                    } else if param_def.required {
                        return Err(anyhow!("Missing required parameter: {}", param_def.name));
                    }
                }
            }
            
            // Append the tool script
            full_script.push_str(&script_content);
            
            return self.execute_script(&full_script);
        }
        
        // Check if it's a custom tool
        if let Some(_custom_tool) = self.custom_tools.get(&path) {
            return self.execute_custom_tool(&path, params);
        }
        
        // Check if it's a built-in system tool
        match tool_path {
            "/bin/tcl_execute" => {
                if let Some(script) = params.get("script").and_then(|s| s.as_str()) {
                    self.execute_script(script)
                } else {
                    Err(anyhow!("Missing required parameter: script"))
                }
            }
            "/bin/tcl_tool_list" => {
                let namespace = params.get("namespace").and_then(|s| s.as_str()).map(String::from);
                let filter = params.get("filter").and_then(|s| s.as_str()).map(String::from);
                let tools = self.list_tools(namespace, filter);
                Ok(tools.join("\n"))
            }
            _ => Err(anyhow!("Tool '{}' not found", tool_path))
        }
    }
    
    /// Discover and index tools from the filesystem
    async fn discover_tools(&mut self) -> Result<String> {
        // Discover tools from the filesystem
        let discovered = self.tool_discovery.discover_tools().await?;
        let count = discovered.len();
        
        // Add discovered tools to our cache
        for tool in discovered {
            self.discovered_tools.insert(tool.path.clone(), tool);
        }
        
        // Register discovered tools as available for execution
        // Note: We don't add them as TCL commands directly since that would require
        // complex callback handling. Instead, they can be executed via exec_tool.
        
        Ok(format!("Discovered {} tools from filesystem", count))
    }
}