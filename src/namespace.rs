use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Namespace {
    Bin,     // System tools (read-only)
    Sbin,    // System admin tools (privileged)
    Docs,    // Documentation tools (read-only)
    User(String), // User namespace
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ToolPath {
    pub namespace: Namespace,
    pub package: Option<String>,
    pub name: String,
    pub version: String,
}

impl ToolPath {
    /// Create a new system binary tool path
    pub fn bin(name: impl Into<String>) -> Self {
        Self {
            namespace: Namespace::Bin,
            package: None,
            name: name.into(),
            version: "latest".to_string(),
        }
    }
    
    /// Create a new system admin tool path
    pub fn sbin(name: impl Into<String>) -> Self {
        Self {
            namespace: Namespace::Sbin,
            package: None,
            name: name.into(),
            version: "latest".to_string(),
        }
    }
    
    /// Create a new documentation tool path
    pub fn docs(name: impl Into<String>) -> Self {
        Self {
            namespace: Namespace::Docs,
            package: None,
            name: name.into(),
            version: "latest".to_string(),
        }
    }
    
    /// Create a new user tool path
    pub fn user(user: impl Into<String>, package: impl Into<String>, name: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            namespace: Namespace::User(user.into()),
            package: Some(package.into()),
            name: name.into(),
            version: version.into(),
        }
    }
    
    /// Parse a tool path from a string representation
    /// Examples:
    /// - "/bin/tcl_execute"
    /// - "/sbin/tcl_tool_add"
    /// - "/alice/utils/reverse_string:1.0"
    /// - "/bob/math/calculate:latest"
    pub fn parse(path: &str) -> Result<Self> {
        if !path.starts_with('/') {
            return Err(anyhow!("Tool path must start with '/'"));
        }
        
        let parts: Vec<&str> = path[1..].split('/').collect();
        
        match parts.as_slice() {
            ["bin", name] => {
                let (name, _version) = Self::parse_name_version(name)?;
                Ok(Self::bin(name))
            }
            ["sbin", name] => {
                let (name, _version) = Self::parse_name_version(name)?;
                Ok(Self::sbin(name))
            }
            [user, package, name_version] => {
                let (name, version) = Self::parse_name_version(name_version)?;
                Ok(Self::user(user.to_string(), package.to_string(), name, version))
            }
            _ => Err(anyhow!("Invalid tool path format: {}", path)),
        }
    }
    
    /// Parse name:version format
    fn parse_name_version(s: &str) -> Result<(String, String)> {
        if let Some((name, version)) = s.split_once(':') {
            Ok((name.to_string(), version.to_string()))
        } else {
            Ok((s.to_string(), "latest".to_string()))
        }
    }
    
    /// Convert to MCP-compatible tool name (snake_case with prefixes)
    pub fn to_mcp_name(&self) -> String {
        match &self.namespace {
            Namespace::Bin => format!("bin___{}", self.name),
            Namespace::Sbin => format!("sbin___{}", self.name),
            Namespace::Docs => format!("docs___{}", self.name),
            Namespace::User(user) => {
                if let Some(package) = &self.package {
                    if self.version == "latest" {
                        format!("user_{}__{}___{}", user, package, self.name)
                    } else {
                        format!("user_{}__{}___{}__v{}", user, package, self.name, self.version.replace('.', "_"))
                    }
                } else {
                    format!("user_{}___{}", user, self.name)
                }
            }
        }
    }
    
    /// Convert from MCP tool name back to ToolPath
    pub fn from_mcp_name(mcp_name: &str) -> Result<Self> {
        if let Some(name) = mcp_name.strip_prefix("bin___") {
            Ok(Self::bin(name))
        } else if let Some(name) = mcp_name.strip_prefix("sbin___") {
            Ok(Self::sbin(name))
        } else if let Some(name) = mcp_name.strip_prefix("docs___") {
            Ok(Self::docs(name))
        } else if let Some(rest) = mcp_name.strip_prefix("user_") {
            // Parse user_<user>__<package>___<name>__v<version>
            // or user_<user>__<package>___<name> (latest)
            // or user_<user>___<name> (no package)
            
            let parts: Vec<&str> = rest.split("__").collect();
            match parts.as_slice() {
                [user, name] => Ok(Self {
                    namespace: Namespace::User(user.to_string()),
                    package: None,
                    name: name.strip_prefix('_').unwrap_or(name).to_string(),
                    version: "latest".to_string(),
                }),
                [user, package, name] => Ok(Self {
                    namespace: Namespace::User(user.to_string()),
                    package: Some(package.to_string()),
                    name: name.strip_prefix('_').unwrap_or(name).to_string(),
                    version: "latest".to_string(),
                }),
                [user, package, name, version] if version.starts_with('v') => {
                    let version = version[1..].replace('_', ".");
                    Ok(Self {
                        namespace: Namespace::User(user.to_string()),
                        package: Some(package.to_string()),
                        name: name.strip_prefix('_').unwrap_or(name).to_string(),
                        version,
                    })
                }
                _ => Err(anyhow!("Invalid MCP tool name format: {}", mcp_name)),
            }
        } else {
            Err(anyhow!("Unknown tool name format: {}", mcp_name))
        }
    }
    
    /// Check if this is a system tool (bin, sbin, or docs)
    pub fn is_system(&self) -> bool {
        matches!(self.namespace, Namespace::Bin | Namespace::Sbin | Namespace::Docs)
    }
    
}

impl fmt::Display for ToolPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.namespace {
            Namespace::Bin => write!(f, "/bin/{}", self.name),
            Namespace::Sbin => write!(f, "/sbin/{}", self.name),
            Namespace::Docs => write!(f, "/docs/{}", self.name),
            Namespace::User(user) => {
                if let Some(package) = &self.package {
                    if self.version == "latest" {
                        write!(f, "/{}/{}/{}", user, package, self.name)
                    } else {
                        write!(f, "/{}/{}/{}:{}", user, package, self.name, self.version)
                    }
                } else {
                    write!(f, "/{}/{}", user, self.name)
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_parse_paths() {
        assert_eq!(
            ToolPath::parse("/bin/tcl_execute").unwrap(),
            ToolPath::bin("tcl_execute")
        );
        
        assert_eq!(
            ToolPath::parse("/sbin/tcl_tool_add").unwrap(),
            ToolPath::sbin("tcl_tool_add")
        );
        
        assert_eq!(
            ToolPath::parse("/alice/utils/reverse_string:1.0").unwrap(),
            ToolPath::user("alice", "utils", "reverse_string", "1.0")
        );
        
        assert_eq!(
            ToolPath::parse("/bob/math/calculate").unwrap(),
            ToolPath::user("bob", "math", "calculate", "latest")
        );
    }
    
    #[test]
    fn test_mcp_names() {
        assert_eq!(
            ToolPath::bin("tcl_execute").to_mcp_name(),
            "bin___tcl_execute"
        );
        
        assert_eq!(
            ToolPath::user("alice", "utils", "reverse_string", "1.0").to_mcp_name(),
            "user_alice__utils___reverse_string__v1_0"
        );
        
        assert_eq!(
            ToolPath::user("bob", "math", "calculate", "latest").to_mcp_name(),
            "user_bob__math___calculate"
        );
    }
    
    #[test]
    fn test_round_trip() {
        let paths = vec![
            ToolPath::bin("tcl_execute"),
            ToolPath::sbin("tcl_tool_add"),
            ToolPath::user("alice", "utils", "reverse_string", "1.0"),
            ToolPath::user("bob", "math", "calculate", "latest"),
        ];
        
        for path in paths {
            let mcp_name = path.to_mcp_name();
            let parsed = ToolPath::from_mcp_name(&mcp_name).unwrap();
            assert_eq!(path, parsed);
        }
    }
}