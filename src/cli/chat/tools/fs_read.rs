use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use eyre::{Result, eyre};

/// Read lines from a file.
///
/// # Arguments
///
/// * `path` - Path to the file to read
/// * `start_line` - Starting line number (1-based index, negative values count from end)
/// * `end_line` - Ending line number (1-based index, negative values count from end)
///
/// # Returns
///
/// The selected lines from the file as a string.
///
/// # Errors
///
/// Returns an error if:
/// - The file does not exist
/// - The path is not a file
/// - The file cannot be read
/// - The starting line is out of range
pub async fn read_file_lines(path: &str, start_line: i32, end_line: i32) -> Result<String> {
    let path = Path::new(path);
    if !path.exists() {
        // Check if there's a similar file that might be what the user intended
        if let Some(parent) = path.parent() {
            if let Ok(entries) = fs::read_dir(parent) {
                let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                for entry in entries {
                    if let Ok(entry) = entry {
                        let entry_name = entry.file_name().to_string_lossy().to_string();
                        if entry_name.to_lowercase() == filename.to_lowercase() || 
                           entry_name.contains(filename) || 
                           filename.contains(&entry_name) {
                            return Err(eyre!("File '{}' not found. Did you mean '{}'?", 
                                           path.display(), entry.path().display()));
                        }
                    }
                }
            }
        }
        return Err(eyre!("File not found: {}", path.display()));
    }

    if !path.is_file() {
        return Err(eyre!("Not a file: {}", path.display()));
    }

    let content = fs::read_to_string(path)?;
    let lines: Vec<&str> = content.lines().collect();
    let line_count = lines.len();
    
    // Convert negative indices to positive ones
    let start = if start_line < 0 {
        line_count.saturating_add(start_line as usize)
    } else {
        start_line.saturating_sub(1) as usize
    };
    
    let end = if end_line < 0 {
        line_count.saturating_add(end_line as usize).saturating_add(1)
    } else if end_line == 0 {
        line_count
    } else {
        end_line as usize
    };
    
    // Ensure start is within bounds
    if start >= line_count {
        return Err(eyre!(
            "Starting line {} is outside of the allowed range (1 to {})",
            start_line,
            line_count
        ));
    }
    
    // Ensure end is always greater than or equal to start
    let end = end.max(start);
    
    let selected_lines = lines.iter()
        .skip(start)
        .take(end.saturating_sub(start))
        .map(|&line| line.to_string())
        .collect::<Vec<String>>()
        .join("\n");
    
    Ok(selected_lines)
}

/// List directory contents with detailed information.
///
/// # Arguments
///
/// * `path` - Path to the directory to list
///
/// # Returns
///
/// A formatted string containing directory contents with file type, permissions,
/// size, modification time, and name.
///
/// # Errors
///
/// Returns an error if:
/// - The directory does not exist
/// - The path is not a directory
/// - The directory cannot be read
pub async fn list_directory(path: &str) -> Result<String> {
    let path = Path::new(path);
    if !path.exists() {
        return Err(eyre!("Directory not found: {}", path.display()));
    }

    if !path.is_dir() {
        return Err(eyre!("Not a directory: {}", path.display()));
    }

    let mut result = String::new();
    
    // Add header
    result.push_str("Type Permissions     Size  Modified             Name\n");
    result.push_str("---- ----------- -------- ------------------- ----------------\n");
    
    let mut entries = Vec::new();
    for entry_result in fs::read_dir(path)? {
        if let Ok(entry) = entry_result {
            entries.push(entry);
        }
    }
    
    // Sort entries (directories first, then alphabetically)
    entries.sort_by(|a, b| {
        let a_is_dir = a.path().is_dir();
        let b_is_dir = b.path().is_dir();
        
        if a_is_dir && !b_is_dir {
            std::cmp::Ordering::Less
        } else if !a_is_dir && b_is_dir {
            std::cmp::Ordering::Greater
        } else {
            a.file_name().cmp(&b.file_name())
        }
    });
    
    for entry in entries {
        let metadata = match entry.metadata() {
            Ok(md) => md,
            Err(_) => continue,
        };
        
        // File type
        let file_type = if metadata.is_dir() {
            "dir "
        } else if metadata.is_file() {
            "file"
        } else if metadata.is_symlink() {
            "link"
        } else {
            "other"
        };
        
        // Permissions (simplified for cross-platform compatibility)
        let permissions = if metadata.permissions().readonly() {
            "r--"
        } else {
            "rw-"
        };
        
        // Size
        let size = metadata.len();
        
        // Modification time
        let modified = metadata.modified().unwrap_or(SystemTime::now());
        let modified_secs = modified.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
        
        // Format date as "YYYY-MM-DD HH:MM"
        let datetime = chrono::DateTime::<chrono::Utc>::from_timestamp(modified_secs as i64, 0)
            .unwrap_or_default()
            .format("%Y-%m-%d %H:%M")
            .to_string();
        
        // Name (with trailing slash for directories)
        let name = entry.file_name().to_string_lossy().to_string();
        let display_name = if metadata.is_dir() {
            format!("{}/", name)
        } else {
            name
        };
        
        result.push_str(&format!("{} {:11} {:8} {} {}\n", 
                                file_type, permissions, size, datetime, display_name));
    }
    
    Ok(result)
}

/// Search for a pattern in a file with context lines.
///
/// # Arguments
///
/// * `path` - Path to the file to search
/// * `pattern` - Pattern to search for (case-insensitive)
/// * `context_lines` - Optional number of context lines to include (default: 2)
///
/// # Returns
///
/// A formatted string containing search results with line numbers and context.
///
/// # Errors
///
/// Returns an error if:
/// - The file does not exist
/// - The path is not a file
/// - The file cannot be read
pub async fn search_file(path: &str, pattern: &str, context_lines: Option<usize>) -> Result<String> {
    let path = Path::new(path);
    if !path.exists() {
        return Err(eyre!("File not found: {}", path.display()));
    }

    if !path.is_file() {
        return Err(eyre!("Not a file: {}", path.display()));
    }

    if pattern.is_empty() {
        return Err(eyre!("Search pattern cannot be empty"));
    }

    let content = fs::read_to_string(path)?;
    let lines: Vec<&str> = content.lines().collect();
    let context = context_lines.unwrap_or(2);
    
    let mut result = String::new();
    let mut matches_found = 0;
    
    // Case insensitive search
    let pattern_lower = pattern.to_lowercase();
    
    for (line_num, line) in lines.iter().enumerate() {
        if line.to_lowercase().contains(&pattern_lower) {
            matches_found += 1;
            
            // Add separator between matches
            if matches_found > 1 {
                result.push_str("\n--\n");
            }
            
            // Calculate context range
            let start = line_num.saturating_sub(context);
            let end = (line_num + context + 1).min(lines.len());
            
            // Add context lines
            for i in start..end {
                let prefix = if i == line_num { "→ " } else { "  " };
                result.push_str(&format!("{}{}: {}\n", prefix, i + 1, lines[i]));
            }
        }
    }
    
    if matches_found == 0 {
        result = format!("Pattern '{}' not found in {}", pattern, path.display());
    } else {
        result = format!("Found {} matches for pattern '{}' in {}:\n\n{}", 
                        matches_found, pattern, path.display(), result);
    }
    
    Ok(result)
}

/// Converts negative 1-based indices to positive 0-based indices.
/// 
/// # Arguments
///
/// * `line_count` - Total number of lines in the file
/// * `i` - Line index (1-based, negative values count from end)
///
/// # Returns
///
/// A positive 0-based index within the valid range.
fn convert_negative_index(line_count: usize, i: i32) -> usize {
    if i <= 0 {
        (line_count as i32 + i).max(0) as usize
    } else {
        i as usize - 1
    }
}
