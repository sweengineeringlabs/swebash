/// Integration tests for tool calling functionality

use swebash_ai::core::tools::create_tool_registry;
use swebash_ai::config::{AiConfig, ToolConfig};

#[tokio::test]
async fn test_filesystem_tool_read_file() {
    let mut config = AiConfig::from_env();
    config.tools = ToolConfig {
        enable_fs: true,
        enable_exec: false,
        enable_web: false,
        ..Default::default()
    };

    let registry = create_tool_registry(&config);

    // Test reading this test file itself
    let args = r#"{"operation": "read", "path": "tests/tools_test.rs"}"#;
    let result = registry.execute("filesystem", args).await;

    assert!(result.is_ok());
    let content = result.unwrap();
    assert!(content.contains("test_filesystem_tool_read_file"));
}

#[tokio::test]
async fn test_filesystem_tool_list_directory() {
    let mut config = AiConfig::from_env();
    config.tools.enable_fs = true;
    config.tools.enable_exec = false;
    config.tools.enable_web = false;

    let registry = create_tool_registry(&config);

    // List the src directory
    let args = r#"{"operation": "list", "path": "src"}"#;
    let result = registry.execute("filesystem", args).await;

    assert!(result.is_ok());
    let content = result.unwrap();
    assert!(content.contains("entries"));
}

#[tokio::test]
async fn test_filesystem_tool_path_traversal_blocked() {
    let mut config = AiConfig::from_env();
    config.tools.enable_fs = true;
    config.tools.enable_exec = false;
    config.tools.enable_web = false;

    let registry = create_tool_registry(&config);

    // Try to read /etc/passwd (should be blocked)
    let args = r#"{"operation": "read", "path": "/etc/passwd"}"#;
    let result = registry.execute("filesystem", args).await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_command_executor_safe_command() {
    let mut config = AiConfig::from_env();
    config.tools.enable_fs = false;
    config.tools.enable_exec = true;
    config.tools.enable_web = false;

    let registry = create_tool_registry(&config);

    // Execute safe command
    let args = r#"{"command": "echo 'hello world'"}"#;
    let result = registry.execute("execute_command", args).await;

    assert!(result.is_ok());
    let content = result.unwrap();
    assert!(content.contains("hello world"));
}

#[tokio::test]
async fn test_command_executor_dangerous_command_blocked() {
    let mut config = AiConfig::from_env();
    config.tools.enable_fs = false;
    config.tools.enable_exec = true;
    config.tools.enable_web = false;

    let registry = create_tool_registry(&config);

    // Try dangerous command (should be blocked)
    let args = r#"{"command": "rm -rf /"}"#;
    let result = registry.execute("execute_command", args).await;

    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("Dangerous") || err_msg.contains("Permission denied"));
}

#[tokio::test]
async fn test_command_executor_timeout() {
    let mut config = AiConfig::from_env();
    config.tools.enable_fs = false;
    config.tools.enable_exec = true;
    config.tools.enable_web = false;
    config.tools.exec_timeout = 2; // 2 second timeout

    let registry = create_tool_registry(&config);

    // Command that sleeps longer than timeout
    let args = r#"{"command": "sleep 5", "timeout_seconds": 2}"#;
    let result = registry.execute("execute_command", args).await;

    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("timeout") || err_msg.contains("timed out"));
}

#[tokio::test]
async fn test_web_search_tool() {
    let mut config = AiConfig::from_env();
    config.tools.enable_fs = false;
    config.tools.enable_exec = false;
    config.tools.enable_web = true;

    let registry = create_tool_registry(&config);

    // Search for Rust programming language
    let args = r#"{"query": "rust programming language", "num_results": 3}"#;
    let result = registry.execute("web_search", args).await;

    // Note: This test requires internet connection and may fail if DuckDuckGo is down
    if result.is_ok() {
        let content = result.unwrap();
        assert!(content.contains("results") || content.contains("query"));
    } else {
        // Log warning but don't fail test if network is unavailable
        eprintln!("Warning: Web search test skipped (network unavailable)");
    }
}

#[tokio::test]
async fn test_tool_registry_definitions() {
    let config = AiConfig::from_env();
    let registry = create_tool_registry(&config);

    let definitions = registry.definitions();

    // Check that we have all three tools
    assert!(definitions.len() >= 3, "Expected at least 3 tools");

    let tool_names: Vec<String> = definitions.iter()
        .map(|d| d.name.clone())
        .collect();

    assert!(tool_names.contains(&"filesystem".to_string()));
    assert!(tool_names.contains(&"execute_command".to_string()));
    assert!(tool_names.contains(&"web_search".to_string()));
}

#[tokio::test]
async fn test_tool_registry_not_found() {
    let config = AiConfig::from_env();
    let registry = create_tool_registry(&config);

    // Try to execute non-existent tool
    let result = registry.execute("nonexistent_tool", "{}").await;

    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("not found") || err_msg.contains("Not found"));
}

#[tokio::test]
async fn test_selective_tool_enablement() {
    let mut config = AiConfig::from_env();
    config.tools.enable_fs = true;
    config.tools.enable_exec = false;  // Disable exec
    config.tools.enable_web = true;

    let registry = create_tool_registry(&config);
    let definitions = registry.definitions();

    let tool_names: Vec<String> = definitions.iter()
        .map(|d| d.name.clone())
        .collect();

    // Should have fs and web, but not exec
    assert!(tool_names.contains(&"filesystem".to_string()));
    assert!(!tool_names.contains(&"execute_command".to_string()));
    assert!(tool_names.contains(&"web_search".to_string()));
}
