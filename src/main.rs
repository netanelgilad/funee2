mod emit_module;
pub mod execution_request;
mod funee_identifier;
mod http_loader;
mod load_module;
mod run_js;

use deno_core::{error::AnyError, op2};
use deno_error::JsErrorBox;
use execution_request::ExecutionRequest;
use funee_identifier::FuneeIdentifier;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, env, fs, path::Path, sync::{Arc, Mutex, LazyLock}};
use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher, EventKind};
use swc_common::SyntaxContext;
use swc_ecma_ast::{CallExpr, Callee, Expr, Ident};

/// Host function: log to stdout
#[op2(fast)]
fn op_log(#[string] message: &str) {
    println!("{}", message);
}

/// Host function: debug log (prefixed)
#[op2(fast)]
fn op_debug(#[string] message: &str) {
    println!("[DEBUG] {}", message);
}

/// Host function: generate cryptographically secure random bytes
/// Returns a hex-encoded string of the requested number of bytes
#[op2]
#[string]
fn op_randomBytes(length: u32) -> String {
    let mut bytes = vec![0u8; length as usize];
    rand::rng().fill_bytes(&mut bytes);
    hex::encode(bytes)
}

// ============================================================================
// Timer Host Functions
// ============================================================================

/// Storage for active timers (for cancellation)
static TIMER_CANCELLERS: LazyLock<Mutex<HashMap<u32, tokio::sync::oneshot::Sender<()>>>> = LazyLock::new(|| Mutex::new(HashMap::new()));
static NEXT_TIMER_ID: LazyLock<Mutex<u32>> = LazyLock::new(|| Mutex::new(1));

/// Host function: start a cancellable timer
/// Returns the timer ID immediately
#[op2(fast)]
fn op_timerStart() -> u32 {
    let timer_id = {
        let mut id = NEXT_TIMER_ID.lock().unwrap();
        let current = *id;
        *id += 1;
        current
    };
    timer_id
}

/// Host function: schedule a timer and wait for completion or cancellation
/// Returns true if completed, false if cancelled
#[op2]
async fn op_timerWait(timer_id: u32, delay_ms: u32) -> bool {
    use tokio::time::{Duration, sleep};
    use tokio::sync::oneshot;
    
    let (tx, rx) = oneshot::channel();
    
    // Store the cancellation sender
    {
        let mut cancellers = TIMER_CANCELLERS.lock().unwrap();
        cancellers.insert(timer_id, tx);
    }
    
    // Race between sleep and cancellation
    tokio::select! {
        _ = sleep(Duration::from_millis(delay_ms as u64)) => {
            // Timer completed - remove from cancellers
            let mut cancellers = TIMER_CANCELLERS.lock().unwrap();
            cancellers.remove(&timer_id);
            true
        }
        _ = rx => {
            // Timer was cancelled
            false
        }
    }
}

/// Host function: cancel a pending timer
/// Returns true if the timer was found and cancelled
#[op2(fast)]
fn op_timerCancel(timer_id: u32) -> bool {
    let mut cancellers = TIMER_CANCELLERS.lock().unwrap();
    if let Some(sender) = cancellers.remove(&timer_id) {
        // Send cancellation signal (ignore error if receiver already dropped)
        let _ = sender.send(());
        true
    } else {
        false
    }
}

// ============================================================================
// Filesystem Host Functions
// ============================================================================

/// Result wrapper for JSON serialization
#[derive(Serialize)]
#[serde(tag = "type")]
enum FsResult<T: Serialize> {
    #[serde(rename = "ok")]
    Ok { value: T },
    #[serde(rename = "error")]
    Err { error: String },
}

/// Host function: read file contents as UTF-8 string
/// Returns JSON: { type: "ok", value: "content" } or { type: "error", error: "message" }
#[op2]
#[string]
fn op_fsReadFile(#[string] path: &str) -> String {
    let result = match fs::read_to_string(path) {
        Ok(content) => FsResult::Ok { value: content },
        Err(e) => FsResult::Err { error: format!("readFile failed: {}", e) },
    };
    serde_json::to_string(&result).unwrap_or_else(|e| format!(r#"{{"type":"error","error":"{}"}}"#, e))
}

/// Host function: read file contents as binary (base64 encoded)
/// Returns JSON: { type: "ok", value: "<base64>" } or { type: "error", error: "message" }
#[op2]
#[string]
fn op_fsReadFileBinary(#[string] path: &str) -> String {
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    let result = match fs::read(path) {
        Ok(bytes) => FsResult::Ok { value: STANDARD.encode(&bytes) },
        Err(e) => FsResult::Err { error: format!("readFileBinary failed: {}", e) },
    };
    serde_json::to_string(&result).unwrap_or_else(|e| format!(r#"{{"type":"error","error":"{}"}}"#, e))
}

/// Host function: write string content to a file
/// Returns JSON: { type: "ok", value: null } or { type: "error", error: "message" }
#[op2]
#[string]
fn op_fsWriteFile(#[string] path: &str, #[string] content: &str) -> String {
    let result: FsResult<()> = match fs::write(path, content) {
        Ok(()) => FsResult::Ok { value: () },
        Err(e) => FsResult::Err { error: format!("writeFile failed: {}", e) },
    };
    serde_json::to_string(&result).unwrap_or_else(|e| format!(r#"{{"type":"error","error":"{}"}}"#, e))
}

/// Host function: write binary content (base64 encoded) to a file
/// Returns JSON: { type: "ok", value: null } or { type: "error", error: "message" }
#[op2]
#[string]
fn op_fsWriteFileBinary(#[string] path: &str, #[string] content_base64: &str) -> String {
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    let result: FsResult<()> = match STANDARD.decode(content_base64) {
        Ok(bytes) => match fs::write(path, bytes) {
            Ok(()) => FsResult::Ok { value: () },
            Err(e) => FsResult::Err { error: format!("writeFileBinary failed: {}", e) },
        },
        Err(e) => FsResult::Err { error: format!("writeFileBinary base64 decode failed: {}", e) },
    };
    serde_json::to_string(&result).unwrap_or_else(|e| format!(r#"{{"type":"error","error":"{}"}}"#, e))
}

/// Host function: check if path is a file (not directory or symlink)
#[op2(fast)]
fn op_fsIsFile(#[string] path: &str) -> bool {
    Path::new(path).is_file()
}

/// Struct returned by op_lstat
#[derive(Serialize)]
struct FileStats {
    size: u64,
    is_file: bool,
    is_directory: bool,
    is_symlink: bool,
    // Unix timestamps in milliseconds (like JS Date.now())
    modified_ms: Option<u64>,
    created_ms: Option<u64>,
    accessed_ms: Option<u64>,
}

/// Host function: get file stats (like lstat - does not follow symlinks)
/// Returns JSON: { type: "ok", value: { size, is_file, ... } } or { type: "error", error: "message" }
#[op2]
#[string]
fn op_fsLstat(#[string] path: &str) -> String {
    let result = match fs::symlink_metadata(path) {
        Ok(metadata) => {
            let stats = FileStats {
                size: metadata.len(),
                is_file: metadata.is_file(),
                is_directory: metadata.is_dir(),
                is_symlink: metadata.file_type().is_symlink(),
                modified_ms: metadata.modified().ok().and_then(|t| {
                    t.duration_since(std::time::UNIX_EPOCH).ok().map(|d| d.as_millis() as u64)
                }),
                created_ms: metadata.created().ok().and_then(|t| {
                    t.duration_since(std::time::UNIX_EPOCH).ok().map(|d| d.as_millis() as u64)
                }),
                accessed_ms: metadata.accessed().ok().and_then(|t| {
                    t.duration_since(std::time::UNIX_EPOCH).ok().map(|d| d.as_millis() as u64)
                }),
            };
            FsResult::Ok { value: stats }
        }
        Err(e) => FsResult::Err { error: format!("lstat failed: {}", e) },
    };
    serde_json::to_string(&result).unwrap_or_else(|e| format!(r#"{{"type":"error","error":"{}"}}"#, e))
}

/// Host function: list directory contents
/// Returns JSON: { type: "ok", value: ["file1", "file2", ...] } or { type: "error", error: "message" }
#[op2]
#[string]
fn op_fsReaddir(#[string] path: &str) -> String {
    let result = match fs::read_dir(path) {
        Ok(read_dir) => {
            let entries: Result<Vec<String>, _> = read_dir
                .map(|entry| entry.map(|e| e.file_name().to_string_lossy().to_string()))
                .collect();
            match entries {
                Ok(list) => FsResult::Ok { value: list },
                Err(e) => FsResult::Err { error: format!("readdir failed: {}", e) },
            }
        }
        Err(e) => FsResult::Err { error: format!("readdir failed: {}", e) },
    };
    serde_json::to_string(&result).unwrap_or_else(|e| format!(r#"{{"type":"error","error":"{}"}}"#, e))
}

// ============================================================================
// OS Host Functions
// ============================================================================

/// Host function: get the system's temporary directory path
#[op2]
#[string]
fn op_tmpdir() -> String {
    std::env::temp_dir().to_string_lossy().to_string()
}

/// Host function: check if a file or directory exists
#[op2(fast)]
fn op_fsExists(#[string] path: &str) -> bool {
    Path::new(path).exists()
}

/// Host function: create a directory (including parents)
/// Returns JSON: { type: "ok", value: null } or { type: "error", error: "message" }
#[op2]
#[string]
fn op_fsMkdir(#[string] path: &str) -> String {
    let result: FsResult<()> = match fs::create_dir_all(path) {
        Ok(()) => FsResult::Ok { value: () },
        Err(e) => FsResult::Err { error: format!("mkdir failed: {}", e) },
    };
    serde_json::to_string(&result).unwrap_or_else(|e| format!(r#"{{"type":"error","error":"{}"}}"#, e))
}

// ============================================================================
// HTTP Host Functions
// ============================================================================

/// Host function: HTTP fetch (blocking version for simplicity)
/// Takes method, URL, headers (as JSON string), and optional body
/// Returns a JSON string with { status, headers, body }
#[op2]
#[string]
fn op_httpFetch(
    #[string] method: &str,
    #[string] url: &str,
    #[string] headers_json: &str,
    #[string] body: &str,
) -> Result<String, JsErrorBox> {
    let client = reqwest::blocking::Client::new();
    
    // Build request based on method
    let mut request_builder = match method.to_uppercase().as_str() {
        "GET" => client.get(url),
        "POST" => client.post(url),
        "PUT" => client.put(url),
        "DELETE" => client.delete(url),
        "PATCH" => client.patch(url),
        "HEAD" => client.head(url),
        _ => return Err(JsErrorBox::type_error(format!("Unsupported HTTP method: {}", method))),
    };
    
    // Parse and add headers
    let headers: HashMap<String, String> = serde_json::from_str(headers_json)
        .map_err(|e| JsErrorBox::generic(format!("Invalid headers JSON: {}", e)))?;
    for (key, value) in headers {
        request_builder = request_builder.header(&key, &value);
    }
    
    // Add body if not empty
    if !body.is_empty() {
        request_builder = request_builder.body(body.to_string());
    }
    
    // Send request
    let response = request_builder.send()
        .map_err(|e| JsErrorBox::generic(format!("HTTP request failed: {}", e)))?;
    
    // Extract response data
    let status = response.status().as_u16();
    let response_headers: HashMap<String, String> = response
        .headers()
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
        .collect();
    let response_body = response.text()
        .map_err(|e| JsErrorBox::generic(format!("Failed to read response body: {}", e)))?;
    
    // Build response JSON
    let result = serde_json::json!({
        "status": status,
        "headers": response_headers,
        "body": response_body
    });
    
    Ok(result.to_string())
}

/// Host function: Async HTTP fetch (web-standard fetch implementation)
/// Takes method, URL, headers (as JSON string), optional body, and follow_redirects flag
/// Returns a JSON string with { status, statusText, headers, body, url, redirected }
#[op2]
#[string]
async fn op_fetch(
    #[string] method: String,
    #[string] url: String,
    #[string] headers_json: String,
    #[string] body: String,
    follow_redirects: bool,
) -> Result<String, JsErrorBox> {
    use reqwest::redirect::Policy;
    
    // Build client with redirect policy
    let client = reqwest::Client::builder()
        .redirect(if follow_redirects { Policy::limited(10) } else { Policy::none() })
        .build()
        .map_err(|e| JsErrorBox::generic(format!("Failed to build HTTP client: {}", e)))?;
    
    // Build request based on method
    let mut request_builder = match method.to_uppercase().as_str() {
        "GET" => client.get(&url),
        "POST" => client.post(&url),
        "PUT" => client.put(&url),
        "DELETE" => client.delete(&url),
        "PATCH" => client.patch(&url),
        "HEAD" => client.head(&url),
        "OPTIONS" => client.request(reqwest::Method::OPTIONS, &url),
        _ => return Err(JsErrorBox::type_error(format!("Unsupported HTTP method: {}", method))),
    };
    
    // Parse and add headers
    let headers: HashMap<String, String> = serde_json::from_str(&headers_json)
        .map_err(|e| JsErrorBox::generic(format!("Invalid headers JSON: {}", e)))?;
    for (key, value) in headers {
        request_builder = request_builder.header(&key, &value);
    }
    
    // Add body if not empty
    if !body.is_empty() {
        request_builder = request_builder.body(body);
    }
    
    // Send request
    let response = request_builder.send().await
        .map_err(|e| JsErrorBox::generic(format!("HTTP request failed: {}", e)))?;
    
    // Extract response data
    let status = response.status().as_u16();
    let status_text = response.status().canonical_reason().unwrap_or("").to_string();
    let final_url = response.url().to_string();
    let redirected = final_url != url;
    
    let response_headers: HashMap<String, String> = response
        .headers()
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
        .collect();
    
    let response_body = response.text().await
        .map_err(|e| JsErrorBox::generic(format!("Failed to read response body: {}", e)))?;
    
    // Build response JSON
    let result = serde_json::json!({
        "status": status,
        "statusText": status_text,
        "headers": response_headers,
        "body": response_body,
        "url": final_url,
        "redirected": redirected
    });
    
    Ok(result.to_string())
}

// ============================================================================
// File Watcher Host Functions
// ============================================================================

/// Watch event for serialization to JS
#[derive(Clone, Serialize, Deserialize)]
struct WatchEvent {
    kind: String,
    path: String,
}

/// State for a single watcher instance
struct WatcherState {
    _watcher: RecommendedWatcher,
    events: Arc<Mutex<Vec<WatchEvent>>>,
}

/// Global storage for active watchers
static WATCHERS: LazyLock<Mutex<HashMap<u32, WatcherState>>> = LazyLock::new(|| Mutex::new(HashMap::new()));
static NEXT_WATCHER_ID: LazyLock<Mutex<u32>> = LazyLock::new(|| Mutex::new(1));

/// Convert notify EventKind to simple string
fn event_kind_to_string(kind: &EventKind) -> &'static str {
    match kind {
        EventKind::Create(_) => "create",
        EventKind::Modify(_) => "modify",
        EventKind::Remove(_) => "remove",
        EventKind::Access(_) => "access",
        EventKind::Other => "other",
        EventKind::Any => "any",
    }
}

/// Host function: start watching a path
/// Returns watcher ID or error JSON
#[op2]
#[string]
fn op_watchStart(#[string] path: &str, recursive: bool) -> String {
    // Get next watcher ID
    let watcher_id = {
        let mut id = NEXT_WATCHER_ID.lock().unwrap();
        let current = *id;
        *id += 1;
        current
    };
    
    // Create event queue
    let events: Arc<Mutex<Vec<WatchEvent>>> = Arc::new(Mutex::new(Vec::new()));
    let events_clone = events.clone();
    
    // Create watcher with callback
    let watcher_result = RecommendedWatcher::new(
        move |res: Result<notify::Event, notify::Error>| {
            if let Ok(event) = res {
                let kind = event_kind_to_string(&event.kind);
                let mut queue = events_clone.lock().unwrap();
                for path in event.paths {
                    queue.push(WatchEvent {
                        kind: kind.to_string(),
                        path: path.to_string_lossy().to_string(),
                    });
                }
            }
        },
        Config::default(),
    );
    
    match watcher_result {
        Ok(mut watcher) => {
            let mode = if recursive { RecursiveMode::Recursive } else { RecursiveMode::NonRecursive };
            if let Err(e) = watcher.watch(Path::new(path), mode) {
                return serde_json::json!({
                    "type": "error",
                    "error": format!("Failed to watch path: {}", e)
                }).to_string();
            }
            
            // Store watcher state
            let state = WatcherState {
                _watcher: watcher,
                events,
            };
            WATCHERS.lock().unwrap().insert(watcher_id, state);
            
            serde_json::json!({
                "type": "ok",
                "value": watcher_id
            }).to_string()
        }
        Err(e) => {
            serde_json::json!({
                "type": "error",
                "error": format!("Failed to create watcher: {}", e)
            }).to_string()
        }
    }
}

/// Host function: poll for pending events
/// Returns JSON array of events or null if none
#[op2]
#[string]
fn op_watchPoll(watcher_id: u32) -> String {
    let watchers = WATCHERS.lock().unwrap();
    if let Some(state) = watchers.get(&watcher_id) {
        let mut events = state.events.lock().unwrap();
        if events.is_empty() {
            "null".to_string()
        } else {
            let drained: Vec<WatchEvent> = events.drain(..).collect();
            serde_json::to_string(&drained).unwrap_or_else(|_| "[]".to_string())
        }
    } else {
        "null".to_string()
    }
}

/// Host function: stop watching and cleanup
#[op2(fast)]
fn op_watchStop(watcher_id: u32) {
    WATCHERS.lock().unwrap().remove(&watcher_id);
}

// ============================================================================
// Subprocess Host Functions
// ============================================================================

use tokio::process::{Child as TokioChild, Command as TokioCommand};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use std::process::Stdio;

/// Process handle storage
struct ProcessHandle {
    child: TokioChild,
}

/// Global storage for active processes
static PROCESSES: LazyLock<Mutex<HashMap<u32, ProcessHandle>>> = LazyLock::new(|| Mutex::new(HashMap::new()));
static NEXT_PROCESS_ID: LazyLock<Mutex<u32>> = LazyLock::new(|| Mutex::new(1));

/// Host function: spawn a new process
/// Returns JSON with process_id and pid, or error
#[op2]
#[string]
fn op_processSpawn(
    #[string] cmd_json: &str,
    #[string] cwd: &str,
    #[string] env_json: &str,
    inherit_env: bool,
    #[string] stdin_mode: &str,
    #[string] stdout_mode: &str,
    #[string] stderr_mode: &str,
) -> Result<String, JsErrorBox> {
    // Parse command array
    let cmd: Vec<String> = serde_json::from_str(cmd_json)
        .map_err(|e| JsErrorBox::generic(format!("Invalid cmd JSON: {}", e)))?;
    
    if cmd.is_empty() {
        return Err(JsErrorBox::generic("Command array cannot be empty"));
    }
    
    // Build command
    let mut command = TokioCommand::new(&cmd[0]);
    command.args(&cmd[1..]);
    
    // Set working directory if specified
    if !cwd.is_empty() {
        command.current_dir(cwd);
    }
    
    // Handle environment
    if !inherit_env {
        command.env_clear();
    }
    
    // Parse and add custom env vars
    let env_vars: HashMap<String, String> = serde_json::from_str(env_json)
        .map_err(|e| JsErrorBox::generic(format!("Invalid env JSON: {}", e)))?;
    for (key, value) in env_vars {
        command.env(key, value);
    }
    
    // Set stdio modes
    command.stdin(match stdin_mode {
        "piped" => Stdio::piped(),
        "inherit" => Stdio::inherit(),
        _ => Stdio::null(),
    });
    command.stdout(match stdout_mode {
        "piped" => Stdio::piped(),
        "inherit" => Stdio::inherit(),
        _ => Stdio::null(),
    });
    command.stderr(match stderr_mode {
        "piped" => Stdio::piped(),
        "inherit" => Stdio::inherit(),
        _ => Stdio::null(),
    });
    
    // Spawn the process
    let child = command.spawn()
        .map_err(|e| JsErrorBox::generic(format!("Failed to spawn process: {}", e)))?;
    
    let pid = child.id().unwrap_or(0);
    
    // Generate process ID and store
    let process_id = {
        let mut id = NEXT_PROCESS_ID.lock().unwrap();
        let current = *id;
        *id += 1;
        current
    };
    
    PROCESSES.lock().unwrap().insert(process_id, ProcessHandle { child });
    
    Ok(serde_json::json!({
        "process_id": process_id,
        "pid": pid,
    }).to_string())
}

/// Host function: write data to process stdin (base64 encoded)
/// Returns bytes written or error
#[op2]
async fn op_processWrite(process_id: u32, #[string] data_base64: String) -> Result<u32, JsErrorBox> {
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    
    // Decode base64 data
    let data = STANDARD.decode(&data_base64)
        .map_err(|e| JsErrorBox::generic(format!("Invalid base64: {}", e)))?;
    
    // Take stdin out of the process handle
    let stdin_opt = {
        let mut processes = PROCESSES.lock().unwrap();
        let handle = processes.get_mut(&process_id)
            .ok_or_else(|| JsErrorBox::generic(format!("Process {} not found", process_id)))?;
        handle.child.stdin.take()
    };
    
    let mut stdin = stdin_opt
        .ok_or_else(|| JsErrorBox::generic("Process stdin not available (already taken or not piped)"))?;
    
    // Write the data
    let len = data.len();
    stdin.write_all(&data).await
        .map_err(|e| JsErrorBox::generic(format!("Write failed: {}", e)))?;
    
    // Put stdin back (so it can be written to again)
    {
        let mut processes = PROCESSES.lock().unwrap();
        if let Some(handle) = processes.get_mut(&process_id) {
            handle.child.stdin = Some(stdin);
        }
    }
    
    Ok(len as u32)
}

/// Host function: close process stdin
#[op2(fast)]
fn op_processCloseStdin(process_id: u32) -> Result<(), JsErrorBox> {
    let mut processes = PROCESSES.lock().unwrap();
    let handle = processes.get_mut(&process_id)
        .ok_or_else(|| JsErrorBox::generic(format!("Process {} not found", process_id)))?;
    
    // Take stdin to drop it, which closes it
    handle.child.stdin.take();
    Ok(())
}

/// Host function: read all stdout from process
/// Returns base64 encoded bytes
#[op2]
#[string]
async fn op_processReadStdout(process_id: u32) -> Result<String, JsErrorBox> {
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    
    let stdout_opt = {
        let mut processes = PROCESSES.lock().unwrap();
        let handle = processes.get_mut(&process_id)
            .ok_or_else(|| JsErrorBox::generic(format!("Process {} not found", process_id)))?;
        handle.child.stdout.take()
    };
    
    let mut stdout = stdout_opt
        .ok_or_else(|| JsErrorBox::generic("Process stdout not available"))?;
    
    let mut buffer = Vec::new();
    stdout.read_to_end(&mut buffer).await
        .map_err(|e| JsErrorBox::generic(format!("Read failed: {}", e)))?;
    
    Ok(STANDARD.encode(&buffer))
}

/// Host function: read all stderr from process
/// Returns base64 encoded bytes
#[op2]
#[string]
async fn op_processReadStderr(process_id: u32) -> Result<String, JsErrorBox> {
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    
    let stderr_opt = {
        let mut processes = PROCESSES.lock().unwrap();
        let handle = processes.get_mut(&process_id)
            .ok_or_else(|| JsErrorBox::generic(format!("Process {} not found", process_id)))?;
        handle.child.stderr.take()
    };
    
    let mut stderr = stderr_opt
        .ok_or_else(|| JsErrorBox::generic("Process stderr not available"))?;
    
    let mut buffer = Vec::new();
    stderr.read_to_end(&mut buffer).await
        .map_err(|e| JsErrorBox::generic(format!("Read failed: {}", e)))?;
    
    Ok(STANDARD.encode(&buffer))
}

/// Host function: wait for process to exit
/// Returns JSON with code, signal, success
#[op2]
#[string]
async fn op_processWait(process_id: u32) -> Result<String, JsErrorBox> {
    let child_opt = {
        let mut processes = PROCESSES.lock().unwrap();
        processes.remove(&process_id).map(|h| h.child)
    };
    
    let mut child = child_opt
        .ok_or_else(|| JsErrorBox::generic(format!("Process {} not found", process_id)))?;
    
    let status = child.wait().await
        .map_err(|e| JsErrorBox::generic(format!("Wait failed: {}", e)))?;
    
    let code = status.code();
    
    // On Unix, get signal if terminated by signal
    #[cfg(unix)]
    let signal = {
        use std::os::unix::process::ExitStatusExt;
        status.signal().map(|s| signal_name(s))
    };
    #[cfg(not(unix))]
    let signal: Option<String> = None;
    
    let success = status.success();
    
    Ok(serde_json::json!({
        "code": code,
        "signal": signal,
        "success": success,
    }).to_string())
}

/// Host function: send signal to process
#[op2(fast)]
fn op_processKill(process_id: u32, #[string] signal: &str) -> Result<(), JsErrorBox> {
    let mut processes = PROCESSES.lock().unwrap();
    let handle = processes.get_mut(&process_id)
        .ok_or_else(|| JsErrorBox::generic(format!("Process {} not found", process_id)))?;
    
    #[cfg(unix)]
    {
        use nix::sys::signal::{kill, Signal};
        use nix::unistd::Pid;
        
        let sig = match signal {
            "SIGTERM" => Signal::SIGTERM,
            "SIGKILL" => Signal::SIGKILL,
            "SIGINT" => Signal::SIGINT,
            "SIGHUP" => Signal::SIGHUP,
            "SIGQUIT" => Signal::SIGQUIT,
            _ => return Err(JsErrorBox::generic(format!("Unknown signal: {}", signal))),
        };
        
        if let Some(pid) = handle.child.id() {
            kill(Pid::from_raw(pid as i32), sig)
                .map_err(|e| JsErrorBox::generic(format!("Kill failed: {}", e)))?;
        }
    }
    
    #[cfg(not(unix))]
    {
        // On Windows, just try to kill the process
        handle.child.start_kill()
            .map_err(|e| JsErrorBox::generic(format!("Kill failed: {}", e)))?;
    }
    
    Ok(())
}

/// Host function: read process environment variable by name
#[op2]
#[string]
fn op_processEnv(#[string] name: &str) -> String {
    std::env::var(name).unwrap_or_default()
}

/// Convert signal number to name
#[cfg(unix)]
fn signal_name(signal: i32) -> String {
    match signal {
        1 => "SIGHUP".to_string(),
        2 => "SIGINT".to_string(),
        3 => "SIGQUIT".to_string(),
        9 => "SIGKILL".to_string(),
        15 => "SIGTERM".to_string(),
        _ => format!("SIG{}", signal),
    }
}

// ============================================================================
// HTTP Server Host Functions
// ============================================================================

use std::net::SocketAddr;
use hyper::{Request as HyperRequest, Response as HyperResponse, body::Incoming, server::conn::http1, Method, StatusCode};
use hyper::service::service_fn;
use hyper_util::rt::TokioIo;
use http_body_util::{BodyExt, Full};
use tokio::net::TcpListener;
use tokio::sync::{mpsc, oneshot, watch};
use bytes::Bytes;

/// Request info sent to JavaScript
#[derive(Serialize, Clone)]
struct ServerRequestInfo {
    request_id: u32,
    method: String,
    url: String,
    headers: Vec<(String, String)>,
    has_body: bool,
}

/// Pending request awaiting response
struct PendingRequest {
    body: Option<String>,
    response_sender: oneshot::Sender<HyperResponse<Full<Bytes>>>,
}

/// Server state
struct HttpServerState {
    shutdown_tx: Option<oneshot::Sender<()>>,
    conn_shutdown_tx: watch::Sender<bool>,
    active_connections: Arc<std::sync::atomic::AtomicU32>,
    request_rx: mpsc::Receiver<(ServerRequestInfo, PendingRequest)>,
    pending_requests: HashMap<u32, PendingRequest>,
    port: u16,
    hostname: String,
}

/// Global storage for servers
static SERVERS: LazyLock<Mutex<HashMap<u32, HttpServerState>>> = LazyLock::new(|| Mutex::new(HashMap::new()));
static NEXT_SERVER_ID: LazyLock<Mutex<u32>> = LazyLock::new(|| Mutex::new(1));
static NEXT_REQUEST_ID: LazyLock<Mutex<u32>> = LazyLock::new(|| Mutex::new(1));

/// Storage for request bodies (shared between server task and ops)
static REQUEST_BODIES: LazyLock<Mutex<HashMap<u32, String>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

/// Host function: start HTTP server
/// Returns JSON with server_id, port, hostname
/// 
/// Note: Uses synchronous bind so port is available immediately,
/// then converts to async TcpListener for the server loop.
#[op2]
#[string]
fn op_serverStart(port: u32, #[string] hostname: &str) -> Result<String, JsErrorBox> {
    let addr: SocketAddr = format!("{}:{}", hostname, port)
        .parse()
        .map_err(|e| JsErrorBox::generic(format!("Invalid address: {}", e)))?;
    
    // Bind synchronously to get the port immediately
    let std_listener = std::net::TcpListener::bind(addr)
        .map_err(|e| JsErrorBox::generic(format!("Failed to bind: {}", e)))?;
    
    // Set non-blocking for tokio
    std_listener.set_nonblocking(true)
        .map_err(|e| JsErrorBox::generic(format!("Failed to set non-blocking: {}", e)))?;
    
    let actual_addr = std_listener.local_addr()
        .map_err(|e| JsErrorBox::generic(format!("Failed to get address: {}", e)))?;
    
    let actual_port = actual_addr.port();
    let actual_hostname = hostname.to_string();
    
    // Convert to tokio TcpListener
    let listener = TcpListener::from_std(std_listener)
        .map_err(|e| JsErrorBox::generic(format!("Failed to create async listener: {}", e)))?;
    
    // Create channels for communication
    let (shutdown_tx, mut shutdown_rx) = oneshot::channel::<()>();
    let (conn_shutdown_tx, conn_shutdown_rx) = watch::channel(false);
    let active_connections = Arc::new(std::sync::atomic::AtomicU32::new(0));
    let (request_tx, request_rx) = mpsc::channel::<(ServerRequestInfo, PendingRequest)>(100);
    
    let server_id = {
        let mut id = NEXT_SERVER_ID.lock().unwrap();
        let current = *id;
        *id += 1;
        current
    };
    
    // Clone for the server task
    let request_tx_clone = request_tx.clone();
    let active_connections_clone = active_connections.clone();
    
    // Spawn server task
    tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = &mut shutdown_rx => {
                    break;
                }
                result = listener.accept() => {
                    match result {
                        Ok((stream, _)) => {
                            let io = TokioIo::new(stream);
                            let tx = request_tx_clone.clone();
                            let mut conn_shutdown = conn_shutdown_rx.clone();
                            let active_conns = active_connections_clone.clone();
                            
                            // Increment active connections
                            active_conns.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                            
                            tokio::spawn(async move {
                                let service = service_fn(|req: HyperRequest<Incoming>| {
                                    let tx = tx.clone();
                                    async move {
                                        // Generate request ID
                                        let request_id = {
                                            let mut id = NEXT_REQUEST_ID.lock().unwrap();
                                            let current = *id;
                                            *id += 1;
                                            current
                                        };
                                        
                                        // Extract request info
                                        let method = req.method().to_string();
                                        let uri = req.uri();
                                        // Only send path+query, JS will construct full URL
                                        let url = format!("{}{}", 
                                            uri.path(),
                                            uri.query().map(|q| format!("?{}", q)).unwrap_or_default()
                                        );
                                        let headers: Vec<(String, String)> = req.headers()
                                            .iter()
                                            .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
                                            .collect();
                                        let has_body = req.method() != Method::GET && req.method() != Method::HEAD;
                                        
                                        // Read body
                                        let body_bytes = req.collect().await
                                            .map(|b| b.to_bytes())
                                            .unwrap_or_default();
                                        let body_str = String::from_utf8_lossy(&body_bytes).to_string();
                                        
                                        // Store body for later retrieval
                                        {
                                            REQUEST_BODIES.lock().unwrap().insert(request_id, body_str.clone());
                                        }
                                        
                                        let info = ServerRequestInfo {
                                            request_id,
                                            method,
                                            url,
                                            headers,
                                            has_body: !body_str.is_empty(),
                                        };
                                        
                                        // Create response channel
                                        let (resp_tx, resp_rx) = oneshot::channel();
                                        
                                        let pending = PendingRequest {
                                            body: Some(body_str),
                                            response_sender: resp_tx,
                                        };
                                        
                                        // Send to accept queue
                                        if tx.send((info, pending)).await.is_err() {
                                            return Ok::<_, hyper::Error>(HyperResponse::builder()
                                                .status(StatusCode::INTERNAL_SERVER_ERROR)
                                                .body(Full::new(Bytes::from("Server shutting down")))
                                                .unwrap());
                                        }
                                        
                                        // Wait for response from JavaScript
                                        match resp_rx.await {
                                            Ok(response) => Ok(response),
                                            Err(_) => Ok(HyperResponse::builder()
                                                .status(StatusCode::INTERNAL_SERVER_ERROR)
                                                .body(Full::new(Bytes::from("Request dropped")))
                                                .unwrap()),
                                        }
                                    }
                                });
                                
                                // Serve the connection with graceful shutdown support
                                let conn = http1::Builder::new()
                                    // Disable keep-alive so connections close after response
                                    .keep_alive(false)
                                    .serve_connection(io, service);
                                tokio::pin!(conn);
                                
                                loop {
                                    tokio::select! {
                                        result = conn.as_mut() => {
                                            if let Err(e) = result {
                                                // Ignore "connection reset by peer" errors during shutdown
                                                let err_str = e.to_string();
                                                if !err_str.contains("connection reset") && 
                                                   !err_str.contains("broken pipe") {
                                                    eprintln!("HTTP connection error: {}", e);
                                                }
                                            }
                                            break;
                                        }
                                        _ = conn_shutdown.changed() => {
                                            if *conn_shutdown.borrow() {
                                                // Gracefully shutdown - finish current request, then close
                                                conn.as_mut().graceful_shutdown();
                                            }
                                        }
                                    }
                                }
                                
                                // Decrement active connections
                                active_conns.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
                            });
                        }
                        Err(e) => {
                            eprintln!("Accept error: {}", e);
                        }
                    }
                }
            }
        }
    });
    
    // Store server state
    let state = HttpServerState {
        shutdown_tx: Some(shutdown_tx),
        conn_shutdown_tx,
        active_connections,
        request_rx,
        pending_requests: HashMap::new(),
        port: actual_port,
        hostname: actual_hostname.clone(),
    };
    
    SERVERS.lock().unwrap().insert(server_id, state);
    
    Ok(serde_json::json!({
        "server_id": server_id,
        "port": actual_port,
        "hostname": actual_hostname,
    }).to_string())
}

/// Host function: accept next request
/// Returns JSON with request info or null if server stopped
#[op2]
#[string]
async fn op_serverAccept(server_id: u32) -> Result<String, JsErrorBox> {
    // Get the receiver from the server state
    let maybe_rx = {
        let mut servers = SERVERS.lock().unwrap();
        if let Some(state) = servers.get_mut(&server_id) {
            // We need to take the receiver temporarily
            Some(std::mem::replace(&mut state.request_rx, mpsc::channel(1).1))
        } else {
            // Server already stopped
            return Ok("null".to_string());
        }
    };
    
    let mut rx = maybe_rx.unwrap();
    
    // Wait for a request with a timeout so we can check if server is stopping
    use tokio::time::{timeout, Duration};
    
    loop {
        match timeout(Duration::from_millis(100), rx.recv()).await {
            Ok(Some((info, pending))) => {
                // Put the receiver back
                {
                    let mut servers = SERVERS.lock().unwrap();
                    if let Some(state) = servers.get_mut(&server_id) {
                        state.request_rx = rx;
                        state.pending_requests.insert(info.request_id, pending);
                    } else {
                        // Server was stopped while we were waiting
                        return Ok("null".to_string());
                    }
                }
                return Ok(serde_json::to_string(&info).unwrap());
            }
            Ok(None) => {
                // Channel closed, server shutting down
                return Ok("null".to_string());
            }
            Err(_) => {
                // Timeout - check if server is still alive
                let servers = SERVERS.lock().unwrap();
                if !servers.contains_key(&server_id) {
                    // Server was stopped
                    return Ok("null".to_string());
                }
                // Continue waiting
                drop(servers);
            }
        }
    }
}

/// Host function: read request body
#[op2]
#[string]
fn op_serverReadBody(request_id: u32) -> Result<String, JsErrorBox> {
    let body = REQUEST_BODIES.lock().unwrap().remove(&request_id);
    Ok(body.unwrap_or_default())
}

/// Host function: send response
#[op2]
async fn op_serverRespond(
    server_id: u32,
    request_id: u32,
    status: u32,
    #[string] headers_json: String,
    #[string] body: String,
) -> Result<(), JsErrorBox> {
    // Get the pending request
    let pending = {
        let mut servers = SERVERS.lock().unwrap();
        if let Some(state) = servers.get_mut(&server_id) {
            state.pending_requests.remove(&request_id)
        } else {
            None
        }
    };
    
    let pending = pending
        .ok_or_else(|| JsErrorBox::generic(format!("Request {} not found", request_id)))?;
    
    // Parse headers
    let headers: HashMap<String, String> = serde_json::from_str(&headers_json)
        .map_err(|e| JsErrorBox::generic(format!("Invalid headers JSON: {}", e)))?;
    
    // Build response
    let status_code = StatusCode::from_u16(status as u16)
        .map_err(|e| JsErrorBox::generic(format!("Invalid status code: {}", e)))?;
    
    let mut response_builder = HyperResponse::builder().status(status_code);
    
    for (name, value) in headers {
        response_builder = response_builder.header(&name, &value);
    }
    
    let response = response_builder
        .body(Full::new(Bytes::from(body)))
        .map_err(|e| JsErrorBox::generic(format!("Failed to build response: {}", e)))?;
    
    // Send response
    let _ = pending.response_sender.send(response);
    
    // Clean up body storage
    REQUEST_BODIES.lock().unwrap().remove(&request_id);
    
    Ok(())
}

/// Host function: stop server
#[op2]
async fn op_serverStop(server_id: u32) -> Result<(), JsErrorBox> {
    // First, signal graceful shutdown to all connections
    let (shutdown_tx, conn_shutdown_tx, active_connections) = {
        let mut servers = SERVERS.lock().unwrap();
        if let Some(state) = servers.get_mut(&server_id) {
            (
                state.shutdown_tx.take(),
                state.conn_shutdown_tx.clone(),
                state.active_connections.clone(),
            )
        } else {
            return Ok(());
        }
    };
    
    // Stop accepting new connections
    if let Some(tx) = shutdown_tx {
        let _ = tx.send(());
    }
    
    // Signal all connections to gracefully shutdown
    let _ = conn_shutdown_tx.send(true);
    
    // Wait for all active connections to complete (with timeout)
    use tokio::time::{timeout, Duration};
    let wait_result = timeout(Duration::from_secs(30), async {
        loop {
            let count = active_connections.load(std::sync::atomic::Ordering::SeqCst);
            if count == 0 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }).await;
    
    if wait_result.is_err() {
        eprintln!("Warning: Timed out waiting for connections to close");
    }
    
    // Remove server state
    SERVERS.lock().unwrap().remove(&server_id);
    
    Ok(())
}

fn main() -> Result<(), AnyError> {
    let args: Vec<String> = env::args().collect();
    let show_version = args.contains(&"--version".to_string());
    if show_version {
        println!("funee {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    
    if args.len() < 2 {
        eprintln!("Usage: funee [--emit] [--reload] [--version] <file.ts>");
        eprintln!("");
        eprintln!("Options:");
        eprintln!("  --emit    Print bundled JavaScript instead of executing");
        eprintln!("  --reload  Bypass HTTP cache and fetch fresh from network");
        eprintln!("  --version Print funee version and exit");
        eprintln!("");
        eprintln!("Runs the default export function from the given TypeScript file.");
        std::process::exit(1);
    }
    
    // Parse args
    let emit_only = args.contains(&"--emit".to_string());
    let force_reload = args.contains(&"--reload".to_string());
    let file_path = args.iter()
        .skip(1)
        .find(|arg| !arg.starts_with("--"))
        .expect("No file path provided");
    let absolute_path = if Path::new(file_path).is_absolute() {
        file_path.clone()
    } else {
        env::current_dir()?
            .join(file_path)
            .to_string_lossy()
            .to_string()
    };
    
    // Create expression to call the default export: default()
    let call_default = Expr::Call(CallExpr {
        span: Default::default(),
        ctxt: SyntaxContext::empty(),
        callee: Callee::Expr(Box::new(Expr::Ident(Ident::new(
            "default".into(),
            Default::default(),
            SyntaxContext::empty(),
        )))),
        type_args: None,
        args: vec![],
    });
    
    // Set up host functions
    let host_functions = HashMap::from([
        (
            FuneeIdentifier {
                name: "log".to_string(),
                uri: "funee".to_string(),
            },
            op_log(),
        ),
        (
            FuneeIdentifier {
                name: "debug".to_string(),
                uri: "funee".to_string(),
            },
            op_debug(),
        ),
        (
            FuneeIdentifier {
                name: "randomBytes".to_string(),
                uri: "funee".to_string(),
            },
            op_randomBytes(),
        ),
        // Filesystem host functions
        (
            FuneeIdentifier {
                name: "fsReadFile".to_string(),
                uri: "funee".to_string(),
            },
            op_fsReadFile(),
        ),
        (
            FuneeIdentifier {
                name: "fsReadFileBinary".to_string(),
                uri: "funee".to_string(),
            },
            op_fsReadFileBinary(),
        ),
        (
            FuneeIdentifier {
                name: "fsWriteFile".to_string(),
                uri: "funee".to_string(),
            },
            op_fsWriteFile(),
        ),
        (
            FuneeIdentifier {
                name: "fsWriteFileBinary".to_string(),
                uri: "funee".to_string(),
            },
            op_fsWriteFileBinary(),
        ),
        (
            FuneeIdentifier {
                name: "fsIsFile".to_string(),
                uri: "funee".to_string(),
            },
            op_fsIsFile(),
        ),
        (
            FuneeIdentifier {
                name: "fsLstat".to_string(),
                uri: "funee".to_string(),
            },
            op_fsLstat(),
        ),
        (
            FuneeIdentifier {
                name: "fsReaddir".to_string(),
                uri: "funee".to_string(),
            },
            op_fsReaddir(),
        ),
        // HTTP host functions
        (
            FuneeIdentifier {
                name: "httpFetch".to_string(),
                uri: "funee".to_string(),
            },
            op_httpFetch(),
        ),
        // Web-standard fetch (async version with full Response info)
        // Note: This is used by the global fetch() implementation
        (
            FuneeIdentifier {
                name: "_fetch".to_string(),
                uri: "funee".to_string(),
            },
            op_fetch(),
        ),
        // OS host functions
        (
            FuneeIdentifier {
                name: "tmpdir".to_string(),
                uri: "funee".to_string(),
            },
            op_tmpdir(),
        ),
        (
            FuneeIdentifier {
                name: "fsExists".to_string(),
                uri: "funee".to_string(),
            },
            op_fsExists(),
        ),
        (
            FuneeIdentifier {
                name: "fsMkdir".to_string(),
                uri: "funee".to_string(),
            },
            op_fsMkdir(),
        ),
        // Watcher host functions
        (
            FuneeIdentifier {
                name: "watchStart".to_string(),
                uri: "funee".to_string(),
            },
            op_watchStart(),
        ),
        (
            FuneeIdentifier {
                name: "watchPoll".to_string(),
                uri: "funee".to_string(),
            },
            op_watchPoll(),
        ),
        (
            FuneeIdentifier {
                name: "watchStop".to_string(),
                uri: "funee".to_string(),
            },
            op_watchStop(),
        ),
        // Timer host functions (internal - accessed via Deno.core.ops, not imports)
        (
            FuneeIdentifier {
                name: "timerStart".to_string(),
                uri: "funee:internal".to_string(),
            },
            op_timerStart(),
        ),
        (
            FuneeIdentifier {
                name: "timerWait".to_string(),
                uri: "funee:internal".to_string(),
            },
            op_timerWait(),
        ),
        (
            FuneeIdentifier {
                name: "timerCancel".to_string(),
                uri: "funee:internal".to_string(),
            },
            op_timerCancel(),
        ),
        // HTTP Server host functions (internal - accessed via Deno.core.ops)
        (
            FuneeIdentifier {
                name: "serverStart".to_string(),
                uri: "funee:internal".to_string(),
            },
            op_serverStart(),
        ),
        (
            FuneeIdentifier {
                name: "serverAccept".to_string(),
                uri: "funee:internal".to_string(),
            },
            op_serverAccept(),
        ),
        (
            FuneeIdentifier {
                name: "serverReadBody".to_string(),
                uri: "funee:internal".to_string(),
            },
            op_serverReadBody(),
        ),
        (
            FuneeIdentifier {
                name: "serverRespond".to_string(),
                uri: "funee:internal".to_string(),
            },
            op_serverRespond(),
        ),
        (
            FuneeIdentifier {
                name: "serverStop".to_string(),
                uri: "funee:internal".to_string(),
            },
            op_serverStop(),
        ),
        // Subprocess host functions (internal - accessed via Deno.core.ops)
        (
            FuneeIdentifier {
                name: "processSpawn".to_string(),
                uri: "funee:internal".to_string(),
            },
            op_processSpawn(),
        ),
        (
            FuneeIdentifier {
                name: "processWrite".to_string(),
                uri: "funee:internal".to_string(),
            },
            op_processWrite(),
        ),
        (
            FuneeIdentifier {
                name: "processCloseStdin".to_string(),
                uri: "funee:internal".to_string(),
            },
            op_processCloseStdin(),
        ),
        (
            FuneeIdentifier {
                name: "processReadStdout".to_string(),
                uri: "funee:internal".to_string(),
            },
            op_processReadStdout(),
        ),
        (
            FuneeIdentifier {
                name: "processReadStderr".to_string(),
                uri: "funee:internal".to_string(),
            },
            op_processReadStderr(),
        ),
        (
            FuneeIdentifier {
                name: "processWait".to_string(),
                uri: "funee:internal".to_string(),
            },
            op_processWait(),
        ),
        (
            FuneeIdentifier {
                name: "processKill".to_string(),
                uri: "funee:internal".to_string(),
            },
            op_processKill(),
        ),
        (
            FuneeIdentifier {
                name: "processEnv".to_string(),
                uri: "funee:internal".to_string(),
            },
            op_processEnv(),
        ),
    ]);
    
    // Locate funee-lib relative to the executable or use FUNEE_LIB_PATH env var
    let funee_lib_path = env::var("FUNEE_LIB_PATH").ok().or_else(|| {
        // Try to find funee-lib relative to the current executable
        env::current_exe().ok().and_then(|exe| {
            // In dev: exe is in target/release or target/debug
            // funee-lib is in the project root
            let mut path = exe.clone();
            // Go up from target/release/funee to project root
            for _ in 0..3 {
                path.pop();
            }
            path.push("funee-lib");
            path.push("index.ts");
            if path.exists() {
                return Some(path.to_string_lossy().to_string());
            }
            // Also check if funee-lib is next to the executable
            let mut path = exe;
            path.pop();
            path.push("funee-lib");
            path.push("index.ts");
            if path.exists() {
                return Some(path.to_string_lossy().to_string());
            }
            // Also check one directory above executable (e.g. <install>/bin/funee and <install>/funee-lib)
            let mut path = path;
            path.pop(); // index.ts
            path.pop(); // funee-lib
            path.pop(); // exe dir (bin)
            path.push("funee-lib");
            path.push("index.ts");
            if path.exists() {
                return Some(path.to_string_lossy().to_string());
            }
            None
        })
    });
    
    let request = ExecutionRequest {
        expression: call_default,
        scope: absolute_path,
        host_functions,
        funee_lib_path,
        file_loader: Box::new(http_loader::HttpFileLoader::with_force_reload(force_reload)?),
    };
    
    if emit_only {
        println!("{}", request.emit());
    } else {
        request.execute()?;
    }
    
    Ok(())
}
