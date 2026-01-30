/// Conversation history ring buffer for chat context.
use crate::api::types::AiMessage;

/// A fixed-capacity ring buffer that stores conversation messages.
///
/// When full, the oldest messages are discarded to make room for new ones.
/// The system message (if any) is always preserved.
pub struct ConversationHistory {
    messages: Vec<AiMessage>,
    capacity: usize,
}

impl ConversationHistory {
    /// Create a new history with the given capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            messages: Vec::with_capacity(capacity),
            capacity,
        }
    }

    /// Add a message to the history.
    ///
    /// If the buffer is full, the oldest non-system message is removed.
    pub fn push(&mut self, message: AiMessage) {
        if self.messages.len() >= self.capacity {
            // Find first non-system message to remove
            if let Some(idx) = self
                .messages
                .iter()
                .position(|m| m.role != crate::api::types::AiRole::System)
            {
                self.messages.remove(idx);
            } else {
                // All system messages - remove oldest
                self.messages.remove(0);
            }
        }
        self.messages.push(message);
    }

    /// Get all messages in order (for sending to LLM).
    pub fn messages(&self) -> &[AiMessage] {
        &self.messages
    }

    /// Clear all non-system messages.
    pub fn clear(&mut self) {
        self.messages
            .retain(|m| m.role == crate::api::types::AiRole::System);
    }

    /// Get the number of messages (excluding system).
    pub fn len(&self) -> usize {
        self.messages
            .iter()
            .filter(|m| m.role != crate::api::types::AiRole::System)
            .count()
    }

    /// Check if history is empty (no non-system messages).
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Format history for display.
    pub fn format_display(&self) -> String {
        let mut output = String::new();
        for msg in &self.messages {
            if msg.role == crate::api::types::AiRole::System {
                continue;
            }
            let role_label = match msg.role {
                crate::api::types::AiRole::User => "You",
                crate::api::types::AiRole::Assistant => "AI",
                crate::api::types::AiRole::System => continue,
            };
            output.push_str(&format!("[{}] {}\n", role_label, msg.content));
        }
        if output.is_empty() {
            output.push_str("(no chat history)");
        }
        output
    }
}
