# codex-nav-mcp-server

An MCP (Model Context Protocol) server that exposes tree-sitter-based code navigation as tools for AI assistants. Instead of shelling out to `grep`/`cat`/`find`, AI agents can use three purpose-built tools to explore codebases via AST-level queries.

## Tools

### `code_nav_init`

Initializes or refreshes the tree-sitter symbol index for the current working directory. Must be called once at the start of a session before using `code_symbols` or `code_query`.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `reset` | boolean | no | Delete existing index and rebuild from scratch. Use after a large refactor or if the index is stale. |

### `code_symbols`

Lists all top-level symbols (functions, structs, classes, enums, traits, etc.) defined in a file or directory. Returns each symbol's name, kind, file path, and line number as JSON.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `path` | string | no | File or directory to scan. Defaults to the working directory. |
| `lang` | string | no | Language to search. Auto-detected from file extension when omitted. One of: `bash`, `c`, `cpp`, `go`, `javascript`, `python`, `rust`, `swift`, `typescript`. |

### `code_query`

Runs an arbitrary tree-sitter S-expression query against source files, returning up to 500 matches with file paths and line numbers.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `query` | string | yes | Tree-sitter S-expression query with named captures (e.g. `(function_item name: (identifier) @name)`). |
| `lang` | string | yes | Language grammar to use (same options as `code_symbols`: `bash`, `c`, `cpp`, `go`, `javascript`, `python`, `rust`, `swift`, `typescript`). |
| `path` | string | no | File or directory to search. Defaults to the working directory. |

## Prompt

The server also exposes an MCP **prompt**, `codex_nav_code_search`, containing guidance on when and how to reach for these tools instead of `grep`/`cat`/`find`. Hosts that surface prompts (e.g. via a slash-command picker) can load it directly; the same text is also sent as the server's `instructions` during initialization.

## Usage

The server communicates over **stdio transport**, making it compatible with any MCP host (Claude Desktop, VS Code extensions, CLI tools, etc.).

### Choosing the directory to index

The server indexes a single working directory, resolved as follows:

1. The `CODE_NAV_CWD` environment variable, if set.
2. Otherwise, the process's current working directory.

Because MCP hosts typically launch the server with their own (often unrelated) working directory, **set `CODE_NAV_CWD` to the absolute path of the project you want to index**. All `path` arguments are resolved relative to this directory.

### Build from source

```sh
cargo build --release
```

The binary is at `target/release/codex-nav-mcp-server`.

### Claude Desktop

Add to your `claude_desktop_config.json` (`~/Library/Application Support/Claude/claude_desktop_config.json` on macOS):

```json
{
  "mcpServers": {
    "codex-nav": {
      "command": "/absolute/path/to/codex-nav-mcp-server",
      "args": [],
      "env": {
        "CODE_NAV_CWD": "/absolute/path/to/your/project"
      }
    }
  }
}
```

Replace `/absolute/path/to/codex-nav-mcp-server` with the path to the binary (e.g. `target/release/codex-nav-mcp-server` from the project root), and set `CODE_NAV_CWD` to the project you want to index (see [Choosing the directory to index](#choosing-the-directory-to-index)).

### VS Code (Cline / Roo Code / etc.)

Add to your MCP settings file (e.g. `.vscode/mcp.json` or the extension's global settings):

```json
{
  "servers": {
    "codex-nav": {
      "type": "stdio",
      "command": "/absolute/path/to/codex-nav-mcp-server",
      "args": []
    }
  }
}
```

### Codex CLI

Codex CLI discovers MCP servers from a config file. Add an entry:

```json
{
  "mcpServers": {
    "codex-nav": {
      "command": "/absolute/path/to/codex-nav-mcp-server",
      "args": []
    }
  }
}
```

### Generic MCP host

Any host that supports stdio-based MCP servers can use:

```
command: codex-nav-mcp-server
args: (none)
transport: stdio
```

### Running directly (stdio)

```sh
cargo run
```

The server listens on stdin and writes JSON-RPC messages to stdout. Logs are written to stderr, so they don't interfere with the protocol.

### Running tests

```sh
cargo test
```

## Architecture

```
┌─────────────────┐     stdio      ┌──────────────────────────────┐
│   MCP Host      │ ◄────────── ►  │   codex-nav-mcp-server       │
│  (Claude, etc.) │                │                              │
│                 │                │  ┌────────────────────────┐  │
│                 │                │  │  NavMcpServer          │  │
│                 │                │  │  ├─ code_nav_init()    │  │
│                 │                │  │  ├─ code_symbols()     │  │
│                 │                │  │  └─ code_query()       │  │
│                 │                │  │                        │  │
│                 │                │  │  ┌────────────────┐    │  │
│                 │                │  │  │ codex-code-nav │    │  │
│                 │                │  │  │ (tree-sitter)  │    │  │
│                 │                │  │  └────────────────┘    │  │
│                 │                │  └────────────────────────┘  │
└─────────────────┘                └──────────────────────────────┘
```

## Dependencies

- **[rmcp](https://crates.io/crates/rmcp)** — Rust MCP protocol implementation (transport, macros, server)
- **codex-code-nav** — Tree-sitter indexing and query engine. Local path dependency, vendored in [`crates/code-nav`](crates/code-nav).
- **tokio** — Async runtime
- **serde** / **serde_json** — Argument parsing and output formatting
- **tracing** / **tracing-subscriber** — Structured logging (to stderr)

## Requirements

- Rust 1.93.0 (as specified in `rust-toolchain.toml`)
- Components: `clippy`, `rustfmt`, `rust-src`

## Supported languages

`bash`, `c`, `cpp`, `go`, `javascript`, `python`, `rust`, `swift`, `typescript`

## License

MIT — see [LICENSE](LICENSE).
