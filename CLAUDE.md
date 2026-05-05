# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Common Commands

```bash
npm install                    # Install frontend dependencies
npm run tauri dev              # Run in development mode (Vite + Tauri)
npm run tauri build            # Production build
npx tsc --noEmit              # TypeScript type check (frontend)
cargo check                    # Rust type check (run from src-tauri/)
cargo build                    # Rust build (run from src-tauri/)
```

There is no test suite yet. The project has no linting configured beyond `tsc --noEmit` and `cargo check`.

## Architecture

**Tauri v2** desktop app. Frameless transparent overlay window (400×300, no decorations, always-on-top, hidden from taskbar).

### IPC Surface

Two Tauri commands invoked from React via `@tauri-apps/api/core`:

| Command | Purpose |
|---|---|
| `chat_with_pet(prompt: String) -> String` | Sends user message with full conversation history to LLM, returns assistant response |
| `clear_conversation() -> ()` | Resets conversation history (keeps system prompt) |

### State Management

- **Backend**: `ConversationHistory(Mutex<Vec<ChatMessage>>)` is Tauri managed state. Contains system prompt at index 0, then alternating user/assistant messages. Every LLM request sends the full history.
- **Frontend**: Local `Message[]` in React state mirrors conversation for display. Messages persist only for the app session — no disk storage.

### LLM Flow

1. `chat_with_pet` checks env vars: Anthropic key takes priority over OpenAI
2. Full `Vec<ChatMessage>` serialized into provider-specific JSON format
3. Request includes tool schemas (see below)
4. Response parser checks for `tool_use` blocks first, text blocks second
5. Assistant response appended to `ConversationHistory`

### Tool Execution

Three tools defined in both OpenAI and Anthropic schemas:

| Tool | Implementation |
|---|---|
| `open_application(app_name)` | `std::process::Command` — maps known app names (notepad, chrome, etc.) to Windows commands, falls back to `cmd /C start` |
| `read_clipboard()` | `arboard` crate — returns clipboard text truncated to 500 chars |
| `change_theme(theme)` | Windows registry — sets `AppsUseLightTheme` and `SystemUsesLightTheme` via `reg add` |

Tools are executed in the `execute_tool` match statement. Tool results are returned as the response string (not fed back to the LLM for follow-up).

## Critical Implementation Details

### Port configuration
Vite dev server binds to **127.0.0.1:1430** (IPv4 explicitly). The default `localhost` resolves to `::1` (IPv6) on Windows which causes `EACCES`. `tauri.conf.json` devUrl must match.

### .env loading
The `run()` function has a **custom .env parser** that calls `std::env::set_var()` for each key. This intentionally overrides system environment variables. The `dotenvy` crate was removed because its `dotenv()` function silently skips variables already present in the environment — if the user has a stale `OPENAI_API_KEY` system var, it would take precedence over the `.env` file.

### TLS backend
`reqwest` uses `native-tls` (Windows Schannel), not `rustls-tls`. `rustls-tls` lacks Windows root certificates and causes TLS verification failures.

### Window transparency
Requires `decorations: false`, `transparent: true`, `shadow: false` in `tauri.conf.json`. The CSS must set `background: transparent !important` on `html`, `body`, and `#root`.

### Drag region
`data-tauri-drag-region` is on `.pet-container` (the full window div), not just the pet sprite. Tauri v2 automatically excludes interactive elements (input, button) from drag regions.

### Icon
The ICO file is a hand-crafted 32×32 BMP-based ICO. The `tauri icon` CLI requires a source image ≥1024px, which doesn't exist yet.
