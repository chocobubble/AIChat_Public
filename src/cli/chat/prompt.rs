use rustyline::{Config, Editor, Result};

pub fn generate_prompt(custom_prompt: Option<&str>) -> String {
    custom_prompt.unwrap_or("> ").to_string()
}

pub fn rl() -> Result<Editor<()>> {
    let config = Config::builder()
        .history_ignore_space(true)
        .completion_type(rustyline::CompletionType::List)
        .build();
    Editor::with_config(config)
}
