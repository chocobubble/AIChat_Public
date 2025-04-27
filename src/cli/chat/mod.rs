pub mod command;
pub mod context;
pub mod conversation_state;
pub mod input_source;
pub mod parse;
pub mod parser;
pub mod prompt;
pub mod tools;

use std::io::Write;
use std::process::ExitCode;

use command::Command;
use context::ContextManager;
use conversation_state::ConversationState;
use eyre::{Result, bail};
use prompt::generate_prompt;
use regex::Regex;
use serde_json::{json, Value};
use tracing::error;

use crate::cli::chat::tools::execute_bash;
use crate::cli::chat::tools::fs_read;
use crate::cli::chat::tools::fs_write;
use crate::gemini_client::{GeminiClient, ToolDefinition};

const WELCOME_TEXT: &str = "
Hi, I'm Gemini Chat. Ask me anything.

Things to try
• Fix the build failures in this project.
• List files in the current directory.
• Write unit tests for my application.
• Help me understand my git status

/help         Show the help dialogue
/quit         Quit the application
";

const HELP_TEXT: &str = "
Gemini Chat CLI

/clear        Clear the conversation history
/help         Show this help dialogue
/quit         Quit the application

!{command}    Quickly execute a command in your current session
";

pub struct ChatContext {
    output: Box<dyn Write>,
    input: Option<String>,
    interactive: bool,
    conversation_state: ConversationState,
    context_manager: Option<ContextManager>,
    accept_all: bool,
    gemini_client: Option<GeminiClient>,
}

impl ChatContext {
    pub fn new(
        output: Box<dyn Write>,
        input: Option<String>,
        interactive: bool,
        accept_all: bool,
    ) -> Self {
        Self {
            output,
            input,
            interactive,
            conversation_state: ConversationState::new(),
            context_manager: Some(ContextManager::new()),
            accept_all,
            gemini_client: None,
        }
    }

    pub async fn run(&mut self) -> Result<ExitCode> {
        // Initialize Gemini client
        self.gemini_client = match GeminiClient::new() {
            Ok(client) => Some(client),
            Err(e) => {
                writeln!(self.output, "Failed to initialize Gemini client: {}", e)?;
                return Ok(ExitCode::FAILURE);
            }
        };

        if self.interactive {
            self.print_welcome()?;
        }

        // Handle non-interactive mode (single query)
        if let Some(input) = self.input.take() {
            self.handle_input(&input).await?;
            return Ok(ExitCode::SUCCESS);
        }

        // Interactive mode
        if self.interactive {
            self.run_interactive().await?;
        }

        Ok(ExitCode::SUCCESS)
    }

    fn print_welcome(&mut self) -> Result<()> {
        writeln!(self.output, "{}", WELCOME_TEXT)?;
        Ok(())
    }

    async fn run_interactive(&mut self) -> Result<()> {
        let mut rl = prompt::rl()?;
        
        loop {
            let prompt_text = generate_prompt(None);
            let readline = rl.readline(&prompt_text);
            
            match readline {
                Ok(line) => {
                    if line.trim().is_empty() {
                        continue;
                    }
                    
                    rl.add_history_entry(line.as_str());
                    
                    if line.trim() == "/quit" {
                        break;
                    }
                    
                    if let Err(e) = self.handle_input(&line).await {
                        writeln!(self.output, "Error: {}", e)?;
                    }
                }
                Err(e) => {
                    writeln!(self.output, "Error: {}", e)?;
                    break;
                }
            }
        }
        
        Ok(())
    }

    async fn handle_input(&mut self, input: &str) -> Result<()> {
        match input.trim() {
            "/help" => {
                writeln!(self.output, "{}", HELP_TEXT)?;
            }
            "/clear" => {
                self.conversation_state = ConversationState::new();
                writeln!(self.output, "Conversation cleared.")?;
            }
            _ => {
                if input.starts_with('!') {
                    // Handle shell command
                    let cmd = &input[1..];
                    let result = execute_bash::execute_bash(cmd).await?;
                    writeln!(self.output, "{}", result)?;
                } else {
                    // Handle normal chat input
                    self.process_chat_input(input).await?;
                }
            }
        }
        
        Ok(())
    }

    async fn process_chat_input(&mut self, input: &str) -> Result<()> {
        // Add user message to conversation state
        self.conversation_state.add_user_message(input);
        
        // Get response from Gemini API
        let response = self.get_gemini_response().await?;
        
        // Display response
        self.display_response(&response).await?;
        
        Ok(())
    }

    async fn display_response(&mut self, response: &str) -> Result<()> {
        // Check if the response contains tool calls
        if let Some((text, tool_calls)) = self.extract_tool_calls(response) {
            // Display the text part
            if !text.trim().is_empty() {
                writeln!(self.output, "{}", text)?;
            }
            
            // Process tool calls
            for tool_call in tool_calls {
                // Execute the tool call
                let result = match self.execute_tool_call(&tool_call).await {
                    Ok(res) => res,
                    Err(e) => {
                        let error_msg = format!("Error executing tool call: {}", e);
                        writeln!(self.output, "{}", error_msg)?;
                        error_msg
                    }
                };
                
                // Add tool call and result to conversation
                self.conversation_state.add_assistant_message(&format!("Tool call: {}", tool_call));
                self.conversation_state.add_user_message(&format!("Tool result: {}", result));
                
                // Get follow-up response from Gemini
                let follow_up = self.get_gemini_response().await?;
                
                // Check if follow-up response also contains tool calls
                if let Some((follow_text, follow_tool_calls)) = self.extract_tool_calls(&follow_up) {
                    if !follow_text.trim().is_empty() {
                        writeln!(self.output, "{}", follow_text)?;
                    }
                    
                    // Process nested tool calls recursively (limited to one level of nesting)
                    for follow_tool_call in follow_tool_calls {
                        let follow_result = self.execute_tool_call(&follow_tool_call).await?;
                        
                        // Add nested tool call and result to conversation
                        self.conversation_state.add_assistant_message(&format!("Tool call: {}", follow_tool_call));
                        self.conversation_state.add_user_message(&format!("Tool result: {}", follow_result));
                        
                        // Get final response after nested tool call
                        let final_response = self.get_gemini_response().await?;
                        writeln!(self.output, "{}", final_response)?;
                        self.conversation_state.add_assistant_message(&final_response);
                    }
                } else {
                    // No nested tool calls, display the follow-up response
                    writeln!(self.output, "{}", follow_up)?;
                    self.conversation_state.add_assistant_message(&follow_up);
                }
            }
        } else {
            // Regular response, just display it
            writeln!(self.output, "{}", response)?;
            self.conversation_state.add_assistant_message(response);
        }
        
        Ok(())
    }

    fn extract_tool_calls(&self, response: &str) -> Option<(String, Vec<String>)> {
        // First try to extract XML-style function calls
        let xml_result = self.extract_xml_tool_calls(response);
        if xml_result.is_some() {
            return xml_result;
        }
        
        // If no XML-style function calls found, try to extract JSON-style tool calls
        self.extract_json_tool_calls(response)
    }
    
    fn extract_xml_tool_calls(&self, response: &str) -> Option<(String, Vec<String>)> {
        // Regular expression to extract tool calls in XML format
        let re = Regex::new(r#"<function_calls>([\s\S]*?)</function_calls>"#).ok()?;
        
        if let Some(captures) = re.captures(response) {
            let tool_call_block = captures.get(1)?.as_str();
            
            // Extract individual tool calls
            let tool_re = Regex::new(r#"<invoke name="([^"]+)">([\s\S]*?)</invoke>"#).ok()?;
            let mut tool_calls = Vec::new();
            
            for tool_match in tool_re.captures_iter(tool_call_block) {
                let tool_name = tool_match.get(1)?.as_str();
                let tool_params = tool_match.get(2)?.as_str();
                
                // Format the tool call as JSON
                let mut params_map = serde_json::Map::new();
                
                // Extract parameters
                let param_re = Regex::new(r#"<parameter name="([^"]+)">([^<]*)</parameter>"#).ok()?;
                for param_match in param_re.captures_iter(tool_params) {
                    let param_name = param_match.get(1)?.as_str();
                    let param_value = param_match.get(2)?.as_str();
                    params_map.insert(param_name.to_string(), Value::String(param_value.to_string()));
                }
                
                let tool_call_json = json!({
                    "name": tool_name,
                    "parameters": params_map
                });
                
                tool_calls.push(tool_call_json.to_string());
            }
            
            // Get the text part (everything before the first tool call)
            let text_part = response.split("<function_calls>").next().unwrap_or("").trim();
            
            return Some((text_part.to_string(), tool_calls));
        }
        
        None
    }
    
    fn extract_json_tool_calls(&self, response: &str) -> Option<(String, Vec<String>)> {
        // Regular expression to extract JSON-style tool calls
        // This pattern looks for: Tool call: {"name":"tool_name","parameters":{...}}
        let re = Regex::new(r#"Tool call: (\{.*?\})"#).ok()?;
        
        let mut tool_calls = Vec::new();
        let mut last_end = 0;
        let mut text_parts = Vec::new();
        
        for captures in re.captures_iter(response) {
            if let Some(json_match) = captures.get(1) {
                // Add the text before this tool call to text_parts
                if let Some(match_start) = captures.get(0) {
                    let start_pos = match_start.start();
                    if start_pos > last_end {
                        text_parts.push(&response[last_end..start_pos]);
                    }
                    last_end = match_start.end();
                }
                
                // Try to parse the JSON
                let json_str = json_match.as_str();
                if let Ok(_) = serde_json::from_str::<Value>(json_str) {
                    // If it's valid JSON, add it to tool_calls
                    tool_calls.push(json_str.to_string());
                }
            }
        }
        
        // Add any remaining text after the last tool call
        if last_end < response.len() {
            text_parts.push(&response[last_end..]);
        }
        
        if !tool_calls.is_empty() {
            // Join all text parts that aren't tool calls
            let text_part = text_parts.join("").trim().to_string();
            return Some((text_part, tool_calls));
        }
        
        None
    }

    async fn execute_tool_call(&self, tool_call: &str) -> Result<String> {
        let tool_call: Value = serde_json::from_str(tool_call)?;
        
        let tool_name = tool_call["name"].as_str().unwrap_or("");
        let parameters = tool_call["parameters"].as_object().unwrap_or(&serde_json::Map::new()).clone();
        
        match tool_name {
            "execute_bash" => {
                let command = parameters.get("command").and_then(|v| v.as_str()).unwrap_or("");
                execute_bash::execute_bash(command).await
            }
            "fs_read" => {
                let path = parameters.get("path").and_then(|v| v.as_str()).unwrap_or("");
                let mode = parameters.get("mode").and_then(|v| v.as_str()).unwrap_or("Line");
                
                // Check if the path exists, if not, try to find similar files
                let result = match mode {
                    "Line" => {
                        let start_line = parameters.get("start_line").and_then(|v| v.as_i64()).unwrap_or(1) as i32;
                        let end_line = parameters.get("end_line").and_then(|v| v.as_i64()).unwrap_or(-1) as i32;
                        fs_read::read_file_lines(path, start_line, end_line).await
                    }
                    "Directory" => {
                        // For directory mode, create the directory if it doesn't exist
                        let dir_path = std::path::Path::new(path);
                        if !dir_path.exists() {
                            // Try to create the directory
                            match std::fs::create_dir_all(dir_path) {
                                Ok(_) => {
                                    tracing::info!("Created directory: {}", path);
                                    // Return empty directory listing
                                    return Ok(format!("Directory created: {}\nThe directory is empty.", path));
                                }
                                Err(e) => {
                                    tracing::error!("Failed to create directory {}: {}", path, e);
                                    // Continue with normal flow, the list_directory will return an error
                                }
                            }
                        }
                        fs_read::list_directory(path).await
                    }
                    "Search" => {
                        let pattern = parameters.get("pattern").and_then(|v| v.as_str()).unwrap_or("");
                        let context_lines = parameters.get("context_lines").and_then(|v| v.as_i64()).map(|v| v as usize);
                        fs_read::search_file(path, pattern, context_lines).await
                    }
                    _ => bail!("Invalid fs_read mode: {}", mode)
                };
                
                // If there's an error and it's about a file not found, try to list the directory
                // to help the model understand what files are available
                if let Err(e) = &result {
                    if e.to_string().contains("File not found") || e.to_string().contains("not found") {
                        // Try to list the current directory to help the model
                        let dir_path = std::path::Path::new(path).parent().unwrap_or(std::path::Path::new("."));
                        if let Ok(dir_listing) = fs_read::list_directory(dir_path.to_str().unwrap_or(".")).await {
                            return Ok(format!("Error: {}.\n\nAvailable files in directory:\n{}", e, dir_listing));
                        }
                    }
                }
                
                result
            }
            "fs_write" => {
                let path = parameters.get("path").and_then(|v| v.as_str()).unwrap_or("");
                let command = parameters.get("command").and_then(|v| v.as_str()).unwrap_or("");
                
                match command {
                    "create" => {
                        let file_text = parameters.get("file_text").and_then(|v| v.as_str()).unwrap_or("");
                        fs_write::create_file(path, file_text).await
                    }
                    "str_replace" => {
                        let old_str = parameters.get("old_str").and_then(|v| v.as_str()).unwrap_or("");
                        let new_str = parameters.get("new_str").and_then(|v| v.as_str()).unwrap_or("");
                        fs_write::replace_in_file(path, old_str, new_str).await
                    }
                    "append" => {
                        let content = parameters.get("new_str").and_then(|v| v.as_str()).unwrap_or("");
                        fs_write::append_to_file(path, content).await
                    }
                    "insert" => {
                        let insert_line = parameters.get("insert_line").and_then(|v| v.as_i64()).unwrap_or(0) as usize;
                        let content = parameters.get("new_str").and_then(|v| v.as_str()).unwrap_or("");
                        fs_write::insert_in_file(path, insert_line, content).await
                    }
                    _ => bail!("Invalid fs_write command: {}", command)
                }
            }
            _ => bail!("Unknown tool: {}", tool_name)
        }
    }

    fn create_system_prompt(&self) -> String {
        let mut prompt = r#"You are Gemini Chat, a helpful AI assistant similar to Amazon Q. You help with coding, answering questions, and system operations.

# Key capabilities
- Knowledge about the user's system context
- Interact with local filesystem to list, read and write files
- Execute bash commands on the user's system
- Provide software focused assistance and recommendations
- Help with infrastructure code and configurations
- Guide users on best practices
- Analyze and optimize resource usage
- Troubleshoot issues and errors
- Assist with CLI commands and automation tasks
- Write and modify software code
- Test and debug software

# Important
You don't have direct access to the user's system. Instead, you must use tools to interact with it.
When you need information about files, directories, or need to run commands, use the appropriate tool.

Available tools:
1. execute_bash - Run shell commands to gather information or perform actions
2. fs_read - Read files or list directories
3. fs_write - Create or modify files

When you need to use a tool, the system will handle the formatting for you. Just focus on providing
the correct tool name and parameters.

Always use these tools when you need system information rather than asking the user to provide it.
After receiving tool results, provide a comprehensive response based on the information gathered.
"#.to_string();

        // Add system context if available
        if let Some(context_manager) = &self.context_manager {
            prompt.push_str("\n\n# System Context\n");
            prompt.push_str(&context_manager.get_system_context());
        }

        prompt
    }

    fn get_tool_definitions(&self) -> Vec<ToolDefinition> {
        vec![
            ToolDefinition {
                name: "execute_bash".to_string(),
                description: "Execute a bash command".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "command": {
                            "type": "string",
                            "description": "The bash command to execute"
                        }
                    },
                    "required": ["command"]
                }),
            },
            ToolDefinition {
                name: "fs_read".to_string(),
                description: "Read a file or directory".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Path to the file or directory"
                        },
                        "mode": {
                            "type": "string",
                            "enum": ["Line", "Directory", "Search"],
                            "description": "Mode to read the file or directory"
                        },
                        "start_line": {
                            "type": "integer",
                            "description": "Starting line number (optional, for Line mode)"
                        },
                        "end_line": {
                            "type": "integer",
                            "description": "Ending line number (optional, for Line mode)"
                        },
                        "pattern": {
                            "type": "string",
                            "description": "Pattern to search for (required, for Search mode)"
                        }
                    },
                    "required": ["path", "mode"]
                }),
            },
            ToolDefinition {
                name: "fs_write".to_string(),
                description: "Write to a file".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Path to the file"
                        },
                        "command": {
                            "type": "string",
                            "enum": ["create", "str_replace", "insert", "append"],
                            "description": "Command to perform on the file"
                        },
                        "file_text": {
                            "type": "string",
                            "description": "Content to write to the file (for create command)"
                        },
                        "old_str": {
                            "type": "string",
                            "description": "String to replace (for str_replace command)"
                        },
                        "new_str": {
                            "type": "string",
                            "description": "New string (for str_replace, insert, append commands)"
                        },
                        "insert_line": {
                            "type": "integer",
                            "description": "Line number to insert at (for insert command)"
                        }
                    },
                    "required": ["path", "command"]
                }),
            }
        ]
    }

    async fn get_gemini_response(&self) -> Result<String> {
        let client = match &self.gemini_client {
            Some(client) => client,
            None => bail!("Gemini client not initialized"),
        };
        
        // Create system prompt
        let system_prompt = self.create_system_prompt();
        
        // Get conversation history
        let messages = self.conversation_state.get_messages();
        
        // Convert messages to format expected by Gemini client
        let formatted_messages: Vec<(&str, &str)> = messages.iter()
            .map(|(role, content)| (role.as_str(), content.as_str()))
            .collect();
        
        // Define available tools
        let tools = self.get_tool_definitions();
        
        // Call Gemini API
        let response = client.generate_content(&system_prompt, &formatted_messages, &tools).await?;
        
        Ok(response)
    }
}
