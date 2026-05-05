# Fetch

A frameless desktop pet and AI assistant built with Tauri. Fetch floats transparently on your screen, accepts natural language commands via a slide-out chat bar, and uses an LLM API with tool calling to execute native actions on your PC.

## Tech Stack

| Layer | Technology |
|---|---|
| Frontend | React 19 + TypeScript + Vite |
| Backend | Rust (Tauri v2) |
| AI | OpenAI (gpt-5-nano) or Anthropic (Claude Sonnet 4.6) |
| IPC | Tauri invoke commands |

## Architecture

```
┌─────────────────────────────┐
│  React UI (floating pet)    │  ← frameless, transparent window
│  • Double-click → chat bar  │
│  • drag to move window      │
└──────────┬──────────────────┘
           │ invoke("chat_with_pet")
           ▼
┌─────────────────────────────┐
│  Rust Backend (Tauri)       │
│  • chat_with_pet command    │
│  • LLM API call (reqwest)   │
│  • Tool execution switch    │
└─────────────────────────────┘
```

## LLM Models

- **OpenAI**: `gpt-5-nano` (set `OPENAI_API_KEY` in `.env`)
- **Anthropic**: `claude-sonnet-4-6` (set `ANTHROPIC_API_KEY` in `.env`)

If both keys are set, Anthropic takes priority.

## Tools

The LLM can call these native tools:

| Tool | Description | Implementation |
|---|---|---|
| `open_application(app_name)` | Launch an application | `std::process::Command` |
| `read_clipboard()` | Read system clipboard text | `arboard` crate |
| `change_theme(theme)` | Switch light/dark mode | Windows registry (`reg add`) |

Supported apps for `open_application`: notepad, calculator, chrome, firefox, edge, terminal, explorer, vscode, spotify. Any unrecognized name is passed to `cmd /C start`.

## Setup

### Prerequisites

- [Node.js](https://nodejs.org/) 18+
- [Rust](https://rustup.rs/)
- [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/) (Windows: Microsoft Visual Studio C++ Build Tools + WebView2)

### Install

```bash
npm install
```

### Configure API Key

```bash
cp .env.example .env
```

Edit `.env` and add one of:

```
OPENAI_API_KEY=sk-your-key-here
# or
ANTHROPIC_API_KEY=sk-ant-your-key-here
```

### Run (dev)

```bash
npm run tauri dev
```

### Build (production)

```bash
npm run tauri build
```

## Usage

| Action | How |
|---|---|
| **Move** | Click and drag anywhere on the pet |
| **Chat** | Double-click the pet → type → Enter |
| **Dismiss chat** | Press Escape or click outside |
| **Commands** | Ask natural language (e.g. "open notepad", "read my clipboard", "switch to dark mode") |

## Project Structure

```
fetch/
├── src/                    # React frontend
│   ├── main.tsx
│   ├── App.tsx             # Pet UI + chat bar
│   ├── App.css             # Animations + styling
│   └── index.css           # Transparent window base
├── src-tauri/              # Rust backend
│   ├── src/
│   │   ├── main.rs         # Windows subsystem entry
│   │   └── lib.rs          # Commands, LLM, tools
│   ├── Cargo.toml
│   └── tauri.conf.json     # Window config
├── index.html
├── vite.config.ts
└── package.json
```
