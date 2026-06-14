use std::collections::HashMap;
use std::sync::Arc;

use codex_code_nav::find_project_root;
use codex_code_nav::run_query;
use codex_code_nav::run_symbols_for_files;
use codex_code_nav::scan_for_changes;
use codex_code_nav::Lang;
use codex_code_nav::NavIndex;
use rmcp::ErrorData as McpError;
use rmcp::handler::server::ServerHandler;
use rmcp::model::CallToolRequestParam;
use rmcp::model::CallToolResult;
use rmcp::model::ListToolsResult;
use rmcp::model::PaginatedRequestParam;
use rmcp::model::ServerCapabilities;
use rmcp::model::ServerInfo;
use rmcp::model::Tool;
use rmcp::service::RequestContext;
use rmcp::service::RoleServer;
use serde::Deserialize;
use serde_json::Value;

const CODEX_NAV_SERVER_VERSION: &str = "0.1.0";

#[derive(Clone)]
pub struct NavMcpServer {
    tools: Arc<Vec<Tool>>,
}

impl NavMcpServer {
    pub fn new() -> Self {
        let tools = vec![
            Self::code_nav_init_tool(),
            Self::code_symbols_tool(),
            Self::code_query_tool(),
        ];
        Self {
            tools: Arc::new(tools),
        }
    }

    // ── Tool definitions ────────────────────────────────────────────

    fn code_nav_init_tool() -> Tool {
        #[expect(clippy::expect_used)]
        let schema: rmcp::model::JsonObject =
            serde_json::from_value(serde_json::json!({
            "type": "object",
            "properties": {
                "reset": {
                    "type": "boolean",
                    "description": "When true, deletes the existing index and rebuilds from scratch. Use this to recover from a corrupted index or after a large refactor. Defaults to false."
                }
            },
            "required": [],
            "additionalProperties": false
        }))
        .expect("code_nav_init schema");

        Tool::new(
            std::borrow::Cow::Borrowed("code_nav_init"),
            std::borrow::Cow::Borrowed(
                "Initialize or refresh the code-nav symbol index for the current working directory. \
                 Call this FIRST at the start of every session, before using code_symbols or \
                 code_query — it must be called before any code search or file exploration. \
                 Pass reset: true to rebuild the index from scratch if it is stale or corrupted.",
            ),
            Arc::new(schema),
        )
    }

    fn code_symbols_tool() -> Tool {
        #[expect(clippy::expect_used)]
        let schema: rmcp::model::JsonObject =
            serde_json::from_value(serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "File or directory to scan for symbols. Defaults to the session's working directory."
                },
                "lang": {
                    "type": "string",
                    "description": "Language to search. Auto-detected from file extension when omitted. One of: bash, c, cpp, go, javascript, python, rust, typescript."
                }
            },
            "required": [],
            "additionalProperties": false
        }))
        .expect("code_symbols schema");

        Tool::new(
            std::borrow::Cow::Borrowed("code_symbols"),
            std::borrow::Cow::Borrowed(
                "List all top-level symbols (functions, structs, classes, enums, traits, etc.) \
                 defined in a file or directory using tree-sitter AST parsing. Returns each symbol's \
                 name, kind, file path, and line number as JSON. \
                 Prefer this over reading or catting a file when you need to understand its structure \
                 or find what functions/types it defines.",
            ),
            Arc::new(schema),
        )
    }

    fn code_query_tool() -> Tool {
        #[expect(clippy::expect_used)]
        let schema: rmcp::model::JsonObject =
            serde_json::from_value(serde_json::json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Tree-sitter S-expression query. Use named captures to label results, e.g. `(function_item name: (identifier) @name)`."
                },
                "lang": {
                    "type": "string",
                    "description": "Language grammar to use. One of: bash, c, cpp, go, javascript, python, rust, typescript."
                },
                "path": {
                    "type": "string",
                    "description": "File or directory to search. Defaults to the session's working directory."
                }
            },
            "required": ["query", "lang"],
            "additionalProperties": false
        }))
        .expect("code_query schema");

        Tool::new(
            std::borrow::Cow::Borrowed("code_query"),
            std::borrow::Cow::Borrowed(
                "Run an arbitrary tree-sitter S-expression query against source files, returning \
                 each capture's text, file path, and line numbers as JSON. Supports all tree-sitter \
                 query predicates. Returns up to 500 matches.",
            ),
            Arc::new(schema),
        )
    }

    // ── Helpers ─────────────────────────────────────────────────────

    /// Resolve the working directory. Use `CODE_NAV_CWD` env var if set,
    /// otherwise fall back to the current process working directory.
    fn resolve_cwd() -> std::path::PathBuf {
        match std::env::var("CODE_NAV_CWD") {
            Ok(dir) => std::path::PathBuf::from(dir),
            Err(_) => std::env::current_dir().unwrap_or_default(),
        }
    }

    fn parse_args<T: for<'de> Deserialize<'de>>(
        request: &CallToolRequestParam,
        tool_name: &str,
    ) -> Result<T, McpError> {
        match request.arguments.as_ref() {
            Some(args) => {
                let obj: HashMap<String, Value> = args
                    .iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect();
                serde_json::from_value(Value::Object(obj.into_iter().collect())).map_err(|e| {
                    McpError::invalid_params(format!("invalid arguments for {tool_name}: {e}"), None)
                })
            }
            None => Err(McpError::invalid_params(
                format!("missing arguments for {tool_name}"),
                None,
            )),
        }
    }
}

// ── Internal handlers (not part of the trait) ─────────────────────

async fn handle_code_nav_init(request: &CallToolRequestParam) -> Result<CallToolResult, McpError> {
    #[derive(Deserialize)]
    struct CodeNavInitArgs {
        #[serde(default)]
        reset: bool,
    }

    let args: CodeNavInitArgs = NavMcpServer::parse_args(request, "code_nav_init")?;

    let cwd = NavMcpServer::resolve_cwd();
    let root = find_project_root(&cwd);

    if args.reset {
        NavIndex::reset(&root).await.map_err(|e| {
            McpError::internal_error(format!("code_nav_init reset failed: {e}"), None)
        })?;
    }

    NavIndex::warm(&cwd, None).await.map_err(|e| {
        McpError::internal_error(format!("code_nav_init warm failed: {e}"), None)
    })?;

    let msg = if args.reset {
        "Index reset and rebuilt successfully."
    } else {
        "Index is up to date."
    };

    Ok(CallToolResult::success(vec![rmcp::model::Content::text(msg)]))
}

async fn handle_code_symbols(request: &CallToolRequestParam) -> Result<CallToolResult, McpError> {
    #[derive(Deserialize)]
    struct CodeSymbolsArgs {
        path: Option<String>,
        lang: Option<String>,
    }

    let args: CodeSymbolsArgs = NavMcpServer::parse_args(request, "code_symbols")?;

    let cwd = NavMcpServer::resolve_cwd();
    let search_path = match &args.path {
        Some(p) => {
            let p = std::path::PathBuf::from(p);
            if p.is_absolute() {
                p
            } else {
                cwd.join(p)
            }
        }
        None => cwd,
    };
    let lang_filter = args.lang.as_deref().and_then(Lang::from_str);

    // Step 1: find project root
    let root = find_project_root(&search_path);

    // Step 2: open (or create) the index
    let index = NavIndex::open(&root).await.map_err(|e| {
        McpError::internal_error(format!("failed to open code-nav index: {e}"), None)
    })?;

    // Step 3: get cached mtimes for the search prefix
    let prefix = search_path.to_string_lossy().into_owned();
    let cached_mtimes = index.get_cached_mtimes(&prefix).await.map_err(|e| {
        McpError::internal_error(format!("failed to read index mtimes: {e}"), None)
    })?;

    // Step 4 & 5: walk & parse stale files (CPU-bound)
    let search_path_clone = search_path.clone();
    let (freshly_parsed, existing_paths) =
        tokio::task::spawn_blocking(move || -> anyhow::Result<_> {
            let (stale, existing) =
                scan_for_changes(&search_path_clone, lang_filter, &cached_mtimes)?;
            let parsed = run_symbols_for_files(&stale)?;
            Ok((parsed, existing))
        })
        .await
        .map_err(|e| McpError::internal_error(format!("symbols task panicked: {e}"), None))?
        .map_err(|e| McpError::internal_error(format!("symbols scan failed: {e}"), None))?;

    // Step 6: update index for each re-parsed file
    for (file_path, mtime, symbols) in &freshly_parsed {
        let ext = file_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");
        if let Some(lang) = Lang::from_extension(ext) {
            index
                .update_file(file_path, lang, *mtime, symbols)
                .await
                .map_err(|e| {
                    McpError::internal_error(format!("index update failed: {e}"), None)
                })?;
        }
    }

    // Step 7: prune deleted files
    index
        .remove_deleted_files(&existing_paths)
        .await
        .map_err(|e| McpError::internal_error(format!("index prune failed: {e}"), None))?;

    // Step 8: return all symbols from the index
    let symbols = index
        .get_symbols_for_prefix(&prefix)
        .await
        .map_err(|e| McpError::internal_error(format!("failed to query index: {e}"), None))?;

    if symbols.is_empty() {
        return Ok(CallToolResult::success(vec![
            rmcp::model::Content::text("No symbols found."),
        ]));
    }

    let content = serde_json::to_string_pretty(&symbols)
        .map_err(|e| McpError::internal_error(format!("failed to serialize symbols: {e}"), None))?;

    Ok(CallToolResult::success(vec![rmcp::model::Content::text(
        content,
    )]))
}

async fn handle_code_query(request: &CallToolRequestParam) -> Result<CallToolResult, McpError> {
    #[derive(Deserialize)]
    struct CodeQueryArgs {
        query: String,
        lang: String,
        path: Option<String>,
    }

    let args: CodeQueryArgs = NavMcpServer::parse_args(request, "code_query")?;

    let lang = Lang::from_str(&args.lang).ok_or_else(|| {
        McpError::invalid_params(
            format!(
                "unknown language {:?}; supported: bash, c, cpp, go, javascript, python, rust, typescript",
                args.lang
            ),
            None,
        )
    })?;

    let cwd = NavMcpServer::resolve_cwd();
    let search_path = match &args.path {
        Some(p) => {
            let p = std::path::PathBuf::from(p);
            if p.is_absolute() {
                p
            } else {
                cwd.join(p)
            }
        }
        None => cwd,
    };

    let query_str = args.query;
    let matches =
        tokio::task::spawn_blocking(move || run_query(&query_str, lang, &search_path))
            .await
            .map_err(|e| McpError::internal_error(format!("query task panicked: {e}"), None))?
            .map_err(|e| McpError::internal_error(format!("query failed: {e}"), None))?;

    if matches.is_empty() {
        return Ok(CallToolResult::success(vec![
            rmcp::model::Content::text("No matches found."),
        ]));
    }

    let content = serde_json::to_string_pretty(&matches)
        .map_err(|e| McpError::internal_error(format!("failed to serialize matches: {e}"), None))?;

    Ok(CallToolResult::success(vec![rmcp::model::Content::text(
        content,
    )]))
}

// ── ServerHandler trait implementation ────────────────────────────

impl ServerHandler for NavMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: Default::default(),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            server_info: rmcp::model::Implementation {
                name: "codex-nav-mcp-server".into(),
                version: CODEX_NAV_SERVER_VERSION.into(),
                ..Default::default()
            },
            instructions: None,
        }
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParam>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        Ok(ListToolsResult {
            tools: self.tools.as_ref().clone(),
            next_cursor: None,
            meta: None,
        })
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParam,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        match request.name.as_ref() {
            "code_nav_init" => handle_code_nav_init(&request).await,
            "code_symbols" => handle_code_symbols(&request).await,
            "code_query" => handle_code_query(&request).await,
            other => Err(McpError::invalid_params(
                format!("unknown tool: {other}"),
                None,
            )),
        }
    }
}

// ════════════════════════════════════════════════════════════════════
// Tests
// ════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use rmcp::model::JsonObject;

    fn make_call(name: &str, args: serde_json::Value) -> CallToolRequestParam {
        let args_map: JsonObject = match args {
            serde_json::Value::Object(map) => map.into_iter().collect(),
            _ => JsonObject::new(),
        };
        CallToolRequestParam {
            name: std::borrow::Cow::Owned(name.to_string()),
            arguments: Some(args_map),
        }
    }

    fn assert_success_text(result: CallToolResult) -> String {
        assert!(
            result.is_error != Some(true),
            "Expected success, got error, content={:?}",
            result.content
        );
        assert!(!result.content.is_empty(), "Expected non-empty content");
        result
            .content
            .iter()
            .filter_map(|c| {
                if let rmcp::model::RawContent::Text(text) = &c.raw {
                    Some(text.text.clone())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Test the handler functions directly (they're private but accessible from
    /// this module since it's inside server.rs).
    #[serial_test::serial]
    #[tokio::test]
    async fn test_code_nav_init_and_symbols() {
        let tmp = tempfile::tempdir().expect("temp dir");
        let project_dir = tmp.path().join("proj");
        std::fs::create_dir_all(project_dir.join("src")).expect("create src");
        std::fs::write(
            project_dir.join("src/lib.rs"),
            r#"
pub fn greet(name: &str) -> String { format!("Hello, {name}!") }
pub struct Point { pub x: f64, pub y: f64 }
"#,
        )
        .expect("write file");

        let orig_cwd = std::env::current_dir().expect("get cwd");
        std::env::set_current_dir(&project_dir).expect("set cwd");

        // Use reset=true to ensure a clean index
        let result = handle_code_nav_init(&make_call(
            "code_nav_init",
            serde_json::json!({"reset": true}),
        ))
        .await
        .expect("code_nav_init");
        let text = assert_success_text(result);
        assert!(!text.is_empty(), "Expected init response");

        // code_symbols
        let sym = handle_code_symbols(&make_call(
            "code_symbols",
            serde_json::json!({"path": "src/lib.rs"}),
        ))
        .await
        .expect("code_symbols");
        let sym_text = assert_success_text(sym);
        assert!(
            sym_text.contains("greet") || sym_text.contains("Point"),
            "Expected symbols containing greet/Point, got: {sym_text}"
        );

        std::env::set_current_dir(&orig_cwd).expect("restore cwd");
    }

    #[serial_test::serial]
    #[tokio::test]
    async fn test_code_query() {
        let tmp = tempfile::tempdir().expect("temp dir");
        let project_dir = tmp.path().join("proj");
        std::fs::create_dir_all(project_dir.join("src")).expect("create src");
        std::fs::write(
            project_dir.join("src/lib.rs"),
            r#"
fn helper() -> i32 { 42 }
pub fn process() -> i32 { helper() }
"#,
        )
        .expect("write file");

        let orig_cwd = std::env::current_dir().expect("get cwd");
        std::env::set_current_dir(&project_dir).expect("set cwd");

        // Init index first with reset to ensure clean index
        handle_code_nav_init(&make_call(
            "code_nav_init",
            serde_json::json!({"reset": true}),
        ))
        .await
        .expect("init");

        // Query for function definitions
        let result = handle_code_query(&make_call(
            "code_query",
            serde_json::json!({
                "query": "(function_item name: (identifier) @name)",
                "lang": "rust",
                "path": "src/lib.rs"
            }),
        ))
        .await
        .expect("code_query");
        let text = assert_success_text(result);
        assert!(
            text.contains("helper") || text.contains("process"),
            "Expected function names, got: {text}"
        );

        // No-match query — should get a friendly message
        let no_match = handle_code_query(&make_call(
            "code_query",
            serde_json::json!({
                "query": "(struct_item name: (type_identifier) @name)",
                "lang": "rust",
                "path": "src/lib.rs"
            }),
        ))
        .await
        .expect("code_query no match");
        let no_text = assert_success_text(no_match);
        assert!(
            no_text.to_lowercase().contains("no match"),
            "Expected 'No matches found', got: {no_text}"
        );

        std::env::set_current_dir(&orig_cwd).expect("restore cwd");
    }

    #[serial_test::serial]
    #[tokio::test]
    async fn test_code_nav_reset_rebuilds_index() {
        let tmp = tempfile::tempdir().expect("temp dir");
        let orig_cwd = std::env::current_dir().expect("get cwd");
        std::env::set_current_dir(tmp.path()).expect("set cwd");

        handle_code_nav_init(&make_call("code_nav_init", serde_json::json!({})))
            .await
            .expect("init");

        let result = handle_code_nav_init(&make_call(
            "code_nav_init",
            serde_json::json!({"reset": true}),
        ))
        .await
        .expect("reset");
        let text = assert_success_text(result);
        assert!(!text.is_empty(), "Expected reset response");

        std::env::set_current_dir(&orig_cwd).expect("restore cwd");
    }

    #[tokio::test]
    async fn test_parse_args_valid() {
        let call = make_call("test", serde_json::json!({"path": "src"}));
        #[derive(serde::Deserialize)]
        struct Args { path: Option<String> }
        let args: Args = NavMcpServer::parse_args(&call, "test").expect("parse");
        assert_eq!(args.path, Some("src".to_string()));
    }

    #[tokio::test]
    async fn test_parse_args_missing() {
        let call = make_call("test", serde_json::json!({"query": "hello"}));
        #[derive(serde::Deserialize)]
        struct Args { path: String }
        let result: Result<Args, _> = NavMcpServer::parse_args(&call, "test");
        assert!(result.is_err(), "Expected error for missing required field");
    }
}
