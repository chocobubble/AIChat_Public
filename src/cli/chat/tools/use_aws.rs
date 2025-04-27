use eyre::{Result, eyre};
use std::process::Command;

pub async fn use_aws(
    service_name: &str,
    operation_name: &str,
    region: &str,
    parameters: &str,
    profile_name: Option<&str>,
    label: &str,
) -> Result<String> {
    let mut cmd = Command::new("aws");
    
    cmd.arg(service_name)
        .arg(operation_name)
        .arg("--region")
        .arg(region);
    
    // Add profile if specified
    if let Some(profile) = profile_name {
        cmd.arg("--profile").arg(profile);
    }
    
    // Parse and add parameters
    if !parameters.is_empty() {
        let params: serde_json::Value = serde_json::from_str(parameters)?;
        
        if let serde_json::Value::Object(map) = params {
            for (key, value) in map {
                let param_key = format!("--{}", key.replace('_', "-"));
                
                match value {
                    serde_json::Value::String(s) => {
                        if s.is_empty() {
                            // Flag with no value
                            cmd.arg(&param_key);
                        } else {
                            cmd.arg(&param_key).arg(s);
                        }
                    },
                    serde_json::Value::Number(n) => {
                        cmd.arg(&param_key).arg(n.to_string());
                    },
                    serde_json::Value::Bool(b) => {
                        cmd.arg(&param_key).arg(b.to_string());
                    },
                    serde_json::Value::Array(_) | serde_json::Value::Object(_) => {
                        let json_str = serde_json::to_string(&value)?;
                        cmd.arg(&param_key).arg(json_str);
                    },
                    serde_json::Value::Null => {
                        // Skip null values
                    },
                }
            }
        }
    }
    
    // Execute the command
    let output = cmd.output()?;
    
    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        Ok(stdout)
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        Err(eyre!("AWS CLI error: {}", stderr))
    }
}
