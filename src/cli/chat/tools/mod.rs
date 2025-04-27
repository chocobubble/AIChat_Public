pub mod execute_bash;
pub mod fs_read;
pub mod fs_write;
pub mod use_aws;

use std::path::{Path, PathBuf};

use eyre::Result;
use serde::{Deserialize, Serialize};

/// Maximum size in bytes for tool responses to prevent excessive output
pub const MAX_TOOL_RESPONSE_SIZE: usize = 1_000_000;

/// Represents the output of a tool invocation
#[derive(Debug, Clone, Serialize)]
pub struct ToolOutput {
    pub output: OutputKind,
}

/// Different kinds of output that tools can produce
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum OutputKind {
    /// Plain text output
    Text(String),
    
    /// JSON-formatted output
    Json(serde_json::Value),
    
    /// Binary data output (base64 encoded)
    Binary(String),
}

/// Specification for a tool that can be invoked
#[derive(Debug, Clone, Deserialize)]
pub struct ToolSpec {
    /// Name of the tool
    pub name: String,
    
    /// Description of what the tool does
    pub description: String,
    
    /// Parameters required by the tool
    pub parameters: serde_json::Value,
}

/// Trait for tools that can be invoked
pub trait Tool {
    /// Validate the tool parameters before execution
    fn validate(&self) -> Result<()>;
    
    /// Execute the tool and return its output
    fn execute(&self) -> Result<ToolOutput>;
    
    /// Get a description of what the tool will do
    fn describe(&self) -> String;
}

/// Sanitize a path argument from a tool call
///
/// This function ensures that paths are properly resolved and normalized.
/// It expands home directory references (~) and converts relative paths
/// to absolute paths based on the current working directory.
///
/// # Arguments
///
/// * `path` - The path string to sanitize
///
/// # Returns
///
/// A sanitized PathBuf
pub fn sanitize_path(path: &str) -> PathBuf {
    let path = path.trim();
    
    // Expand home directory if path starts with ~
    if path.starts_with('~') {
        if let Some(home) = dirs::home_dir() {
            if path.len() == 1 {
                return home;
            } else if path.starts_with("~/") {
                return home.join(&path[2..]);
            }
        }
    }
    
    // Convert to absolute path if relative
    let path_buf = Path::new(path);
    if path_buf.is_relative() {
        if let Ok(current_dir) = std::env::current_dir() {
            return current_dir.join(path_buf);
        }
    }
    
    path_buf.to_path_buf()
}

/// Format a path for display, showing it relative to a base directory if possible
///
/// # Arguments
///
/// * `base_dir` - The base directory to make paths relative to
/// * `path` - The path to format
///
/// # Returns
///
/// A formatted path string
pub fn format_path(base_dir: PathBuf, path: &Path) -> String {
    if let Ok(relative) = path.strip_prefix(&base_dir) {
        if relative.components().count() == 0 {
            return ".".to_string();
        }
        return relative.to_string_lossy().to_string();
    }
    path.to_string_lossy().to_string()
}
