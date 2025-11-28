# Codex CLI - New Developer Onboarding Guide

Welcome to the Codex CLI project! This comprehensive guide will help you understand the codebase, set up your development environment, and start contributing effectively.

## Table of Contents

1. [Project Overview](#1-project-overview)
2. [Repository Structure](#2-repository-structure)
3. [Getting Started](#3-getting-started)
4. [Key Components](#4-key-components)
5. [Development Workflow](#5-development-workflow)
6. [Architecture Decisions](#6-architecture-decisions)
7. [Common Tasks](#7-common-tasks)
8. [Potential Gotchas](#8-potential-gotchas)
9. [Documentation and Resources](#9-documentation-and-resources)
10. [Onboarding Checklist](#10-onboarding-checklist)

---

## 1. Project Overview

### What is Codex CLI?

Codex CLI is a coding agent from OpenAI that runs locally on your computer. It provides an interactive terminal UI (TUI) for AI-assisted coding, with capabilities including:

- Interactive chat-based coding assistance
- Automated code generation and refactoring
- Shell command execution with safety controls
- Git operations and PR creation
- MCP (Model Context Protocol) server integration

### Tech Stack

| Category | Technology | Purpose |
|----------|------------|---------|
| **Primary Language** | Rust 1.90.0 (Edition 2024) | Main CLI implementation |
| **Secondary Language** | TypeScript 5.9 | SDK and tooling |
| **Async Runtime** | Tokio | Asynchronous execution |
| **TUI Framework** | Ratatui 0.29 (custom fork) | Terminal user interface |
| **Terminal** | Crossterm 0.28 (custom fork) | Cross-platform terminal handling |
| **HTTP Client** | Reqwest 0.12 | API communication |
| **Serialization** | Serde/serde_json | JSON handling |
| **Testing** | cargo-nextest, insta | Fast tests, snapshot testing |
| **Build Tool** | Cargo, pnpm | Rust and TS package management |

### Architecture Pattern

The project follows a **modular monorepo architecture**:

- **Cargo Workspace**: 46 interdependent Rust crates
- **pnpm Workspace**: 3 TypeScript packages
- **Layered Design**: CLI → TUI → Core → Protocol layers
- **Event-driven**: Streaming responses with SSE

### Key Dependencies

| Dependency | Purpose |
|------------|---------|
| `ratatui` | Terminal UI rendering |
| `tokio` | Async runtime |
| `reqwest` | HTTP client for API calls |
| `rmcp` | Model Context Protocol SDK |
| `tree-sitter` | Code parsing |
| `landlock/seatbelt` | Platform sandboxing |
| `insta` | Snapshot testing |

---

## 2. Repository Structure

```
opencodex/
├── codex-rs/                 # Primary Rust implementation (main codebase)
│   ├── cli/                  # CLI entry point and subcommands
│   ├── core/                 # Core business logic (68+ modules)
│   ├── tui/                  # Terminal UI implementation
│   ├── exec/                 # Non-interactive execution mode
│   ├── mcp-server/           # MCP server implementation
│   ├── execpolicy/           # Command execution policies
│   ├── linux-sandbox/        # Linux sandboxing (Landlock)
│   ├── process-hardening/    # Process security hardening
│   ├── login/                # Authentication management
│   ├── keyring-store/        # Credential storage
│   ├── chatgpt/              # ChatGPT API client
│   ├── backend-client/       # Backend API communication
│   ├── protocol/             # Wire protocol definitions
│   ├── file-search/          # Fast file search
│   ├── utils/                # Utility crates (git, cache, image, etc.)
│   └── Cargo.toml            # Workspace definition
│
├── sdk/typescript/           # TypeScript SDK for programmatic access
│   ├── src/                  # SDK source code
│   └── package.json          # npm package config
│
├── shell-tool-mcp/           # MCP server for shell tools
│   ├── bin/                  # Entry point scripts
│   └── package.json          # npm package config
│
├── codex-cli/                # Legacy TS CLI wrapper (deprecated)
│
├── docs/                     # User documentation (22 markdown files)
│   ├── getting-started.md    # Usage guide
│   ├── config.md             # Configuration reference
│   ├── sandbox.md            # Security/sandbox docs
│   ├── contributing.md       # Contribution guidelines
│   └── ...
│
├── scripts/                  # Build and utility scripts
│   ├── asciicheck.py         # ASCII validation
│   ├── debug-codex.sh        # Debug helper
│   └── ...
│
├── .github/workflows/        # CI/CD workflows
│   ├── rust-ci.yml           # Rust CI pipeline
│   ├── rust-release.yml      # Release automation
│   ├── sdk.yml               # TypeScript SDK CI
│   └── ...
│
├── package.json              # Root monorepo config
├── pnpm-workspace.yaml       # pnpm workspace definition
└── README.md                 # Project overview
```

### Key Directories Explained

| Directory | Purpose |
|-----------|---------|
| `codex-rs/core/` | Heart of the application - contains all business logic |
| `codex-rs/tui/` | Full-screen terminal interface with chat UI |
| `codex-rs/cli/` | Command dispatcher for tui, exec, mcp-server subcommands |
| `codex-rs/exec/` | Non-interactive mode for automation/CI |
| `codex-rs/mcp-server/` | Makes Codex available as an MCP server |
| `sdk/typescript/` | Programmatic access to Codex from TypeScript |

---

## 3. Getting Started

### Prerequisites

| Requirement | Version | Notes |
|------------|---------|-------|
| **Operating System** | macOS 12+, Ubuntu 20.04+, Windows 11 (WSL2) | Native Windows not supported |
| **Rust** | 1.90.0 | Specified in `rust-toolchain.toml` |
| **Node.js** | >= 22 | For TypeScript tooling |
| **pnpm** | >= 10.8.1 | Package manager |
| **Git** | 2.23+ | For PR helpers |
| **RAM** | 4GB minimum | 8GB recommended |

### Environment Setup

#### 1. Clone the Repository

```bash
git clone https://github.com/openai/codex.git
cd codex
```

#### 2. Install Rust Toolchain

```bash
# Install rustup if not present
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source "$HOME/.cargo/env"

# Navigate to Rust workspace
cd codex-rs

# Install required components (rustup will auto-install the correct version)
rustup component add rustfmt
rustup component add clippy
```

#### 3. Install Node.js Dependencies (Optional - for TypeScript work)

```bash
# From repository root
cd ..
corepack enable
pnpm install
```

#### 4. Install Development Tools

```bash
# Install just (command runner)
cargo install just

# Install cargo-nextest (fast test runner)
cargo install cargo-nextest

# Install cargo-insta (snapshot testing)
cargo install cargo-insta
```

### Building the Project

```bash
cd codex-rs

# Development build
cargo build

# Release build (optimized)
cargo build --release

# Build specific crate
cargo build -p codex-core
```

### Running Locally

```bash
cd codex-rs

# Using just (recommended)
just codex                    # Run Codex TUI
just codex "explain this"     # Run with initial prompt
just tui                      # Run TUI directly
just exec "fix lint errors"   # Run non-interactive mode

# Using cargo directly
cargo run --bin codex -- "explain this codebase"
cargo run --bin codex -- exec "run tests"
```

### Running Tests

```bash
cd codex-rs

# Using just (recommended - uses nextest)
just test

# Using cargo nextest directly
cargo nextest run --no-fail-fast

# Run tests for specific crate
cargo nextest run -p codex-core

# Run with standard cargo test
cargo test

# Update snapshots (when tests fail due to expected changes)
cargo insta review
```

### Code Quality Checks

```bash
cd codex-rs

# Format code
just fmt
# Or: cargo fmt -- --config imports_granularity=Item

# Run clippy lints
just clippy
# Or: cargo clippy --all-features --tests

# Apply clippy fixes
just fix -p codex-core
```

---

## 4. Key Components

### Entry Points

| File | Binary | Purpose |
|------|--------|---------|
| `codex-rs/cli/src/main.rs` | `codex` | Main CLI dispatcher |
| `codex-rs/tui/src/lib.rs` | - | TUI application logic |
| `codex-rs/exec/src/lib.rs` | - | Non-interactive execution |
| `codex-rs/mcp-server/src/lib.rs` | - | MCP server logic |
| `sdk/typescript/src/index.ts` | - | TypeScript SDK entry |

### Core Business Logic (`codex-rs/core/`)

The `core` crate contains the main application logic:

| Module | Purpose |
|--------|---------|
| `codex.rs` (121KB) | Main state machine and orchestration |
| `client.rs` (19KB) | API client logic |
| `auth.rs` (42KB) | Authentication handling |
| `git_info.rs` (39KB) | Git context extraction |
| `parse_command.rs` (57KB) | Shell command parsing |
| `mcp_connection_manager.rs` (39KB) | MCP server connections |
| `sandbox.rs` | Sandbox policy management |
| `tools/` | Tool implementations (shell, git, etc.) |

### TUI Layer (`codex-rs/tui/`)

Ratatui-based terminal interface:

- **52 snapshot tests** for UI verification
- Live markdown rendering
- Diff visualization
- Chat history management
- Input handling with fuzzy file search

### Configuration

User configuration is stored in `~/.codex/config.toml`:

```toml
# Model selection
model = "gpt-5.1"
model_provider = "openai"

# Features
[features]
web_search_request = true
view_image_tool = true

# Model providers
[model_providers.openai]
name = "OpenAI"
base_url = "https://api.openai.com/v1"
env_key = "OPENAI_API_KEY"
```

### Authentication

Credentials stored in `~/.codex/auth.json`. Methods:

1. **ChatGPT Login** (recommended): `codex login`
2. **API Key**: `printenv OPENAI_API_KEY | codex login --with-api-key`

---

## 5. Development Workflow

### Git Branch Naming

- Feature branches: `feat/descriptive-name`
- Bug fixes: `fix/issue-description`
- Refactoring: `refactor/what-changed`

### Creating a New Feature

1. **Open an issue first** - Get approval before major work
2. **Create a topic branch** from `main`
3. **Make focused changes** - Keep PRs small and atomic
4. **Add tests** - Every feature needs test coverage
5. **Update docs** - If user-facing behavior changes
6. **Run all checks locally**:
   ```bash
   cargo test && cargo clippy --tests && cargo fmt -- --config imports_granularity=Item
   ```
7. **Open a PR** with description: What? Why? How?

### Testing Requirements

- Every new feature needs test coverage
- Snapshot tests for UI changes (use `cargo insta review`)
- Integration tests in `tests/` directories
- Mock responses using `ResponseMock` and SSE builders

### Code Style

- Format with `cargo fmt -- --config imports_granularity=Item`
- Follow clippy suggestions (many lints are set to `deny`)
- Keep commits atomic and passing
- No `.expect()` or `.unwrap()` (use `deny` lint)

### PR Process

1. Fill in PR template (What? Why? How?)
2. Link to related issue
3. Run all checks locally
4. Mark as "Ready for Review" when complete
5. Sign the CLA by commenting: `I have read the CLA Document and I hereby sign the CLA`

### CI/CD Pipeline

The project uses GitHub Actions for:

- **rust-ci.yml**: Multi-platform builds and tests
- **rust-release.yml**: Automated releases on `rust-v*` tags
- **sdk.yml**: TypeScript SDK CI
- **cargo-deny.yml**: Dependency audits

Targets built:
- macOS: arm64, x86_64
- Linux: x86_64 (gnu/musl), arm64 (gnu/musl)
- Windows: x86_64, arm64

---

## 6. Architecture Decisions

### Design Patterns

| Pattern | Usage |
|---------|-------|
| **Event-driven** | Streaming SSE responses from API |
| **Layered architecture** | CLI → TUI → Core → Protocol |
| **State machine** | Main codex loop in `codex.rs` |
| **Dependency injection** | Test mocking via traits |
| **Builder pattern** | Configuration and request building |

### State Management

- TUI state managed in `tui` crate
- Core state via `Codex` struct in `core/codex.rs`
- Configuration via `Config` struct loaded from TOML

### Error Handling

- `anyhow::Result` for application errors
- `thiserror` for custom error types
- No panics in production code (`.expect()` and `.unwrap()` denied)

### Security Measures

| Platform | Sandbox Technology |
|----------|-------------------|
| macOS | Seatbelt (native sandbox) |
| Linux | Landlock LSM |
| Windows | Windows Sandbox (experimental) |
| All | seccomp rule enforcement |

Execution policies (`execpolicy`) control what commands can run.

### Performance Optimizations

- Release builds use fat LTO (`lto = "fat"`)
- Single codegen unit for better optimization
- Symbol stripping for smaller binaries
- Async I/O throughout with Tokio

---

## 7. Common Tasks

### Adding a New CLI Subcommand

1. Add command definition in `codex-rs/cli/src/main.rs`
2. Create implementation in appropriate crate
3. Add tests
4. Update `--help` output

### Adding a New Tool

1. Define tool in `codex-rs/core/src/tools/`
2. Register in tool dispatcher
3. Add sandboxing rules if needed
4. Write tests with mock responses

### Adding a New API Endpoint Integration

1. Update relevant client in `codex-rs/backend-client/` or `codex-rs/chatgpt/`
2. Add request/response types in `codex-rs/protocol/`
3. Handle in core logic
4. Add mock tests

### Adding a UI Component

1. Create widget in `codex-rs/tui/src/`
2. Add snapshot tests
3. Run `cargo insta review` to approve snapshots

### Running a Specific Test

```bash
# Run specific test
cargo nextest run test_name

# Run tests in specific crate
cargo nextest run -p codex-core

# Run with output
cargo nextest run -- --nocapture
```

### Debugging

```bash
# Enable debug output
RUST_BACKTRACE=1 cargo run --bin codex

# Use debug script
./scripts/debug-codex.sh

# Enable tracing
CODEX_LOG=debug cargo run --bin codex
```

### Updating Dependencies

```bash
# Update Cargo.lock
cargo update

# Update specific dependency
cargo update -p package-name

# Check for security issues
cargo deny check
```

---

## 8. Potential Gotchas

### Non-Obvious Configurations

- **Rust toolchain**: Version 1.90.0 is auto-installed via `rust-toolchain.toml`
- **Patched dependencies**: `ratatui`, `crossterm`, and `rmcp` use custom forks (see `Cargo.toml` `[patch.crates-io]`)
- **pnpm workspace**: TypeScript packages need `pnpm install` from root

### Required Environment Variables

| Variable | Purpose | When Needed |
|----------|---------|-------------|
| `OPENAI_API_KEY` | API authentication | When using API key auth |
| `CODEX_HOME` | Config directory | Optional, defaults to `~/.codex` |
| `CODEX_LOG` | Logging level | Debug (`debug`, `trace`) |
| `RUST_BACKTRACE` | Stack traces | Debugging (`1` or `full`) |

### External Service Dependencies

- **OpenAI API**: Required for model inference
- **ChatGPT Login Server**: `localhost:1455` during auth
- **Git**: For repository operations

### Known Issues / Workarounds

1. **Homebrew upgrades**: See FAQ for `brew upgrade codex` issues
2. **Headless login**: Requires SSH port forwarding or credential copying
3. **Windows**: Only supported via WSL2

### Performance Considerations

- Initial builds are slow (46 crates)
- Use `cargo-nextest` for faster tests
- Release builds take longer (LTO enabled)
- Use `sccache` for faster incremental builds in CI

### Areas of Technical Debt

- `codex-cli/` is deprecated (legacy TypeScript wrapper)
- `execpolicy-legacy/` crate for backwards compatibility
- Some experimental features behind feature flags

---

## 9. Documentation and Resources

### Internal Documentation

| Document | Location | Purpose |
|----------|----------|---------|
| README | `README.md` | Project overview |
| Getting Started | `docs/getting-started.md` | Usage guide |
| Configuration | `docs/config.md` | Full config reference |
| Example Config | `docs/example-config.md` | Real-world examples |
| Sandbox | `docs/sandbox.md` | Security controls |
| Contributing | `docs/contributing.md` | How to contribute |
| FAQ | `docs/faq.md` | Common questions |
| Install | `docs/install.md` | Installation options |

### API Documentation

```bash
# Generate and view Rust docs
cd codex-rs
cargo doc --open
```

### External Resources

- [OpenAI Platform Docs](https://platform.openai.com/docs/)
- [Model Context Protocol](https://modelcontextprotocol.io/)
- [Ratatui Documentation](https://ratatui.rs/)
- [AGENTS.md Specification](https://agents.md/)

### Code Examples

- TypeScript SDK: `sdk/typescript/README.md`
- Configuration: `docs/example-config.md`
- Shell integration: `docs/getting-started.md`

---

## 10. Onboarding Checklist

Use this checklist to track your onboarding progress:

### Week 1: Environment Setup

- [ ] Clone the repository
- [ ] Install Rust 1.90.0 (via rustup)
- [ ] Install Node.js 22+ and pnpm 10.8.1+
- [ ] Install development tools (just, cargo-nextest, cargo-insta)
- [ ] Successfully run `cargo build` in `codex-rs/`
- [ ] Run `just test` and see tests pass
- [ ] Run `just codex` and explore the TUI

### Week 1-2: Understand the Codebase

- [ ] Read through this ONBOARDING.md
- [ ] Read `docs/getting-started.md`
- [ ] Read `docs/config.md` (first 200 lines)
- [ ] Read `docs/contributing.md`
- [ ] Explore the directory structure
- [ ] Identify the entry points in `codex-rs/cli/src/main.rs`
- [ ] Browse `codex-rs/core/src/lib.rs` to understand module organization

### Week 2: Make Your First Contribution

- [ ] Find a "good first issue" or documentation fix
- [ ] Create a feature branch
- [ ] Make a small change
- [ ] Run formatting: `just fmt`
- [ ] Run linting: `just clippy`
- [ ] Run tests: `just test`
- [ ] Open a PR and sign the CLA

### Week 2-3: Deeper Understanding

- [ ] Understand the TUI layer in `codex-rs/tui/`
- [ ] Explore the core logic in `codex-rs/core/src/codex.rs`
- [ ] Review how tools work in `codex-rs/core/src/tools/`
- [ ] Understand the authentication flow in `codex-rs/login/`
- [ ] Look at snapshot tests in `codex-rs/tui/tests/`

### Ongoing

- [ ] Set up your editor with rust-analyzer
- [ ] Join relevant communication channels
- [ ] Identify an area you want to specialize in
- [ ] Shadow a PR review or code discussion

---

## Quick Reference

### Most Used Commands

```bash
# Build and run
just codex "your prompt"      # Run with prompt
just test                      # Run tests
just fmt                       # Format code
just clippy                    # Run lints
just fix -p <crate>           # Apply clippy fixes

# Cargo commands
cargo build                    # Build
cargo nextest run              # Run tests
cargo doc --open               # View docs
cargo insta review             # Review snapshots
```

### Important Files

| File | Purpose |
|------|---------|
| `codex-rs/Cargo.toml` | Workspace definition, dependencies |
| `codex-rs/justfile` | Build commands |
| `codex-rs/rust-toolchain.toml` | Rust version (1.90.0) |
| `~/.codex/config.toml` | User configuration |
| `~/.codex/auth.json` | Authentication credentials |

### Getting Help

- Open a [GitHub Discussion](https://github.com/openai/codex/discussions)
- Check the [FAQ](./docs/faq.md)
- Security issues: security@openai.com

---

Welcome to the team! If you have questions, don't hesitate to ask.
