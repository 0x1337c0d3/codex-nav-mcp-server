# Code Navigation Tools (MCP)

The codex-nav MCP server provides three source navigation tools:

- `code_nav_init`: initialize or refresh the symbol index for the current working directory.
- `code_symbols`: list top-level symbols in a file or directory using tree-sitter.
- `code_query`: run tree-sitter S-expression queries against source files.

Hosts may display these names with an MCP server prefix. The tool names provided by this server are the short names above.

This is structural source-code search, not embedding/vector semantic search. Use it when the question can be answered from syntax, definitions, symbols, call expressions, or other tree-sitter patterns.

## Default Workflow

Before the first source-code exploration in a session, call:

```text
code_nav_init()
```

Use `reset: true` only after a large refactor, suspected index corruption, or stale results.

For source code:

- Use `code_symbols(path="src/foo.rs")` to understand what a file defines.
- Use `code_symbols(path="src/")` to scan a module or directory.
- Use `code_query` to find definitions, calls, impls, or language syntax patterns.

For plain text, config files, logs, generated data, or a quick literal substring search, normal file or shell search tools are still appropriate.

## Query Selection

| Question | Prefer |
|----------|--------|
| What functions/types are in this file? | `code_symbols(path="file.rs")` |
| What is defined in this directory? | `code_symbols(path="src/")` |
| Where is `handle_event` defined? | `code_query` with a function/method definition query |
| Where is `handle_event` called? | `code_query` with a call expression query |
| Where is struct/class `Config` defined? | `code_query` with a type/class definition query |
| What implements trait/interface `Renderer`? | `code_query` with an impl/interface query |

If exact tree-sitter syntax is uncertain, first inspect symbols with `code_symbols`, then use a narrower `code_query` or fall back to literal text search.

## Rust Query Examples

```scheme
; Where is the function handle_event defined?
(function_item name: (identifier) @name (#eq? @name "handle_event")) @fn

; Where is handle_event called?
(call_expression function: (identifier) @fn (#eq? @fn "handle_event")) @call

; Where is handle_event called as a method?
(call_expression function: (field_expression field: (field_identifier) @method (#eq? @method "handle_event"))) @call

; Where is struct Config defined?
(struct_item name: (type_identifier) @name (#eq? @name "Config")) @struct

; Where is enum Status defined?
(enum_item name: (type_identifier) @name (#eq? @name "Status")) @enum

; What implements the Renderer trait?
(impl_item trait: (type_identifier) @trait (#eq? @trait "Renderer")) @impl
```

## Python Query Examples

```scheme
; Where is market_order() called?
(call function: (identifier) @fn (#eq? @fn "market_order")) @call

; Where is OrdersClient.market_order called as a method?
(call function: (attribute attribute: (identifier) @method (#eq? @method "market_order"))) @call

; Where is OrdersClient defined?
(class_definition name: (identifier) @name (#eq? @name "OrdersClient")) @class

; Where is market_order defined?
(function_definition name: (identifier) @name (#eq? @name "market_order")) @fn
```

Supported languages: `bash`, `c`, `cpp`, `go`, `javascript`, `python`, `rust`, `typescript`.

Use named captures such as `@fn`, `@call`, and `@name`; they make results easier to interpret.
