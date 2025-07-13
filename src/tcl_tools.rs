use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, oneshot};
use tracing::info;

use crate::tcl_executor::TclCommand;

use crate::namespace::ToolPath;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub path: ToolPath,
    pub description: String,
    pub script: String,
    pub parameters: Vec<ParameterDefinition>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParameterDefinition {
    pub name: String,
    pub description: String,
    pub required: bool,
    pub type_name: String,
}

#[derive(Clone)]
pub struct TclToolBox {
    executor: mpsc::Sender<TclCommand>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TclExecuteRequest {
    /// TCL script to execute
    pub script: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TclToolAddRequest {
    /// User namespace (required for user tools)
    pub user: String,
    /// Package name (required for user tools)
    pub package: String,
    /// Name of the new tool
    pub name: String,
    /// Version of the tool (defaults to "latest")
    #[serde(default = "default_version")]
    pub version: String,
    /// Description of what the tool does
    pub description: String,
    /// TCL script that implements the tool
    pub script: String,
    /// Parameters that the tool accepts
    #[serde(default)]
    pub parameters: Vec<ParameterDefinition>,
}

fn default_version() -> String {
    "latest".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TclToolRemoveRequest {
    /// Full tool path (e.g., "/alice/utils/reverse_string:1.0")
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TclToolListRequest {
    /// Filter tools by namespace (optional)
    #[serde(default)]
    pub namespace: Option<String>,
    /// Filter tools by name pattern (optional)
    #[serde(default)]
    pub filter: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TclExecToolRequest {
    /// Tool path to execute (e.g., "/bin/list_dir")
    pub tool_path: String,
    /// Parameters to pass to the tool
    #[serde(default)]
    pub params: serde_json::Value,
}

impl TclToolBox {
    pub fn new(executor: mpsc::Sender<TclCommand>) -> Self {
        Self { executor }
    }

    pub async fn tcl_execute(&self, request: TclExecuteRequest) -> Result<String> {
        info!("Executing TCL script: {}", request.script);
        
        let (tx, rx) = oneshot::channel();
        self.executor.send(TclCommand::Execute {
            script: request.script,
            response: tx,
        }).await.map_err(|_| anyhow!("Failed to send command to executor"))?;
        
        rx.await.map_err(|_| anyhow!("Failed to receive response from executor"))?
    }
    
    pub async fn tcl_tool_add(&self, request: TclToolAddRequest) -> Result<String> {
        let path = ToolPath::user(&request.user, &request.package, &request.name, &request.version);
        info!("Adding new TCL tool: {}", path);
        
        let (tx, rx) = oneshot::channel();
        self.executor.send(TclCommand::AddTool {
            path,
            description: request.description,
            script: request.script,
            parameters: request.parameters,
            response: tx,
        }).await.map_err(|_| anyhow!("Failed to send command to executor"))?;
        
        rx.await.map_err(|_| anyhow!("Failed to receive response from executor"))?
    }
    
    pub async fn tcl_tool_remove(&self, request: TclToolRemoveRequest) -> Result<String> {
        let path = ToolPath::parse(&request.path)?;
        info!("Removing TCL tool: {}", path);
        
        let (tx, rx) = oneshot::channel();
        self.executor.send(TclCommand::RemoveTool {
            path,
            response: tx,
        }).await.map_err(|_| anyhow!("Failed to send command to executor"))?;
        
        rx.await.map_err(|_| anyhow!("Failed to receive response from executor"))?
    }
    
    pub async fn tcl_tool_list(&self, request: TclToolListRequest) -> Result<String> {
        info!("Listing TCL tools with namespace: {:?}, filter: {:?}", request.namespace, request.filter);
        
        let (tx, rx) = oneshot::channel();
        self.executor.send(TclCommand::ListTools {
            namespace: request.namespace,
            filter: request.filter,
            response: tx,
        }).await.map_err(|_| anyhow!("Failed to send command to executor"))?;
        
        let tools = rx.await.map_err(|_| anyhow!("Failed to receive response from executor"))??;
        
        // Format as JSON with full paths
        Ok(serde_json::to_string_pretty(&tools)?)
    }
    
    pub async fn execute_custom_tool(&self, mcp_name: &str, params: serde_json::Value) -> Result<String> {
        let path = ToolPath::from_mcp_name(mcp_name)?;
        
        let (tx, rx) = oneshot::channel();
        self.executor.send(TclCommand::ExecuteCustomTool {
            path,
            params,
            response: tx,
        }).await.map_err(|_| anyhow!("Failed to send command to executor"))?;
        
        rx.await.map_err(|_| anyhow!("Failed to receive response from executor"))?
    }
    
    pub async fn get_tool_definitions(&self) -> Result<Vec<ToolDefinition>> {
        let (tx, rx) = oneshot::channel();
        self.executor.send(TclCommand::GetToolDefinitions {
            response: tx,
        }).await.map_err(|_| anyhow!("Failed to send command to executor"))?;
        
        Ok(rx.await.map_err(|_| anyhow!("Failed to receive response from executor"))?)
    }
    
    pub async fn initialize_persistence(&self) -> Result<String> {
        let (tx, rx) = oneshot::channel();
        self.executor.send(TclCommand::InitializePersistence {
            response: tx,
        }).await.map_err(|_| anyhow!("Failed to send command to executor"))?;
        
        rx.await.map_err(|_| anyhow!("Failed to receive response from executor"))?
    }
    
    pub async fn exec_tool(&self, request: TclExecToolRequest) -> Result<String> {
        info!("Executing tool: {} with params: {:?}", request.tool_path, request.params);
        
        let (tx, rx) = oneshot::channel();
        self.executor.send(TclCommand::ExecTool {
            tool_path: request.tool_path,
            params: request.params,
            response: tx,
        }).await.map_err(|_| anyhow!("Failed to send command to executor"))?;
        
        rx.await.map_err(|_| anyhow!("Failed to receive response from executor"))?
    }
    
    pub async fn discover_tools(&self) -> Result<String> {
        info!("Discovering tools from filesystem");
        
        let (tx, rx) = oneshot::channel();
        self.executor.send(TclCommand::DiscoverTools {
            response: tx,
        }).await.map_err(|_| anyhow!("Failed to send command to executor"))?;
        
        rx.await.map_err(|_| anyhow!("Failed to receive response from executor"))?
    }
}