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
| `lang` | string | no | Language to search. Auto-detected from file extension when omitted. One of: `bash`, `c`, `cpp`, `go`, `javascript`, `python`, `rust`, `typescript`. |

### `code_query`

Runs an arbitrary tree-sitter S-expression query against source files, returning up to 500 matches with file paths and line numbers.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `query` | string | yes | Tree-sitter S-expression query with named captures (e.g. `(function_item name: (identifier) @name)`). |
| `lang` | string | yes | Language grammar to use (same options as `code_symbols`). |
| `path` | string | no | File or directory to search. Defaults to the working directory. |

## Usage

The server communicates over **stdio transport**, making it compatible with any MCP host (Claude Desktop, VS Code extensions, CLI tools, etc.).

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
      "args": []
    }
  }
}
```

Replace `/absolute/path/to/codex-nav-mcp-server` with the path to the binary (e.g. `target/release/codex-nav-mcp-server` from the project root).

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
- **[codex-code-nav](https://github.com/nidex/codex-rs)** — Tree-sitter indexing and query engine (local dependency)
- **tokio** — Async runtime
- **serde** / **serde_json** — Argument parsing and output formatting
- **tracing** / **tracing-subscriber** — Structured logging (to stderr)

## Requirements

- Rust 1.93.0 (as specified in `rust-toolchain.toml`)
- Components: `clippy`, `rustfmt`, `rust-src`

## Supported languages

`bash`, `c`, `cpp`, `go`, `javascript`, `python`, `rust`, `typescript`

## License

Apache-2.0
