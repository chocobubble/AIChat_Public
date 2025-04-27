use std::process::Command;

use eyre::{Result, eyre};

/// Execute a bash command and return its output.
///
/// This function runs the provided command in a bash shell and captures
/// both stdout and stderr output. It handles command execution in a secure
/// manner and provides detailed error information if the command fails.
///
/// # Arguments
///
/// * `command` - The bash command to execute as a string
///
/// # Returns
///
/// A string containing the combined stdout and stderr output of the command,
/// or an error if the command execution failed.
///
/// # Security Considerations
///
/// This function executes arbitrary shell commands, which can be dangerous
/// if used with untrusted input. Always validate and sanitize commands
/// before passing them to this function.
///
/// # Examples
///
/// ```
/// let result = execute_bash("ls -la").await?;
/// println!("{}", result);
/// ```
pub async fn execute_bash(command: &str) -> Result<String> {
    if command.trim().is_empty() {
        return Err(eyre!("Command cannot be empty"));
    }

    // Log the command being executed (for debugging purposes)
    tracing::debug!("Executing bash command: {}", command);

    // Execute the command using bash
    let output = Command::new("bash")
        .arg("-c")
        .arg(command)
        .output()
        .map_err(|e| eyre!("Failed to execute command: {}", e))?;

    // Combine stdout and stderr
    let mut result = String::new();
    
    // Add stdout if not empty
    if !output.stdout.is_empty() {
        result.push_str(&String::from_utf8_lossy(&output.stdout));
    }
    
    // Add stderr if not empty (with a prefix to distinguish it)
    if !output.stderr.is_empty() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        
        // If we already have stdout content, add a separator
        if !result.is_empty() && !result.ends_with('\n') {
            result.push('\n');
        }
        
        // Add stderr with a prefix if the command failed
        if !output.status.success() {
            result.push_str("Error: ");
        }
        
        result.push_str(&stderr);
    }
    
    // If the command failed and there's no output, provide a generic error message
    if !output.status.success() && result.is_empty() {
        result = format!("Command failed with exit code: {}", output.status);
    }
    
    // Ensure the result ends with a newline for better formatting
    if !result.is_empty() && !result.ends_with('\n') {
        result.push('\n');
    }
    
    Ok(result)
}
