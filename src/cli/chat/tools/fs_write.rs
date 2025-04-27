use std::fs;
use std::io::Write;
use std::path::Path;

use eyre::{Result, eyre};

/// Create a new file with the specified content.
///
/// If the file already exists, it will be overwritten. If the parent directories
/// don't exist, they will be created automatically.
///
/// # Arguments
///
/// * `path` - Path to the file to create
/// * `content` - Content to write to the file
///
/// # Returns
///
/// A success message indicating the file was created.
///
/// # Errors
///
/// Returns an error if:
/// - The parent directory cannot be created
/// - The file cannot be written to
pub async fn create_file(path: &str, content: &str) -> Result<String> {
    let path = Path::new(path);
    
    // Create parent directories if they don't exist
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent)
                .map_err(|e| eyre!("Failed to create directory {}: {}", parent.display(), e))?;
        }
    }
    
    // Write the content to the file
    fs::write(path, content)
        .map_err(|e| eyre!("Failed to write to file {}: {}", path.display(), e))?;
    
    Ok(format!("File created successfully: {}", path.display()))
}

/// Replace a string in a file with a new string.
///
/// # Arguments
///
/// * `path` - Path to the file to modify
/// * `old_str` - String to replace
/// * `new_str` - New string to insert
///
/// # Returns
///
/// A success message indicating the string was replaced.
///
/// # Errors
///
/// Returns an error if:
/// - The file does not exist
/// - The file cannot be read
/// - The old string is not found in the file
/// - The file cannot be written to
pub async fn replace_in_file(path: &str, old_str: &str, new_str: &str) -> Result<String> {
    let path = Path::new(path);
    if !path.exists() {
        return Err(eyre!("File not found: {}", path.display()));
    }

    // Read the file content
    let content = fs::read_to_string(path)
        .map_err(|e| eyre!("Failed to read file {}: {}", path.display(), e))?;
    
    // Check if the old string exists in the file
    if !content.contains(old_str) {
        return Err(eyre!("String not found in file: {}", old_str));
    }
    
    // Replace the string
    let new_content = content.replace(old_str, new_str);
    
    // Write the modified content back to the file
    fs::write(path, new_content)
        .map_err(|e| eyre!("Failed to write to file {}: {}", path.display(), e))?;
    
    Ok(format!("String replaced successfully in {}", path.display()))
}

/// Append content to the end of a file.
///
/// # Arguments
///
/// * `path` - Path to the file to append to
/// * `content` - Content to append
///
/// # Returns
///
/// A success message indicating the content was appended.
///
/// # Errors
///
/// Returns an error if:
/// - The file does not exist
/// - The file cannot be opened for appending
/// - The content cannot be written to the file
pub async fn append_to_file(path: &str, content: &str) -> Result<String> {
    let path = Path::new(path);
    if !path.exists() {
        return Err(eyre!("File not found: {}", path.display()));
    }

    // Read the current content first
    let current_content = fs::read_to_string(path)
        .map_err(|e| eyre!("Failed to read file {}: {}", path.display(), e))?;
    
    // Open the file for writing
    let mut file = fs::OpenOptions::new()
        .write(true)
        .append(true)
        .open(path)
        .map_err(|e| eyre!("Failed to open file for appending {}: {}", path.display(), e))?;
    
    // Add a newline if the file doesn't end with one and isn't empty
    if !current_content.is_empty() && !current_content.ends_with('\n') {
        file.write_all(b"\n")
            .map_err(|e| eyre!("Failed to write newline to file {}: {}", path.display(), e))?;
    }
    
    // Write the new content
    file.write_all(content.as_bytes())
        .map_err(|e| eyre!("Failed to append to file {}: {}", path.display(), e))?;
    
    // Always end with a newline for better formatting
    if !content.ends_with('\n') {
        file.write_all(b"\n")
            .map_err(|e| eyre!("Failed to write final newline to file {}: {}", path.display(), e))?;
    }
    
    Ok(format!("Content appended successfully to {}", path.display()))
}

/// Insert content at a specific line in a file.
///
/// # Arguments
///
/// * `path` - Path to the file to modify
/// * `line_number` - Line number to insert at (1-based index)
/// * `content` - Content to insert
///
/// # Returns
///
/// A success message indicating the content was inserted.
///
/// # Errors
///
/// Returns an error if:
/// - The file does not exist
/// - The file cannot be read
/// - The line number is out of range
/// - The file cannot be written to
pub async fn insert_in_file(path: &str, line_number: usize, content: &str) -> Result<String> {
    let path = Path::new(path);
    if !path.exists() {
        return Err(eyre!("File not found: {}", path.display()));
    }

    // Read the file content
    let file_content = fs::read_to_string(path)
        .map_err(|e| eyre!("Failed to read file {}: {}", path.display(), e))?;
    
    // Split the content into lines
    let lines: Vec<&str> = file_content.lines().collect();
    
    // Check if the line number is valid
    if line_number > lines.len() {
        return Err(eyre!("Line number {} is out of range (file has {} lines)", 
                       line_number, lines.len()));
    }
    
    // Insert the content at the specified line
    let mut new_lines = Vec::new();
    for (i, &line) in lines.iter().enumerate() {
        new_lines.push(line);
        if i + 1 == line_number {
            new_lines.push(content);
        }
    }
    
    // Join the lines back together
    let new_content = new_lines.join("\n");
    
    // Write the modified content back to the file
    fs::write(path, new_content)
        .map_err(|e| eyre!("Failed to write to file {}: {}", path.display(), e))?;
    
    Ok(format!("Content inserted successfully at line {} in {}", line_number, path.display()))
}
