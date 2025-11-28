# Safe syncing this fork with upstream Codex

This document describes how to keep this fork in sync with the original `openai/codex`
repository **without losing our local improvements**, and how to safely use Codex CLI
to help with that process.

The guidance assumes:

- Your fork remote is called `origin` (this repo: `ce-data-e/opencodex`).
- The upstream OpenAI repo is added as `upstream` (see setup below).
- Your long‑lived integration branch with Gemini support is `allow_gemini`.

Adjust names if your local setup differs.

---

## 1. Local changes in this fork

All local work we care about preserving is currently represented as these commits
on `allow_gemini` (relative to `main`):

- `09b666e3fc7449e2f6750f7df9a9147a32bf0724`  
  **feat: add thought signature support for Gemini function calls**  
  Key effects:
  - Extends `ResponseItem::FunctionCall` with a `thought_signature` field for Gemini
    thinking mode (`codex-rs/protocol/src/models.rs`).
  - Wires `thought_signature` through:
    - Chat Completions request builder
      (`codex-rs/codex-api/src/requests/chat.rs`), including nested
      `tool_calls[].extra_content.google.thought_signature` for Gemini 3 Pro.
    - Gemini request builder and SSE parser
      (`codex-rs/codex-api/src/requests/gemini.rs`,
      `codex-rs/codex-api/src/sse/gemini.rs`,
      `codex-rs/codex-api/src/endpoint/gemini.rs`,
      `codex-rs/codex-api/src/sse/chat.rs`).
  - Integrates Gemini into the Codex model‑family system so it behaves like a
    coding agent (same system instructions and tool configuration as
    `gpt-5.1-codex`), including:
    - `shell_type = ShellCommand`
    - `apply_patch_tool_type = Freeform`
    - `supports_parallel_tool_calls = true`  
    (`codex-rs/core/src/model_family.rs`).
  - Minor related adjustments in `codex-rs/core/src/tools/handlers/read_file.rs`.

- `2348c3ee17df08ed55401f47308a9c620b2a0c7e`  
  **feat: add support for Google Gemini API and update related endpoints**  
  Key effects:
  - Adds a dedicated Gemini endpoint/client that talks to the Google
    `generateContent` API and maps it into Codex’s streaming interface
    (`codex-rs/codex-api/src/endpoint/gemini.rs`,
    `codex-rs/codex-api/src/sse/gemini.rs`).
  - Introduces a Gemini request builder that converts Codex `ResponseItem`s to
    Gemini `contents[].parts[]` (`codex-rs/codex-api/src/requests/gemini.rs`).
  - Extends the provider and client layers to support `WireApi::Gemini` and
    route Gemini‑configured providers through the new client
    (`codex-rs/codex-api/src/provider.rs`,
    `codex-rs/codex-api/src/endpoint/mod.rs`,
    `codex-rs/codex-api/src/sse/mod.rs`,
    `codex-rs/codex-api/src/lib.rs`,
    `codex-rs/core/src/client.rs`,
    `codex-rs/core/src/model_provider_info.rs`).
  - Adds `run_fork.md` instructions and tests to exercise the new path.

- `7d570d86133867e3a7ed596e00e4bc0ccf229331`  
  **feat: add onboarding and quick start guides for new developers**  
  - New docs: `ONBOARDING.md`, `QUICKSTART.md`.

- `4591df9660995c52b8aeaef886bc9fffbd84aa92`  
  **chore: update .gitignore to exclude .aider files**  
  - Single‑line `.gitignore` addition.

These four commits are what we need to preserve whenever we sync with upstream.
Future work should continue to build on top of `allow_gemini` so the “local
patch set” remains a small, well‑defined tail on top of upstream.

---

## 2. Recommended branch & remote layout

To keep syncing safe and predictable, use this topology:

- `origin` – your fork (`ce-data-e/opencodex`).
- `upstream` – the official `openai/codex` repo.
- `main` – a clean mirror of `upstream/main` (no fork‑only commits).
- `allow_gemini` – your long‑lived integration branch:
  - Based on `main`.
  - Contains the four commits listed above (and any future enhancements).

One‑time setup:

```bash
git remote add upstream https://github.com/openai/codex.git  # if not present

# Make sure main tracks origin/main and is clean
git checkout main
git status
```

If `main` has any local commits beyond `origin/main`, decide whether to move
them to `allow_gemini` (usually yes) or drop them before adopting this guide.

---

## 3. Safe sync procedure (CLI‑friendly)

Run these steps every time you want to pull in upstream changes while keeping
your Gemini improvements.

### 3.1. Update `main` from upstream

```bash
git checkout main
git fetch upstream
git merge --ff-only upstream/main
```

Notes:

- `--ff-only` is important: it guarantees `main` stays a fast‑forward mirror of
  `upstream/main`. If this fails, you have local commits on `main`; either move
  them to `allow_gemini` or resolve that divergence manually before proceeding.
- After this, `main` should be identical to `upstream/main`.

### 3.2. Make a safety backup of `allow_gemini`

```bash
git checkout allow_gemini
git fetch origin
git merge --ff-only origin/allow_gemini  # ensure you're up to date with your fork

# Create a dated backup branch in case the rebase goes wrong
git branch backup/allow_gemini-$(date +%Y%m%d-%H%M%S)
```

If anything goes badly during rebase, you can always:

```bash
git checkout allow_gemini
git reset --hard backup/allow_gemini-...
```

### 3.3. Rebase `allow_gemini` onto the new upstream

```bash
git checkout allow_gemini
git rebase main
```

Conceptually, this replays the four local commits:

- `4591df96` `.gitignore` tweak
- `7d570d86` onboarding docs
- `2348c3ee` Gemini API support
- `09b666e3` Gemini thought signatures & Codex integration

on top of the latest `upstream/main`.

If there are no conflicts, you’re done with history rewriting. Continue with
post‑sync validation (section 4).

If there **are** conflicts, follow section 5.

### 3.4. Push updated branch to your fork

Once rebase and validation succeed:

```bash
git push --force-with-lease origin allow_gemini
```

Always use `--force-with-lease`, not `--force`, so you don’t accidentally
overwrite someone else’s work.

---

## 4. Post‑sync validation checklist

Run these from `codex-rs/` on the `allow_gemini` branch:

```bash
cd codex-rs

# 1. Format Rust code
just fmt

# 2. Fix clippy issues in touched projects (at minimum)
just fix -p codex-api
just fix -p codex-core

# 3. Run targeted tests
cargo test -p codex-api
cargo test -p codex-core
```

If you touched common/core/protocol crates in a given sync, consider running
the full test suite as well:

```bash
cargo test --all-features
```

Only skip or trim tests when they are known to be incompatible with your
environment (e.g., seatbelt / sandbox constraints), and document any skipped
tests in your PR or notes.

---

## 5. Handling rebase conflicts safely

During `git rebase main` on `allow_gemini`, `git` may stop with conflicts.
Inspect conflicts with:

```bash
git status
git diff        # or: git diff --stat
```

### 5.1. Files most likely to conflict

Conflicts that matter most for Gemini behavior are typically in:

- `codex-rs/codex-api/src/endpoint/gemini.rs`
- `codex-rs/codex-api/src/requests/gemini.rs`
- `codex-rs/codex-api/src/sse/gemini.rs`
- `codex-rs/codex-api/src/requests/chat.rs`
- `codex-rs/codex-api/src/sse/chat.rs`
- `codex-rs/core/src/client.rs`
- `codex-rs/core/src/model_provider_info.rs`
- `codex-rs/core/src/model_family.rs`
- `codex-rs/protocol/src/models.rs`
- `codex-rs/core/src/tools/handlers/read_file.rs`

Other conflicts (e.g., in docs, `.gitignore`, or unrelated modules) are usually
safe to resolve in favor of upstream or by combining changes mechanically.

### 5.2. When the agent may auto‑resolve

It is generally safe for the Codex CLI agent to resolve conflicts **without**
human intervention when:

1. **Pure formatting / comments / imports**  
   - Differences are only whitespace, import ordering, doc comments, or other
     non‑behavioral changes.

2. **Non‑overlapping edits**  
   - Upstream added new functions or enum variants, and our branch added
     Gemini‑specific code in different parts of the file.
   - Example: upstream adds a new provider field; our Gemini code lives in a
     separate match arm or function.

3. **Straightforward field extensions**  
   - Upstream adds new optional struct fields that don’t conflict with our
     `thought_signature` plumbing or Gemini logic.
   - Agent can keep both: include upstream fields and retain our Gemini fields.

4. **Docs and `.gitignore`**  
   - Conflicts in `ONBOARDING.md`, `QUICKSTART.md`, or `.gitignore` where the
     resolution is “keep both lines/sections” are fine for the agent to apply.

In these cases, the agent should:

- Prefer combining both sides when simple.
- Re‑run `just fmt`, `just fix -p …`, and targeted tests afterward.

### 5.3. When the agent must get human input

The agent **must stop and ask you** before deciding when any of the following
are true:

1. **Behavioral changes to Gemini wiring**  
   - Upstream modifies:
     - The Gemini client or request builders.
     - How `WireApi::Gemini` is mapped in `ModelProviderInfo` or
       `ModelClient::stream`.
   - Our fork also changes these same functions/structs.
   - Example: upstream introduces its own thought‑signature handling or
     different Gemini endpoint paths.

2. **Structural changes to `ResponseItem` or protocol types**  
   - Upstream adds/removes fields on `ResponseItem::FunctionCall`,
     `FunctionCallOutputPayload`, or related enums.
   - Our `thought_signature` and tool‑call handling must be reconciled with
     those changes.

3. **Large refactors in touched modules**  
   - Upstream significantly rewrites files like:
     - `codex-rs/core/src/client.rs`
     - `codex-rs/codex-api/src/endpoint/gemini.rs`
     - `codex-rs/codex-api/src/sse/chat.rs`
   - The conflict cannot be resolved by a simple “take ours” or “take theirs”
     without re‑understanding the new design.

4. **Conflicting behavior around long‑running tasks**  
   - Upstream changes the logic around task loops, tool routing, or
     conversation history in ways that may affect long‑running tasks (for
     example, changes in `codex-rs/core/src/codex.rs` or
     `codex-rs/core/src/response_processing.rs`).

5. **Any case where intent is unclear**  
   - If the agent cannot confidently explain *why* a particular side should
     win and what behavior will result, it should not guess.

In these situations, the agent should:

1. Summarize the conflict for you:
   - Files and hunks involved.
   - What upstream is trying to do.
   - How that interacts with our Gemini behavior.
2. Propose one or more resolution strategies in natural language.
3. Wait for your explicit go‑ahead before applying any non‑trivial merge.

### 5.4. Concrete CLI conflict‑resolution flow

When `git rebase main` stops with conflicts:

```bash
# See what’s conflicted
git status

# Use Codex CLI to inspect and propose a resolution
codex
```

In Codex, you can paste:

> I’m rebasing `allow_gemini` onto `main`. Please help resolve the current
> conflicts following `safe_syncing.md`. Start by showing me which files are
> conflicted and whether they fall into the “agent may auto‑resolve” or
> “get human input” categories.

Then let Codex:

1. Run `git diff` / `git diff --stat` for you.
2. Classify each conflict as safe/unsafe to auto‑resolve.
3. For safe ones, apply merges.
4. For unsafe ones, stop and ask.

After all conflicts are resolved:

```bash
git add <files you fixed>
git rebase --continue
```

Repeat until the rebase finishes or you decide to abort:

```bash
git rebase --abort  # if you want to go back to the pre-rebase state
```

---

## 6. Summary of “never do this” rules

- **Do not** commit fork‑only work directly to `main`. Always go through
  `allow_gemini` (or another feature branch).
- **Do not** `git pull` on `allow_gemini` from upstream; always sync via:
  - `main` ← `upstream/main`
  - `allow_gemini` ← `rebase main`
- **Do not** use `git push --force` without `--force-with-lease`.
- **Do not** let the agent unilaterally rewrite behavior in the Gemini or
  protocol layers when upstream has also changed those areas. Always review
  those merges as a human.

Following this guide keeps our Gemini integration (`2348c3ee` and `09b666e3`)
and local docs/config changes (`7d570d86`, `4591df96`) intact while tracking the
latest improvements from upstream Codex.

