# swebash

> **TLDR:** A WASM-based Unix-like shell with AI-powered agents. See [docs](docs/README.md) for details.

## Quick Start

```bash
# One-time setup
./sbh setup && source ~/.bashrc

# Build and run
./sbh build && ./sbh run
```

## Features

- WASM shell engine (`no_std`, `wasm32-unknown-unknown`)
- Configurable workspace sandbox (path-based access control, TOML config)
- Multi-agent AI chat (Anthropic, OpenAI, Gemini)
- Agent auto-detection and switching
- Interactive readline with history and arrow-key navigation
- Tool-aware agents (fs, exec, web)

## Documentation

| Resource | Description |
|----------|-------------|
| [Overview](docs/README.md) | Central documentation hub |
| [Architecture](docs/3-design/architecture.md) | System design (three-crate + SEA) |
| [Setup Guide](docs/4-development/setup_guide.md) | Dev environment setup |
| [Developer Guide](docs/4-development/developer_guide.md) | Day-to-day workflow and conventions |
| [Installation](docs/7-operation/installation.md) | User install instructions |

## Workspace Crates

| Crate | Path | Description |
|-------|------|-------------|
| `engine` | `features/shell/engine` | WASM shell engine (`no_std`) |
| `swebash` | `features/shell/host` | Native REPL + WASM runtime |
| `swebash-readline` | `features/shell/readline` | Terminal line-editing + history |
| `swebash-ai` | `features/ai` | LLM integration (SEA pattern) |

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for branch naming, commit conventions, and PR process.

## Security

See [SECURITY.md](SECURITY.md) for vulnerability reporting.

## License

MIT — see [LICENSE](LICENSE).

## Support

See [SUPPORT.md](SUPPORT.md) for help channels.
