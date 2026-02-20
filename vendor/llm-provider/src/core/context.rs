//! Context window management and token validation
//!
//! Provides pre-flight validation, auto-truncation, and warning thresholds
//! for LLM context window management.
//!
//! # Features
//!
//! 1. **Pre-flight validation** - Validates token count before sending to LLM
//! 2. **Auto-truncation** - Automatically truncates/summarizes older messages when near limit
//! 3. **Warning threshold** - Emits warnings at configurable capacity threshold (default 80%)
//!
//! # Example
//!
//! ```ignore
//! use llm_provider::core::context::{ContextValidator, ContextConfig};
//!
//! let validator = ContextValidator::new(ContextConfig {
//!     warning_threshold: 0.8,
//!     auto_truncate: true,
//!     reserve_for_response: 4096,
//! });
//!
//! let result = validator.validate(&request, &model_info)?;
//! match result {
//!     ValidationResult::Ok => { /* proceed */ }
//!     ValidationResult::Warning { used, max, message } => {
//!         warn!("{}", message);
//!         // proceed with caution
//!     }
//!     ValidationResult::Truncated { original, truncated, removed_count } => {
//!         // use truncated messages
//!     }
//! }
//! ```

use crate::api::{
    CompletionRequest, ContentPart, LlmError, LlmResult, Message, MessageContent, ModelInfo,
    ToolDefinition,
};
use tracing::{debug, info, warn};

/// Configuration for context validation
#[derive(Debug, Clone)]
pub struct ContextConfig {
    /// Warning threshold as a fraction of context window (0.0-1.0)
    /// Default: 0.8 (80%)
    pub warning_threshold: f32,

    /// Whether to automatically truncate messages when exceeding limit
    /// Default: true
    pub auto_truncate: bool,

    /// Tokens to reserve for the model's response
    /// Default: 4096
    pub reserve_for_response: u32,

    /// Minimum messages to keep (system + last N user messages)
    /// Default: 3
    pub min_messages_to_keep: usize,

    /// Characters per token estimate (for simple heuristic)
    /// Default: 4.0
    pub chars_per_token: f32,
}

impl Default for ContextConfig {
    fn default() -> Self {
        Self {
            warning_threshold: 0.8,
            auto_truncate: true,
            reserve_for_response: 4096,
            min_messages_to_keep: 3,
            chars_per_token: 4.0,
        }
    }
}

/// Result of context validation
#[derive(Debug, Clone)]
pub enum ValidationResult {
    /// Request is within limits
    Ok {
        estimated_tokens: u32,
        context_window: u32,
        utilization: f32,
    },

    /// Request is approaching limit (above warning threshold)
    Warning {
        estimated_tokens: u32,
        context_window: u32,
        utilization: f32,
        message: String,
    },

    /// Request exceeded limit and was truncated
    Truncated {
        original_tokens: u32,
        truncated_tokens: u32,
        removed_message_count: usize,
        messages: Vec<Message>,
    },

    /// Request exceeds limit and cannot be truncated further
    Exceeded {
        estimated_tokens: u32,
        context_window: u32,
        message: String,
    },
}

impl ValidationResult {
    /// Check if validation passed (Ok or Warning)
    pub fn is_ok(&self) -> bool {
        matches!(self, ValidationResult::Ok { .. } | ValidationResult::Warning { .. })
    }

    /// Check if truncation occurred
    pub fn is_truncated(&self) -> bool {
        matches!(self, ValidationResult::Truncated { .. })
    }

    /// Check if validation failed
    pub fn is_exceeded(&self) -> bool {
        matches!(self, ValidationResult::Exceeded { .. })
    }

    /// Get the (possibly truncated) messages
    pub fn messages(&self) -> Option<&Vec<Message>> {
        match self {
            ValidationResult::Truncated { messages, .. } => Some(messages),
            _ => None,
        }
    }
}

/// A conversation unit that groups related messages together
/// (e.g., assistant message with tool_calls + all tool result messages)
#[derive(Debug, Clone)]
struct ConversationUnit {
    messages: Vec<Message>,
    tokens: u32,
}

/// Context validator for pre-flight token validation
#[derive(Debug, Clone)]
pub struct ContextValidator {
    config: ContextConfig,
}

impl Default for ContextValidator {
    fn default() -> Self {
        Self::new(ContextConfig::default())
    }
}

impl ContextValidator {
    /// Create a new context validator with the given configuration
    pub fn new(config: ContextConfig) -> Self {
        Self { config }
    }

    /// Estimate tokens for a single message
    pub fn estimate_message_tokens(&self, message: &Message) -> u32 {
        let content_tokens = match &message.content {
            MessageContent::Text(text) => self.estimate_text_tokens(text),
            MessageContent::Parts(parts) => {
                parts.iter().map(|p| self.estimate_part_tokens(p)).sum()
            }
        };

        // Add overhead for message structure (role, etc.)
        // Roughly 4 tokens for message formatting
        let overhead = 4;

        // Add tokens for tool calls if present
        let tool_call_tokens: u32 = message
            .tool_calls
            .iter()
            .map(|tc| {
                // Tool call overhead + function name + arguments
                10 + self.estimate_text_tokens(&tc.name)
                    + self.estimate_text_tokens(&tc.arguments)
            })
            .sum();

        content_tokens + overhead + tool_call_tokens
    }

    /// Estimate tokens for text content
    fn estimate_text_tokens(&self, text: &str) -> u32 {
        // Simple heuristic: chars / chars_per_token
        // This is approximate - tiktoken would be more accurate
        let chars = text.chars().count() as f32;
        (chars / self.config.chars_per_token).ceil() as u32
    }

    /// Estimate tokens for a content part
    fn estimate_part_tokens(&self, part: &ContentPart) -> u32 {
        match part {
            ContentPart::Text { text } => self.estimate_text_tokens(text),
            ContentPart::ImageUrl { .. } => {
                // Images typically use ~85 tokens for low detail, ~765 for high detail
                // Use a conservative estimate
                500
            }
            ContentPart::ImageBase64 { data, .. } => {
                // Estimate based on base64 data size
                // Very rough: base64 images use variable tokens
                let size_kb = data.len() / 1024;
                (100 + size_kb * 10) as u32
            }
        }
    }

    /// Estimate tokens for tool definitions
    fn estimate_tools_tokens(&self, tools: &[ToolDefinition]) -> u32 {
        tools
            .iter()
            .map(|tool| {
                // Tool definition overhead + name + description + schema
                20 + self.estimate_text_tokens(&tool.name)
                    + self.estimate_text_tokens(&tool.description)
                    + self.estimate_text_tokens(&tool.parameters.to_string())
            })
            .sum()
    }

    /// Estimate total tokens for a completion request
    pub fn estimate_request_tokens(&self, request: &CompletionRequest) -> u32 {
        let message_tokens: u32 = request
            .messages
            .iter()
            .map(|m| self.estimate_message_tokens(m))
            .sum();

        let tools_tokens = request
            .tools
            .as_ref()
            .map(|t| self.estimate_tools_tokens(t))
            .unwrap_or(0);

        // Add some overhead for the request structure
        let request_overhead = 10;

        message_tokens + tools_tokens + request_overhead
    }

    /// Validate a request against the model's context window
    pub fn validate(
        &self,
        request: &CompletionRequest,
        model_info: &ModelInfo,
    ) -> ValidationResult {
        let estimated_tokens = self.estimate_request_tokens(request);
        let available_window = model_info
            .context_window
            .saturating_sub(self.config.reserve_for_response);
        let utilization = estimated_tokens as f32 / available_window as f32;

        debug!(
            model = %model_info.id,
            estimated_tokens = estimated_tokens,
            context_window = model_info.context_window,
            available_window = available_window,
            utilization = %format!("{:.1}%", utilization * 100.0),
            "Context validation"
        );

        // Check if within limits
        if estimated_tokens <= available_window {
            // Check warning threshold
            if utilization >= self.config.warning_threshold {
                let message = format!(
                    "Context window at {:.0}% capacity ({}/{} tokens). Consider starting a new conversation.",
                    utilization * 100.0,
                    estimated_tokens,
                    available_window
                );
                warn!("{}", message);

                return ValidationResult::Warning {
                    estimated_tokens,
                    context_window: model_info.context_window,
                    utilization,
                    message,
                };
            }

            return ValidationResult::Ok {
                estimated_tokens,
                context_window: model_info.context_window,
                utilization,
            };
        }

        // Exceeded limit - try auto-truncation if enabled
        if self.config.auto_truncate {
            if let Some((truncated_messages, removed_count)) =
                self.truncate_messages(&request.messages, available_window)
            {
                let truncated_tokens = truncated_messages
                    .iter()
                    .map(|m| self.estimate_message_tokens(m))
                    .sum();

                info!(
                    original_tokens = estimated_tokens,
                    truncated_tokens = truncated_tokens,
                    removed_messages = removed_count,
                    "Auto-truncated conversation to fit context window"
                );

                return ValidationResult::Truncated {
                    original_tokens: estimated_tokens,
                    truncated_tokens,
                    removed_message_count: removed_count,
                    messages: truncated_messages,
                };
            }
        }

        // Cannot fit even with truncation
        let message = format!(
            "Context length exceeded: {} tokens used, {} available. Please start a new conversation.",
            estimated_tokens, available_window
        );

        ValidationResult::Exceeded {
            estimated_tokens,
            context_window: model_info.context_window,
            message,
        }
    }

    /// Truncate messages to fit within token limit
    ///
    /// Strategy:
    /// 1. Keep system message (if any)
    /// 2. Group messages into "conversation units" (tool_use + tool_results stay together)
    /// 3. Keep the most recent units until limit reached
    /// 4. Remove oldest units first
    fn truncate_messages(
        &self,
        messages: &[Message],
        max_tokens: u32,
    ) -> Option<(Vec<Message>, usize)> {
        if messages.is_empty() {
            return None;
        }

        // Separate system messages from others
        let (system_messages, other_messages): (Vec<_>, Vec<_>) = messages
            .iter()
            .cloned()
            .partition(|m| matches!(m.role, crate::api::Role::System));

        // Calculate tokens for system messages (always kept)
        let system_tokens: u32 = system_messages
            .iter()
            .map(|m| self.estimate_message_tokens(m))
            .sum();

        // If system messages alone exceed limit, we can't help
        if system_tokens >= max_tokens {
            return None;
        }

        let available_for_others = max_tokens - system_tokens;

        // Group messages into conversation units
        // A unit is either:
        // - An assistant message with tool_calls + all following tool result messages
        // - A regular user or assistant message without tool_calls
        let units = self.group_into_units(&other_messages);

        // Keep as many recent units as possible (from newest to oldest)
        let mut kept_units: Vec<&ConversationUnit> = Vec::new();
        let mut kept_tokens = 0u32;

        for unit in units.iter().rev() {
            if kept_tokens + unit.tokens <= available_for_others {
                kept_units.push(unit);
                kept_tokens += unit.tokens;
            } else {
                // Stop when we can't fit more
                break;
            }
        }

        // Reverse to restore chronological order
        kept_units.reverse();

        // Flatten units back to messages
        let kept_messages: Vec<Message> = kept_units
            .iter()
            .flat_map(|u| u.messages.clone())
            .collect();

        let removed_count = other_messages.len() - kept_messages.len();

        // Combine system messages with kept messages
        let mut result = system_messages;
        result.extend(kept_messages);

        // Only return if we actually removed something AND fit within limit
        if removed_count > 0 {
            let final_tokens: u32 = result.iter().map(|m| self.estimate_message_tokens(m)).sum();
            if final_tokens <= max_tokens {
                Some((result, removed_count))
            } else {
                None // Still over limit even after truncation
            }
        } else {
            None // No truncation needed/possible
        }
    }

    /// Group messages into conversation units that preserve tool_use/tool_result pairing
    fn group_into_units(&self, messages: &[Message]) -> Vec<ConversationUnit> {
        let mut units: Vec<ConversationUnit> = Vec::new();
        let mut i = 0;

        while i < messages.len() {
            let msg = &messages[i];

            // Check if this is an assistant message with tool calls
            if matches!(msg.role, crate::api::Role::Assistant) && !msg.tool_calls.is_empty() {
                // Start a new unit with the assistant message
                let mut unit_messages = vec![msg.clone()];
                let mut unit_tokens = self.estimate_message_tokens(msg);
                i += 1;

                // Collect all following tool result messages
                while i < messages.len() {
                    let next_msg = &messages[i];
                    if matches!(next_msg.role, crate::api::Role::Tool) {
                        unit_messages.push(next_msg.clone());
                        unit_tokens += self.estimate_message_tokens(next_msg);
                        i += 1;
                    } else {
                        break;
                    }
                }

                units.push(ConversationUnit {
                    messages: unit_messages,
                    tokens: unit_tokens,
                });
            } else {
                // Regular message - its own unit
                units.push(ConversationUnit {
                    messages: vec![msg.clone()],
                    tokens: self.estimate_message_tokens(msg),
                });
                i += 1;
            }
        }

        units
    }

    /// Validate and return an error if exceeded (for use in service)
    pub fn validate_or_error(
        &self,
        request: &CompletionRequest,
        model_info: &ModelInfo,
    ) -> LlmResult<ValidationResult> {
        let result = self.validate(request, model_info);

        match &result {
            ValidationResult::Exceeded {
                estimated_tokens,
                context_window,
                message,
            } => {
                warn!("{}", message);
                Err(LlmError::ContextLengthExceeded {
                    used: *estimated_tokens,
                    max: *context_window,
                })
            }
            _ => Ok(result),
        }
    }
}

/// Builder for ContextConfig
#[derive(Debug, Default)]
pub struct ContextConfigBuilder {
    config: ContextConfig,
}

impl ContextConfigBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set warning threshold (0.0-1.0)
    pub fn warning_threshold(mut self, threshold: f32) -> Self {
        self.config.warning_threshold = threshold.clamp(0.0, 1.0);
        self
    }

    /// Enable or disable auto-truncation
    pub fn auto_truncate(mut self, enabled: bool) -> Self {
        self.config.auto_truncate = enabled;
        self
    }

    /// Set tokens to reserve for response
    pub fn reserve_for_response(mut self, tokens: u32) -> Self {
        self.config.reserve_for_response = tokens;
        self
    }

    /// Set minimum messages to keep
    pub fn min_messages_to_keep(mut self, count: usize) -> Self {
        self.config.min_messages_to_keep = count;
        self
    }

    /// Set characters per token estimate
    pub fn chars_per_token(mut self, chars: f32) -> Self {
        self.config.chars_per_token = chars.max(1.0);
        self
    }

    /// Build the config
    pub fn build(self) -> ContextConfig {
        self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::Role;

    fn make_text_message(role: Role, text: &str) -> Message {
        Message {
            role,
            content: MessageContent::Text(text.to_string()),
            name: None,
            tool_call_id: None,
            tool_calls: vec![],
            cache_control: None,
        }
    }

    fn make_model_info(context_window: u32) -> ModelInfo {
        ModelInfo {
            id: "test-model".to_string(),
            name: "Test Model".to_string(),
            provider: "test".to_string(),
            context_window,
            supports_vision: false,
            supports_function_calling: true,
            supports_streaming: true,
        }
    }

    #[test]
    fn test_token_estimation() {
        let validator = ContextValidator::default();

        // ~100 chars = ~25 tokens
        let msg = make_text_message(Role::User, &"a".repeat(100));
        let tokens = validator.estimate_message_tokens(&msg);

        // Should be around 25 + overhead
        assert!(tokens >= 25 && tokens <= 35);
    }

    #[test]
    fn test_validation_ok() {
        let validator = ContextValidator::default();
        let model_info = make_model_info(200_000);

        let request = CompletionRequest {
            model: "test".to_string(),
            messages: vec![
                make_text_message(Role::System, "You are helpful."),
                make_text_message(Role::User, "Hello!"),
            ],
            temperature: None,
            max_tokens: None,
            top_p: None,
            stop: None,
            tools: None,
            tool_choice: None,
        };

        let result = validator.validate(&request, &model_info);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validation_warning() {
        let config = ContextConfigBuilder::new()
            .warning_threshold(0.5) // 50% threshold for testing
            .reserve_for_response(100)
            .build();
        let validator = ContextValidator::new(config);

        // Small context window to trigger warning
        let model_info = make_model_info(200);

        let request = CompletionRequest {
            model: "test".to_string(),
            messages: vec![
                make_text_message(Role::User, &"a".repeat(300)), // ~75 tokens
            ],
            temperature: None,
            max_tokens: None,
            top_p: None,
            stop: None,
            tools: None,
            tool_choice: None,
        };

        let result = validator.validate(&request, &model_info);
        assert!(matches!(result, ValidationResult::Warning { .. }));
    }

    #[test]
    fn test_validation_exceeded() {
        let config = ContextConfigBuilder::new()
            .auto_truncate(false)
            .reserve_for_response(50)
            .build();
        let validator = ContextValidator::new(config);

        // Very small context window
        let model_info = make_model_info(100);

        let request = CompletionRequest {
            model: "test".to_string(),
            messages: vec![
                make_text_message(Role::User, &"a".repeat(1000)), // ~250 tokens
            ],
            temperature: None,
            max_tokens: None,
            top_p: None,
            stop: None,
            tools: None,
            tool_choice: None,
        };

        let result = validator.validate(&request, &model_info);
        assert!(result.is_exceeded());
    }

    #[test]
    fn test_auto_truncation() {
        let config = ContextConfigBuilder::new()
            .auto_truncate(true)
            .reserve_for_response(50)
            .min_messages_to_keep(1)
            .build();
        let validator = ContextValidator::new(config);

        // Small context window
        let model_info = make_model_info(200);

        let request = CompletionRequest {
            model: "test".to_string(),
            messages: vec![
                make_text_message(Role::System, "System prompt."),
                make_text_message(Role::User, &"Old message. ".repeat(50)),
                make_text_message(Role::Assistant, &"Old response. ".repeat(50)),
                make_text_message(Role::User, "Recent message."),
            ],
            temperature: None,
            max_tokens: None,
            top_p: None,
            stop: None,
            tools: None,
            tool_choice: None,
        };

        let result = validator.validate(&request, &model_info);

        // Should be truncated
        assert!(result.is_truncated());

        if let ValidationResult::Truncated {
            messages,
            removed_message_count,
            ..
        } = result
        {
            // System message should be kept
            assert!(messages
                .iter()
                .any(|m| matches!(m.role, Role::System)));
            // Some messages should be removed
            assert!(removed_message_count > 0);
            // Total messages should be less than original
            assert!(messages.len() < 4);
        }
    }

    #[test]
    fn test_validate_or_error() {
        let config = ContextConfigBuilder::new()
            .auto_truncate(false)
            .reserve_for_response(50)
            .build();
        let validator = ContextValidator::new(config);

        let model_info = make_model_info(100);

        let request = CompletionRequest {
            model: "test".to_string(),
            messages: vec![make_text_message(Role::User, &"a".repeat(1000))],
            temperature: None,
            max_tokens: None,
            top_p: None,
            stop: None,
            tools: None,
            tool_choice: None,
        };

        let result = validator.validate_or_error(&request, &model_info);
        assert!(result.is_err());

        if let Err(LlmError::ContextLengthExceeded { .. }) = result {
            // Expected
        } else {
            panic!("Expected ContextLengthExceeded error");
        }
    }
}
