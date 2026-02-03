/// L1 Common: Request/response types for the AI service.

/// Role of a message participant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiRole {
    System,
    User,
    Assistant,
}

/// A single message in a conversation.
#[derive(Debug, Clone)]
pub struct AiMessage {
    pub role: AiRole,
    pub content: String,
}

impl AiMessage {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: AiRole::System,
            content: content.into(),
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: AiRole::User,
            content: content.into(),
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: AiRole::Assistant,
            content: content.into(),
        }
    }
}

/// Options controlling LLM completion behavior.
#[derive(Debug, Clone)]
pub struct CompletionOptions {
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
}

impl Default for CompletionOptions {
    fn default() -> Self {
        Self {
            temperature: Some(0.3),
            max_tokens: Some(1024),
        }
    }
}

/// Raw LLM response.
#[derive(Debug, Clone)]
pub struct AiResponse {
    pub content: String,
    pub model: String,
}

// ── Feature-specific request/response types ──

/// Request to translate natural language to a shell command.
#[derive(Debug, Clone)]
pub struct TranslateRequest {
    pub natural_language: String,
    pub cwd: String,
    pub recent_commands: Vec<String>,
}

/// Response containing the translated shell command.
#[derive(Debug, Clone)]
pub struct TranslateResponse {
    pub command: String,
    pub explanation: String,
}

/// Request to explain a shell command.
#[derive(Debug, Clone)]
pub struct ExplainRequest {
    pub command: String,
}

/// Response with the command explanation.
#[derive(Debug, Clone)]
pub struct ExplainResponse {
    pub explanation: String,
}

/// Request for a conversational chat message.
#[derive(Debug, Clone)]
pub struct ChatRequest {
    pub message: String,
}

/// Response from the chat assistant.
#[derive(Debug, Clone)]
pub struct ChatResponse {
    pub reply: String,
}

/// Request for autocomplete suggestions.
#[derive(Debug, Clone)]
pub struct AutocompleteRequest {
    pub partial_input: String,
    pub cwd: String,
    pub cwd_entries: Vec<String>,
    pub recent_commands: Vec<String>,
}

/// Response with autocomplete suggestions.
#[derive(Debug, Clone)]
pub struct AutocompleteResponse {
    pub suggestions: Vec<String>,
}

/// Status of the AI service.
#[derive(Debug, Clone)]
pub struct AiStatus {
    pub enabled: bool,
    pub provider: String,
    pub model: String,
    pub ready: bool,
    pub description: String,
}

/// Information about a registered agent.
#[derive(Debug, Clone)]
pub struct AgentInfo {
    /// Unique agent identifier (e.g. "shell", "review").
    pub id: String,
    /// Human-readable name (e.g. "Shell Assistant").
    pub display_name: String,
    /// Short description of the agent's purpose.
    pub description: String,
    /// Whether this agent is currently active.
    pub active: bool,
}

/// Events emitted during a streaming chat response.
#[derive(Debug, Clone)]
pub enum ChatStreamEvent {
    /// A partial content delta (token chunk).
    Delta(String),
    /// Stream complete — contains the full assembled reply.
    Done(String),
}
