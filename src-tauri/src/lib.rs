use std::process::Command;
use std::sync::Mutex;

// ── Conversation state ───────────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct ChatMessage {
    role: String,
    content: String,
}

struct ConversationHistory(Mutex<Vec<ChatMessage>>);

impl ConversationHistory {
    fn new() -> Self {
        Self(Mutex::new(vec![ChatMessage {
            role: "system".into(),
            content: "You are Fetch, a helpful desktop AI assistant pet. You can use tools to help the user. Keep responses concise and friendly.".into(),
        }]))
    }
}

// ── Tauri commands ───────────────────────────────────────────────────────────

#[tauri::command]
async fn chat_with_pet(
    state: tauri::State<'_, ConversationHistory>,
    prompt: String,
) -> Result<String, String> {
    // Add user message to history
    {
        let mut history = state.0.lock().unwrap();
        history.push(ChatMessage { role: "user".into(), content: prompt });
    }

    // Build messages from history
    let messages: Vec<ChatMessage> = state.0.lock().unwrap().clone();

    let response = if let Ok(key) = std::env::var("ANTHROPIC_API_KEY") {
        eprintln!("[fetch] Using Anthropic API");
        chat_with_anthropic(&key, &messages).await
    } else if let Ok(key) = std::env::var("OPENAI_API_KEY") {
        let masked = format!("{}...{} (len={})",
            &key[..key.len().min(20)],
            &key[key.len().saturating_sub(4)..],
            key.len());
        eprintln!("[fetch] Using OpenAI API | key: {masked}");
        chat_with_openai(&key, &messages).await
    } else {
        let cwd = std::env::current_dir().map(|p| p.display().to_string()).unwrap_or_default();
        eprintln!("[fetch] No API key found. CWD: {cwd}");
        Err(format!("No API key found in {cwd}. Set OPENAI_API_KEY or ANTHROPIC_API_KEY in .env"))
    }?;

    // Save assistant response to history
    {
        let mut history = state.0.lock().unwrap();
        history.push(ChatMessage { role: "assistant".into(), content: response.clone() });
    }

    Ok(response)
}

#[tauri::command]
async fn clear_conversation(state: tauri::State<'_, ConversationHistory>) -> Result<(), String> {
    let mut history = state.0.lock().unwrap();
    history.truncate(1); // keep system prompt
    eprintln!("[fetch] Conversation cleared");
    Ok(())
}

// ── OpenAI path ──────────────────────────────────────────────────────────────

async fn chat_with_openai(api_key: &str, messages: &[ChatMessage]) -> Result<String, String> {
    let client = reqwest::Client::new();

    let msgs: Vec<serde_json::Value> = messages.iter().map(|m| {
        serde_json::json!({ "role": m.role, "content": m.content })
    }).collect();

    let body = serde_json::json!({
        "model": "gpt-5.4-nano",
        "messages": msgs,
        "tools": tools_schema(),
        "tool_choice": "auto"
    });

    eprintln!("[fetch] Sending request to OpenAI ({} history msgs)...", messages.len());

    let resp = client
        .post("https://api.openai.com/v1/chat/completions")
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| {
            eprintln!("[fetch] HTTP send error: {e}");
            format!("HTTP error: {e}")
        })?;

    let status = resp.status();
    eprintln!("[fetch] OpenAI response status: {status}");

    let text = resp.text().await.map_err(|e| {
        eprintln!("[fetch] Body read error: {e}");
        format!("Read error: {e}")
    })?;

    eprintln!("[fetch] OpenAI response body (first 500 chars): {}", &text[..text.len().min(500)]);

    let json: serde_json::Value = serde_json::from_str(&text).map_err(|e| {
        eprintln!("[fetch] JSON parse error: {e}");
        format!("JSON parse error: {e}")
    })?;

    process_llm_response(json).await
}

// ── Anthropic path ───────────────────────────────────────────────────────────

async fn chat_with_anthropic(api_key: &str, messages: &[ChatMessage]) -> Result<String, String> {
    let client = reqwest::Client::new();

    // Separate system message from conversation messages
    let system_msg = messages.iter().find(|m| m.role == "system");
    let conversation: Vec<serde_json::Value> = messages.iter()
        .filter(|m| m.role != "system")
        .map(|m| {
            serde_json::json!({ "role": m.role, "content": m.content })
        }).collect();

    let mut body = serde_json::json!({
        "model": "claude-sonnet-4-6",
        "max_tokens": 1024,
        "messages": conversation,
        "tools": anthropic_tools_schema()
    });

    if let Some(sys) = system_msg {
        body["system"] = serde_json::json!(sys.content);
    }

    eprintln!("[fetch] Sending request to Anthropic ({} history msgs)...", messages.len());

    let resp = client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| {
            eprintln!("[fetch] HTTP send error: {e}");
            format!("HTTP error: {e}")
        })?;

    let status = resp.status();
    eprintln!("[fetch] Anthropic response status: {status}");

    let text = resp.text().await.map_err(|e| {
        eprintln!("[fetch] Body read error: {e}");
        format!("Read error: {e}")
    })?;

    eprintln!("[fetch] Anthropic response body (first 500 chars): {}", &text[..text.len().min(500)]);

    let json: serde_json::Value = serde_json::from_str(&text).map_err(|e| {
        eprintln!("[fetch] JSON parse error: {e}");
        format!("JSON parse error: {e}")
    })?;

    process_anthropic_response(json).await
}

// ── Tool schemas ─────────────────────────────────────────────────────────────

fn tools_schema() -> serde_json::Value {
    serde_json::json!([
        {
            "type": "function",
            "function": {
                "name": "open_application",
                "description": "Launch an application on the user's PC",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "app_name": {
                            "type": "string",
                            "description": "The name of the application to open (e.g., 'notepad', 'calculator', 'chrome')"
                        }
                    },
                    "required": ["app_name"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "read_clipboard",
                "description": "Read the current contents of the system clipboard",
                "parameters": {
                    "type": "object",
                    "properties": {}
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "change_theme",
                "description": "Switch the system between light and dark theme",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "theme": {
                            "type": "string",
                            "enum": ["light", "dark"],
                            "description": "The theme to apply"
                        }
                    },
                    "required": ["theme"]
                }
            }
        }
    ])
}

fn anthropic_tools_schema() -> serde_json::Value {
    serde_json::json!([
        {
            "name": "open_application",
            "description": "Launch an application on the user's PC",
            "input_schema": {
                "type": "object",
                "properties": {
                    "app_name": {
                        "type": "string",
                        "description": "The name of the application to open (e.g., 'notepad', 'calculator', 'chrome')"
                    }
                },
                "required": ["app_name"]
            }
        },
        {
            "name": "read_clipboard",
            "description": "Read the current contents of the system clipboard",
            "input_schema": {
                "type": "object",
                "properties": {}
            }
        },
        {
            "name": "change_theme",
            "description": "Switch the system between light and dark theme",
            "input_schema": {
                "type": "object",
                "properties": {
                    "theme": {
                        "type": "string",
                        "enum": ["light", "dark"],
                        "description": "The theme to apply"
                    }
                },
                "required": ["theme"]
            }
        }
    ])
}

// ── Response processing ──────────────────────────────────────────────────────

async fn process_llm_response(resp: serde_json::Value) -> Result<String, String> {
    let choice = &resp["choices"][0]["message"];

    if let Some(tool_calls) = choice["tool_calls"].as_array() {
        if let Some(tc) = tool_calls.first() {
            let func_name = tc["function"]["name"].as_str().unwrap_or("");
            let args_str = tc["function"]["arguments"].as_str().unwrap_or("{}");
            let args: serde_json::Value =
                serde_json::from_str(args_str).unwrap_or(serde_json::json!({}));

            return execute_tool(func_name, &args).await;
        }
    }

    choice["content"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| "No content in response".to_string())
}

async fn process_anthropic_response(resp: serde_json::Value) -> Result<String, String> {
    let content_blocks = resp["content"]
        .as_array()
        .ok_or_else(|| "No content blocks in response".to_string())?;

    for block in content_blocks {
        match block["type"].as_str() {
            Some("tool_use") => {
                let func_name = block["name"].as_str().unwrap_or("");
                let args = &block["input"];
                return execute_tool(func_name, args).await;
            }
            Some("text") => {
                if let Some(text) = block["text"].as_str() {
                    return Ok(text.to_string());
                }
            }
            _ => continue,
        }
    }

    Err("No text or tool use found in response".to_string())
}

// ── Tool execution switchboard ───────────────────────────────────────────────

async fn execute_tool(name: &str, args: &serde_json::Value) -> Result<String, String> {
    match name {
        "open_application" => {
            let app = args["app_name"]
                .as_str()
                .unwrap_or("notepad")
                .to_lowercase();

            let result = open_application(&app);
            match result {
                Ok(()) => Ok(format!("Opened {}", app)),
                Err(e) => Ok(format!("Failed to open {}: {}", app, e)),
            }
        }
        "read_clipboard" => read_clipboard(),
        "change_theme" => {
            let theme = args["theme"].as_str().unwrap_or("dark");
            change_theme(theme);
            Ok(format!("Theme changed to {} mode", theme))
        }
        _ => Err(format!("Unknown tool: {}", name)),
    }
}

// ── Native tool implementations ──────────────────────────────────────────────

fn open_application(app_name: &str) -> Result<(), String> {
    let (cmd, args) = match app_name {
        "notepad" => ("notepad", vec![]),
        "calculator" | "calc" => ("calc", vec![]),
        "chrome" => ("cmd", vec!["/C", "start", "chrome"]),
        "firefox" => ("cmd", vec!["/C", "start", "firefox"]),
        "edge" => ("cmd", vec!["/C", "start", "msedge"]),
        "terminal" | "cmd" => ("cmd", vec!["/C", "start", "cmd"]),
        "explorer" | "files" => ("explorer", vec![]),
        "vscode" | "code" => ("cmd", vec!["/C", "code"]),
        "spotify" => ("cmd", vec!["/C", "start", "spotify:"]),
        _ => ("cmd", vec!["/C", "start", app_name]),
    };

    Command::new(cmd)
        .args(&args)
        .spawn()
        .map_err(|e| format!("Could not launch {}: {}", app_name, e))?;

    Ok(())
}

fn read_clipboard() -> Result<String, String> {
    let mut clipboard =
        arboard::Clipboard::new().map_err(|e| format!("Clipboard error: {}", e))?;

    clipboard
        .get_text()
        .map(|text| {
            if text.len() > 500 {
                format!("Clipboard: {}...", &text[..500])
            } else {
                format!("Clipboard: {}", text)
            }
        })
        .map_err(|e| format!("Failed to read clipboard: {}", e))
}

fn change_theme(theme: &str) {
    let registry_value = match theme {
        "light" => "1",
        "dark" => "0",
        _ => "0",
    };

    let _ = Command::new("reg")
        .args([
            "add",
            "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize",
            "/v",
            "AppsUseLightTheme",
            "/t",
            "REG_DWORD",
            "/d",
            registry_value,
            "/f",
        ])
        .output();

    let _ = Command::new("reg")
        .args([
            "add",
            "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize",
            "/v",
            "SystemUsesLightTheme",
            "/t",
            "REG_DWORD",
            "/d",
            registry_value,
            "/f",
        ])
        .output();
}

fn find_env_file() -> Option<String> {
    let candidates = vec![
        std::env::current_dir().ok().map(|p| p.join(".env")),
        std::env::current_dir().ok().and_then(|p| p.parent().map(|q| q.join(".env"))),
        std::env::var("CARGO_MANIFEST_DIR").ok().and_then(|p| {
            std::path::Path::new(&p).parent().map(|q| q.join(".env"))
        }),
    ];

    for candidate in candidates.into_iter().flatten() {
        if candidate.exists() {
            return Some(candidate.display().to_string());
        }
    }
    None
}

// ── App entry point ──────────────────────────────────────────────────────────

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let env_file = find_env_file();
    if let Some(path) = &env_file {
        eprintln!("[fetch] Loading .env from {path}");
        if let Ok(contents) = std::fs::read_to_string(path) {
            for line in contents.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                if let Some((key, value)) = line.split_once('=') {
                    let key = key.trim();
                    let value = value.trim().trim_matches('"').trim_matches('\'');
                    std::env::set_var(key, value);
                }
            }
        }
    } else {
        eprintln!("[fetch] No .env file found");
    }

    tauri::Builder::default()
        .manage(ConversationHistory::new())
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![chat_with_pet, clear_conversation])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
