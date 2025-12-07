# Running the Forked Codex with Klarna AI Gateway

This fork adds support for additional model providers through the Klarna AI Gateway.

## Prerequisites

- Rust toolchain (install via [rustup](https://rustup.rs/))
- AI Gateway API key (set as `AI_GATEWAY_API_KEY` environment variable)

## Building

```bash
cd /Users/chimezie.ezirim/code/opencodex/codex-rs

# Build in release mode
cargo build --release

# The binary will be at: target/release/codex
```

## Running

### Option 1: Run directly from target

```bash
./target/release/codex
```

### Option 2: Install locally

```bash
# Install to ~/.cargo/bin (make sure it's in your PATH)
cargo install --path cli
```

Then run with:
```bash
codex
```

### Option 3: Create an alias

Add to your `~/.zshrc` or `~/.bashrc`:

```bash
alias codex-fork="/Users/chimezie.ezirim/code/opencodex/codex-rs/target/release/codex"
```

## Configuration

The Klarna AI Gateway uses the **OpenAI-compatible Chat Completions API** for all models, including Gemini.

### Basic Setup in `~/.codex/config.toml`

```toml
# Use Gemini through Klarna AI Gateway
model = "gemini-3-pro-preview"
model_provider = "klarna-ai-gateway"

# Provider definition
[model_providers.klarna-ai-gateway]
name = "Klarna AI Gateway"
base_url = "https://ai-gateway-us.nonprod.klarna.net/v1"
env_key = "AI_GATEWAY_API_KEY"
wire_api = "chat"
```

### Configuration Options

| Field | Description |
|-------|-------------|
| `model_provider` | **Required at top level** - Key of the provider to use |
| `model` | The model name to use (e.g., `gemini-3-pro-preview`, `gpt-5.1-codex`) |
| `base_url` | Base URL for the API |
| `env_key` | Environment variable containing the API key |
| `wire_api` | API format: `"chat"` for Chat Completions, `"responses"` for Responses API |

### Switching Models

Simply change the `model` field in your config:

```toml
# For Gemini
model = "gemini-3-pro-preview"

# For GPT
model = "gpt-5.1-codex"

# For Claude via Vertex
model = "vertex_anthropic/claude-sonnet-4-5@20250929"
```

## Environment Variables

Make sure your API key is set:

```bash
export AI_GATEWAY_API_KEY="your-api-key-here"
```

## Available Models

Models available through Klarna AI Gateway (from `/v1/models`):

**Gemini:**
- `gemini-3-pro-preview` - Latest Gemini 3 Pro preview
- `gemini-3-pro-image-preview` - Gemini 3 Pro with image generation

**GPT:**
- `gpt-5.1-2025-11-13` - GPT 5.1
- `gpt-4.1` - GPT 4.1
- `gpt-4.1-mini` - GPT 4.1 Mini
- `gpt-4.1-nano` - GPT 4.1 Nano

**Claude:**
- `anthropic/claude-sonnet-4-5-20250929`
- `anthropic/claude-opus-4-5-20251101`
- `vertex_anthropic/claude-sonnet-4-5@20250929`
- `sonnet-3.5`
- `haiku`
- `opus`

## Features Supported

- Streaming text responses
- Function/tool calling
- Multi-turn conversations
- System instructions
- Token usage tracking

## Troubleshooting

### Build errors

If you encounter build errors, ensure you have the latest Rust toolchain:

```bash
rustup update
```

### API errors

1. Verify your API key is set correctly: `echo $AI_GATEWAY_API_KEY`
2. Check the base URL: `https://ai-gateway-us.nonprod.klarna.net/v1`
3. Ensure the model name is valid (check `/v1/models` endpoint)
4. Use `wire_api = "chat"` for Chat Completions format

### Debug mode

Run with debug logging:

```bash
RUST_LOG=debug codex
```

### Test the endpoint

```bash
curl -X POST "https://ai-gateway-us.nonprod.klarna.net/v1/chat/completions" \
  -H "Authorization: Bearer $AI_GATEWAY_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{"model":"gemini-3-pro-preview","messages":[{"role":"user","content":"Hi"}]}'
```

## Note on Native Gemini API

This fork also includes a native Gemini wire API (`wire_api = "gemini"`) that uses Google's native `generateContent` endpoint format. However, since the Klarna AI Gateway provides OpenAI-compatible access to Gemini models, **you don't need to use the native Gemini API** - just use `wire_api = "chat"` with the Gemini model name.

The native Gemini implementation is available for gateways that expose the raw Vertex AI endpoints at `/google/v1beta1/publishers/google/models/{model}:generateContent`.
