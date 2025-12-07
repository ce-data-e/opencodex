# Codex CLI - Quick Start Guide

Get up and running with Codex CLI development in 10 minutes.

## Prerequisites

- **OS**: macOS 12+, Ubuntu 20.04+, or Windows 11 (WSL2 only)
- **Git**: 2.23+
- **RAM**: 4GB minimum (8GB recommended)

## 1. Clone & Setup (5 minutes)

```bash
# Clone the repository
git clone https://github.com/openai/codex.git
cd codex/codex-rs

# Install Rust (if not already installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source "$HOME/.cargo/env"

# Install components (toolchain auto-installs from rust-toolchain.toml)
rustup component add rustfmt clippy

# Install development tools
cargo install just cargo-nextest cargo-insta
```

## 2. Build & Verify (3 minutes)

```bash
# Build the project
cargo build

# Run the tests
just test

# Check code quality
just clippy
```

## 3. Run Codex (2 minutes)

```bash
# Start the TUI
just codex

# Or with an initial prompt
just codex "explain this codebase to me"

# Non-interactive mode
just exec "list all files"
```

## Essential Commands

| Command | Description |
|---------|-------------|
| `just codex` | Run Codex TUI |
| `just test` | Run all tests |
| `just fmt` | Format code |
| `just clippy` | Run lints |
| `just fix -p <crate>` | Apply lint fixes |

## Project Structure

```
codex-rs/
├── cli/          # CLI entry point
├── core/         # Business logic
├── tui/          # Terminal UI
├── exec/         # Non-interactive mode
└── Cargo.toml    # Workspace config
```

## Making Your First Change

```bash
# 1. Create a branch
git checkout -b feat/my-change

# 2. Make your changes...

# 3. Run checks
just fmt && just clippy && just test

# 4. Commit and push
git add -A
git commit -m "feat: my change"
git push -u origin feat/my-change
```

## Configuration

User config: `~/.codex/config.toml`

```toml
model = "gpt-5.1"

[features]
web_search_request = true
```

## Getting Help

- Full guide: [ONBOARDING.md](./ONBOARDING.md)
- Documentation: [docs/](./docs/)
- FAQ: [docs/faq.md](./docs/faq.md)
- Issues: [GitHub Issues](https://github.com/openai/codex/issues)

## Next Steps

1. Read [ONBOARDING.md](./ONBOARDING.md) for comprehensive documentation
2. Explore [docs/getting-started.md](./docs/getting-started.md) for usage
3. Review [docs/contributing.md](./docs/contributing.md) before PRs
4. Run `cargo doc --open` to browse API documentation
