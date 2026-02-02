/// L1 SPI: Tool-aware chat engine provider
///
/// Implementation of ChatEngine that adds tool calling capabilities
/// while maintaining all standard engine features (memory, context, tokens).

use std::sync::Arc;

use async_trait::async_trait;
use parking_lot::RwLock;
use tracing::{debug, info};

use agent_controller::engine::Engine;
use agent_controller::types::{EngineMetrics, EngineState};
use chat_engine::{
    ChatConfig, ChatEngine, ChatMessage, ChatResponse, ChatRole, ChatError, ChatResult,
    ConversationState, ContextError, ContextWindow, InMemoryManager, MemoryManager, TokenUsage,
};
use llm_provider::{
    CompletionRequest, FinishReason, LlmService, Message, MessageContent, Role,
    ToolCall, ToolChoice,
};
use react::{AgentEvent, AgentEventSender, SessionStatus};

use crate::core::tools::ToolRegistry;

/// Tool-aware implementation of ChatEngine SPI.
///
/// This provider implements the full ChatEngine interface with tool calling support.
/// It maintains all standard capabilities (memory, context window, token tracking)
/// while adding the ability to execute tools in a loop until the LLM returns
/// a final response.
pub struct ToolAwareChatEngine {
    // Standard ChatEngine state
    conversation_id: String,
    llm: Arc<dyn LlmService>,
    memory: Arc<InMemoryManager>,
    context: RwLock<ContextWindow>,
    config: RwLock<ChatConfig>,
    state: RwLock<EngineState>,
    conversation_state: RwLock<ConversationState>,
    metrics: RwLock<ToolAwareMetrics>,

    // Tool-specific additions
    tools: Arc<ToolRegistry>,
}

#[derive(Debug, Clone, Default)]
struct ToolAwareMetrics {
    messages_sent: u64,
    messages_received: u64,
    total_input_tokens: u64,
    total_output_tokens: u64,
    errors: u64,
    // Tool-specific metrics
    tool_calls: u64,
    tool_successes: u64,
    tool_failures: u64,
}

impl ToolAwareChatEngine {
    pub fn new(
        llm: Arc<dyn LlmService>,
        config: ChatConfig,
        tools: ToolRegistry,
    ) -> Self {
        let max_context_tokens = 128_000;
        let reserved_tokens = config.max_tokens as usize + 1024;

        Self {
            conversation_id: uuid::Uuid::new_v4().to_string(),
            llm,
            memory: Arc::new(InMemoryManager::new(config.max_history)),
            context: RwLock::new(ContextWindow::with_reserved(
                max_context_tokens,
                reserved_tokens,
            )),
            config: RwLock::new(config),
            state: RwLock::new(EngineState::Idle),
            conversation_state: RwLock::new(ConversationState::Idle),
            metrics: RwLock::new(ToolAwareMetrics::default()),
            tools: Arc::new(tools),
        }
    }

    // ── Internal helpers ──

    fn build_messages(&self) -> Vec<Message> {
        let context = self.context.read();
        context.messages().cloned().collect()
    }

    fn to_llm_message(msg: &ChatMessage) -> Message {
        Message {
            role: match msg.role {
                ChatRole::System => Role::System,
                ChatRole::User => Role::User,
                ChatRole::Assistant => Role::Assistant,
            },
            content: MessageContent::Text(msg.content.clone()),
            name: None,
            tool_call_id: None,
            tool_calls: vec![],
            cache_control: None,
        }
    }

    fn add_to_context(&self, message: Message) -> ChatResult<()> {
        let mut context = self.context.write();

        match context.add_message(message.clone()) {
            Ok(()) => Ok(()),
            Err(ContextError::MessageTooLarge { .. }) => {
                Err(ChatError::Config("Message too large for context window".to_string()))
            }
            Err(ContextError::WindowExceeded { .. }) => {
                let config = self.config.read();
                let target_tokens = context.available_tokens() / 2;
                context.truncate_to_fit(target_tokens);

                if config.enable_summarization {
                    debug!("Context truncated, older messages removed");
                }

                context
                    .add_message(message)
                    .map_err(|e| ChatError::Config(format!(
                        "Failed to add message after truncation: {}", e
                    )))
            }
            Err(e) => Err(ChatError::Config(format!("Context error: {}", e))),
        }
    }

    fn set_state(&self, state: EngineState) {
        *self.state.write() = state;
    }

    fn set_conversation_state(&self, state: ConversationState) {
        *self.conversation_state.write() = state;
    }

    // ── Tool calling logic ──

    async fn execute_tool_calls(
        &self,
        tool_calls: &[ToolCall],
        _events: &AgentEventSender,
    ) -> Vec<(String, String)> {
        let mut results = Vec::new();

        for tool_call in tool_calls {
            self.metrics.write().tool_calls += 1;

            debug!(
                "Executing tool: {} with args: {}",
                tool_call.name, tool_call.arguments
            );

            // Execute tool
            let result = self.tools
                .execute(&tool_call.name, &tool_call.arguments)
                .await;

            let result_content = match result {
                Ok(content) => {
                    self.metrics.write().tool_successes += 1;
                    debug!("Tool {} succeeded", tool_call.name);
                    content
                }
                Err(e) => {
                    self.metrics.write().tool_failures += 1;
                    debug!("Tool {} failed: {}", tool_call.name, e);
                    format!(r#"{{"error": true, "message": "{}"}}"#, e)
                }
            };

            results.push((tool_call.id.clone(), result_content));
        }

        results
    }

    async fn send_internal(
        &self,
        message: ChatMessage,
        events: AgentEventSender,
    ) -> ChatResult<ChatResponse> {
        self.set_state(EngineState::Running);
        self.set_conversation_state(ConversationState::Processing);

        let _ = events.send(AgentEvent::StatusChange {
            status: SessionStatus::Running,
            message: Some("Processing message".to_string()),
        }).await;

        // Initialize context with system prompt if first message
        if self.memory.message_count() == 0 {
            if let Some(ref system_prompt) = self.config.read().system_prompt {
                let system_msg = Message {
                    role: Role::System,
                    content: MessageContent::Text(system_prompt.clone()),
                    name: None,
                    tool_call_id: None,
                    tool_calls: vec![],
                    cache_control: None,
                };
                self.add_to_context(system_msg)?;
            }
        }

        // Add user message to context and memory
        let user_msg = Self::to_llm_message(&message);
        self.add_to_context(user_msg)?;
        self.memory.add_message(message).await?;
        self.metrics.write().messages_sent += 1;

        // Tool calling loop
        let mut iteration = 0;
        let mut cumulative_usage = TokenUsage {
            input_tokens: 0,
            output_tokens: 0,
            total_tokens: 0,
        };

        loop {
            if iteration >= self.tools.config.max_iterations {
                self.metrics.write().errors += 1;
                return Err(ChatError::Config(
                    "Max tool iterations reached".to_string()
                ));
            }

            // Build request with tools
            let messages = self.build_messages();
            let (model, temperature, max_tokens) = {
                let config = self.config.read();
                (config.model.clone(), config.temperature, config.max_tokens)
            };

            let request = CompletionRequest {
                model,
                messages,
                temperature: Some(temperature),
                max_tokens: Some(max_tokens),
                top_p: None,
                stop: None,
                tools: Some(self.tools.definitions()),
                tool_choice: Some(ToolChoice::Auto),
            };

            debug!("Sending LLM request with {} tools", self.tools.definitions().len());

            // Call LLM
            let response = self.llm.complete(request).await
                .map_err(|e| ChatError::Llm(e.to_string()))?;

            self.metrics.write().messages_received += 1;

            // Track token usage (scope to ensure guard is dropped)
            {
                let usage = &response.usage;
                let mut m = self.metrics.write();
                m.total_input_tokens += usage.prompt_tokens as u64;
                m.total_output_tokens += usage.completion_tokens as u64;

                cumulative_usage.input_tokens += usage.prompt_tokens as u64;
                cumulative_usage.output_tokens += usage.completion_tokens as u64;
                cumulative_usage.total_tokens += usage.total_tokens as u64;
            }

            // Check for tool calls
            if response.tool_calls.is_empty() || response.finish_reason == FinishReason::Stop {
                // No tool calls - final response
                let content = response.content.unwrap_or_default();

                debug!("LLM returned final response (no tool calls)");

                let assistant_msg = Message {
                    role: Role::Assistant,
                    content: MessageContent::Text(content.clone()),
                    name: None,
                    tool_call_id: None,
                    tool_calls: vec![],
                    cache_control: None,
                };
                self.add_to_context(assistant_msg)?;

                let assistant_chat_msg = ChatMessage::assistant(&content);
                self.memory.add_message(assistant_chat_msg).await?;

                self.set_state(EngineState::Idle);
                self.set_conversation_state(ConversationState::Idle);

                let _ = events.send(AgentEvent::StatusChange {
                    status: SessionStatus::Completed,
                    message: None,
                }).await;

                return Ok(ChatResponse {
                    message: ChatMessage::assistant(&content),
                    complete: true,
                    usage: Some(cumulative_usage),
                    latency: None,
                });
            }

            // Execute tool calls
            info!("Executing {} tool calls", response.tool_calls.len());
            let tool_results = self.execute_tool_calls(&response.tool_calls, &events).await;

            // Add tool results to context
            for (tool_call_id, content) in tool_results {
                let tool_msg = Message {
                    role: Role::Tool,
                    content: MessageContent::Text(content),
                    name: None,
                    tool_call_id: Some(tool_call_id),
                    tool_calls: vec![],
                    cache_control: None,
                };
                self.add_to_context(tool_msg)?;
            }

            iteration += 1;
        }
    }
}

// ── Implement Engine trait ──

#[async_trait]
impl Engine for ToolAwareChatEngine {
    fn name(&self) -> &str {
        "tool-aware-chat"
    }

    fn state(&self) -> EngineState {
        *self.state.read()
    }

    fn pause(&self) {
        self.set_state(EngineState::Paused);
        self.set_conversation_state(ConversationState::Paused);
    }

    fn resume(&self) {
        self.set_state(EngineState::Running);
        self.set_conversation_state(ConversationState::Idle);
    }

    fn abort(&self) {
        self.set_state(EngineState::Error);
        self.set_conversation_state(ConversationState::Error);
    }

    fn reset(&self) {
        self.set_state(EngineState::Idle);
        self.set_conversation_state(ConversationState::Idle);
        self.context.write().clear();
        self.metrics.write().errors = 0;
    }

    fn metrics(&self) -> EngineMetrics {
        let m = self.metrics.read();
        EngineMetrics {
            turns: m.messages_sent as u32,
            llm_calls: m.messages_received as u32,
            input_tokens: m.total_input_tokens,
            output_tokens: m.total_output_tokens,
            errors: m.errors as u32,
            tool_calls: m.tool_calls as u32,
            ..Default::default()
        }
    }
}

// ── Implement ChatEngine trait (SPI) ──

#[async_trait]
impl ChatEngine for ToolAwareChatEngine {
    async fn send(
        &self,
        message: ChatMessage,
        events: AgentEventSender,
    ) -> ChatResult<ChatResponse> {
        self.send_internal(message, events).await
    }

    async fn send_streaming(
        &self,
        message: ChatMessage,
        events: AgentEventSender,
    ) -> ChatResult<ChatResponse> {
        // TODO: Implement streaming with tool support
        // For now, delegate to non-streaming
        self.send_internal(message, events).await
    }

    fn conversation_state(&self) -> ConversationState {
        *self.conversation_state.read()
    }

    fn memory(&self) -> Arc<dyn MemoryManager> {
        self.memory.clone()
    }

    fn config(&self) -> ChatConfig {
        self.config.read().clone()
    }

    fn set_config(&mut self, config: ChatConfig) {
        *self.config.write() = config;
    }

    async fn new_conversation(&self) -> ChatResult<()> {
        self.memory.clear().await?;
        self.context.write().clear();
        self.set_state(EngineState::Idle);
        self.set_conversation_state(ConversationState::Idle);
        Ok(())
    }

    fn conversation_id(&self) -> &str {
        &self.conversation_id
    }

    fn message_count(&self) -> usize {
        self.memory.message_count()
    }
}
