pub enum Command {
    Help,
    Clear,
    Quit,
    ShellCommand(String),
    ChatMessage(String),
}
