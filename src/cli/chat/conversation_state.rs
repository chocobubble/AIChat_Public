pub struct ConversationState {
    messages: Vec<(String, String)>,
}

impl ConversationState {
    pub fn new() -> Self {
        Self {
            messages: Vec::new(),
        }
    }

    pub fn add_user_message(&mut self, message: &str) {
        self.messages.push(("user".to_string(), message.to_string()));
    }

    pub fn add_assistant_message(&mut self, message: &str) {
        self.messages.push(("assistant".to_string(), message.to_string()));
    }

    pub fn get_messages(&self) -> &[(String, String)] {
        &self.messages
    }

    pub fn clear(&mut self) {
        self.messages.clear();
    }
}
