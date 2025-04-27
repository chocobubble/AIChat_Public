use std::env;
use std::path::PathBuf;

pub struct ContextManager {
    pub current_dir: PathBuf,
    pub os_type: String,
    pub username: String,
}

impl ContextManager {
    pub fn new() -> Self {
        let current_dir = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        
        let os_type = if cfg!(target_os = "windows") {
            "windows".to_string()
        } else if cfg!(target_os = "macos") {
            "macos".to_string()
        } else if cfg!(target_os = "linux") {
            "linux".to_string()
        } else {
            "unknown".to_string()
        };
        
        let username = env::var("USER")
            .or_else(|_| env::var("USERNAME"))
            .unwrap_or_else(|_| "user".to_string());
        
        Self {
            current_dir,
            os_type,
            username,
        }
    }
    
    pub fn get_system_context(&self) -> String {
        format!(
            "Operating System: {}\nCurrent Directory: {}\nUsername: {}",
            self.os_type,
            self.current_dir.display(),
            self.username
        )
    }
}
