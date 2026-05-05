
use std::process::Command;

#[tauri::command]
async fn chat_with_pet(prompt: String) -> Result<String, String> {
    if let Ok(key) = dotenvy::var("ANTHROPIC_API_KEY") {
        chat_with_anthropic(&key, &prompt).await
    } else if let Ok(key) = dotenvy::var("OPENAI_API_KEY") {
        chat_with_openai(&key, &prompt).await
    } else {
        Err("No API key found. Set OPENAI_API_KEY or ANTHROPIC_API_KEY in .env".to_string())
    }
}

// ── OpenAI path ──────────────────────────────────────────────────────────────

async fn chat_with_openai(api_key: &str, prompt: &str) -> Result<String, String> {
    let client = reqwest::Client::new();

    let body = serde_json::json!({
        "model": "gpt-5-nano",
        "messages": [
            {
                "role": "system",
                "content": "You are Fetch, a helpful desktop AI assistant pet. You can use tools to help the user. Keep responses concise and friendly."
            },
            {
                "role": "user",
                "content": prompt
            }
        ],
        "tools": tools_schema(),
        "tool_choice": "auto"
    });

    let resp = client
        .post("https://api.openai.com/v1/chat/completions")
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("HTTP error: {}", e))?
        .json::<serde_json::Value>()
        .await
        .map_err(|e| format!("JSON parse error: {}", e))?;

    process_llm_response(resp).await
}

// ── Anthropic path ───────────────────────────────────────────────────────────

async fn chat_with_anthropic(api_key: &str, prompt: &str) -> Result<String, String> {
    let client = reqwest::Client::new();

    let body = serde_json::json!({
        "model": "claude-sonnet-4-6",
        "max_tokens": 1024,
        "system": "You are Fetch, a helpful desktop AI assistant pet. You can use tools to help the user. Keep responses concise and friendly.",
        "messages": [
            {
                "role": "user",
                "content": prompt
            }
        ],
        "tools": anthropic_tools_schema()
    });

    let resp = client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("HTTP error: {}", e))?
        .json::<serde_json::Value>()
        .await
        .map_err(|e| format!("JSON parse error: {}", e))?;

    process_anthropic_response(resp).await
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

    // Check for tool calls
    if let Some(tool_calls) = choice["tool_calls"].as_array() {
        if let Some(tc) = tool_calls.first() {
            let func_name = tc["function"]["name"].as_str().unwrap_or("");
            let args_str = tc["function"]["arguments"].as_str().unwrap_or("{}");
            let args: serde_json::Value =
                serde_json::from_str(args_str).unwrap_or(serde_json::json!({}));

            return execute_tool(func_name, &args).await;
        }
    }

    // Text response
    choice["content"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| "No content in response".to_string())
}

async fn process_anthropic_response(resp: serde_json::Value) -> Result<String, String> {
    // Anthropic returns an array of content blocks
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
    // Set Windows registry theme preference via reg command
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

// ── App entry point ──────────────────────────────────────────────────────────

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Load .env file
    let _ = dotenvy::dotenv();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![chat_with_pet])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
