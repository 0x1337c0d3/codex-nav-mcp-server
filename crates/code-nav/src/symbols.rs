use std::path::Path;
use std::path::PathBuf;
use std::time::SystemTime;

use anyhow::Result;
use serde::Serialize;

use crate::language::Lang;
use crate::query::run_query;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SymbolKind {
    Fn,
    Struct,
    Union,
    Enum,
    Trait,
    Mod,
    Type,
    Const,
    Static,
    Macro,
    Class,
    Method,
    Interface,
    Namespace,
    Unknown,
}

impl SymbolKind {
    fn from_capture_prefix(prefix: &str) -> Self {
        match prefix {
            "fn" => Self::Fn,
            "struct" => Self::Struct,
            "union" => Self::Union,
            "enum" => Self::Enum,
            "trait" => Self::Trait,
            "mod" => Self::Mod,
            "type" => Self::Type,
            "const" => Self::Const,
            "static" => Self::Static,
            "macro" => Self::Macro,
            "class" => Self::Class,
            "method" => Self::Method,
            "interface" => Self::Interface,
            "namespace" => Self::Namespace,
            _ => Self::Unknown,
        }
    }

    /// Round-trip string form used for SQLite storage.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Fn => "fn",
            Self::Struct => "struct",
            Self::Union => "union",
            Self::Enum => "enum",
            Self::Trait => "trait",
            Self::Mod => "mod",
            Self::Type => "type",
            Self::Const => "const",
            Self::Static => "static",
            Self::Macro => "macro",
            Self::Class => "class",
            Self::Method => "method",
            Self::Interface => "interface",
            Self::Namespace => "namespace",
            Self::Unknown => "unknown",
        }
    }

    /// Deserialize from the string stored in SQLite.
    pub fn from_str(s: &str) -> Self {
        Self::from_capture_prefix(s)
    }
}

#[derive(Debug, Serialize)]
pub struct Symbol {
    pub name: String,
    pub kind: SymbolKind,
    pub file: String,
    pub line: usize,
}

/// Returns the tree-sitter query for extracting top-level symbols from a language.
///
/// Capture names follow the convention `<kind>.name` so the caller can derive
/// `SymbolKind` from the prefix before the first `.`.
fn symbols_query(lang: Lang) -> &'static str {
    match lang {
        Lang::Rust => {
            "(source_file
               (function_item name: (identifier) @fn.name))
             (mod_item
               body: (declaration_list
                 (function_item name: (identifier) @fn.name)))
             (impl_item
               body: (declaration_list
                 (function_item name: (identifier) @method.name)))
             (trait_item
               body: (declaration_list
                 (function_item name: (identifier) @method.name)))
             (foreign_mod_item
               body: (declaration_list
                 (function_signature_item name: (identifier) @fn.name)))
             (trait_item
               body: (declaration_list
                 (function_signature_item name: (identifier) @method.name)))
             (struct_item name: (type_identifier) @struct.name)
             (union_item name: (type_identifier) @union.name)
             (enum_item name: (type_identifier) @enum.name)
             (trait_item name: (type_identifier) @trait.name)
             (mod_item name: (identifier) @mod.name)
             (type_item name: (type_identifier) @type.name)
             (const_item name: (identifier) @const.name)
             (static_item name: (identifier) @static.name)
             (macro_definition name: (identifier) @macro.name)"
        }
        Lang::Python => {
            "(module
               (function_definition name: (identifier) @fn.name))
             (module
               (decorated_definition
                 definition: (function_definition name: (identifier) @fn.name)))
             (class_definition
               body: (block
                 (function_definition name: (identifier) @method.name)))
             (class_definition
               body: (block
                 (decorated_definition
                   definition: (function_definition name: (identifier) @method.name))))
             (class_definition name: (identifier) @class.name)"
        }
        Lang::JavaScript => {
            "(function_declaration name: (identifier) @fn.name)
             (generator_function_declaration name: (identifier) @fn.name)
             (function_expression name: (identifier) @fn.name)
             (generator_function name: (identifier) @fn.name)
             (lexical_declaration
               (variable_declarator
                 name: (identifier) @fn.name
                 value: [(arrow_function) (function_expression)]))
             (variable_declaration
               (variable_declarator
                 name: (identifier) @fn.name
                 value: [(arrow_function) (function_expression)]))
             (class_declaration name: (identifier) @class.name)
             (method_definition name: (property_identifier) @method.name)"
        }
        Lang::TypeScript => {
            "(function_declaration name: (identifier) @fn.name)
             (function_signature name: (identifier) @fn.name)
             (generator_function_declaration name: (identifier) @fn.name)
             (function_expression name: (identifier) @fn.name)
             (generator_function name: (identifier) @fn.name)
             (lexical_declaration
               (variable_declarator
                 name: (identifier) @fn.name
                 value: [(arrow_function) (function_expression)]))
             (variable_declaration
               (variable_declarator
                 name: (identifier) @fn.name
                 value: [(arrow_function) (function_expression)]))
             (class_declaration name: (type_identifier) @class.name)
             (abstract_class_declaration name: (type_identifier) @class.name)
             (method_definition name: (property_identifier) @method.name)
             (method_signature name: (property_identifier) @method.name)
             (abstract_method_signature name: (property_identifier) @method.name)
             (interface_declaration name: (type_identifier) @interface.name)
             (type_alias_declaration name: (type_identifier) @type.name)
             (enum_declaration name: (identifier) @enum.name)
             (internal_module name: (_) @namespace.name)"
        }
        Lang::Go => {
            "(function_declaration name: (identifier) @fn.name)
             (method_declaration name: (field_identifier) @method.name)
             (method_elem name: (field_identifier) @method.name)
             (type_spec name: (type_identifier) @type.name)
             (type_alias name: (type_identifier) @type.name)"
        }
        Lang::C => {
            "(function_declarator declarator: (identifier) @fn.name)
             (struct_specifier name: (type_identifier) @struct.name)
             (union_specifier name: (type_identifier) @union.name)
             (enum_specifier name: (type_identifier) @enum.name)
             (type_definition declarator: (type_identifier) @type.name)"
        }
        Lang::Cpp => {
            "(function_declarator declarator: (identifier) @fn.name)
             (function_declarator declarator: (field_identifier) @method.name)
             (function_declarator declarator: (qualified_identifier) @method.name)
             (class_specifier name: (type_identifier) @class.name)
             (struct_specifier name: (type_identifier) @struct.name)
             (union_specifier name: (type_identifier) @union.name)
             (enum_specifier name: (type_identifier) @enum.name)
             (namespace_definition name: (namespace_identifier) @namespace.name)
             (type_definition declarator: (type_identifier) @type.name)
             (alias_declaration name: (type_identifier) @type.name)"
        }
        Lang::Swift => {
            "(class_declaration
               declaration_kind: \"struct\"
               name: (type_identifier) @struct.name)
             (class_declaration
               declaration_kind: \"class\"
               name: (type_identifier) @class.name)
             (class_declaration
               declaration_kind: \"actor\"
               name: (type_identifier) @class.name)
             (class_declaration
               declaration_kind: \"enum\"
               name: (type_identifier) @enum.name)
             (class_declaration
               declaration_kind: \"extension\"
               name: (_) @type.name)
             (function_declaration
               (simple_identifier) @fn.name)
             (protocol_declaration
               name: (type_identifier) @interface.name)
             (typealias_declaration
               name: (type_identifier) @type.name)"
        }
        Lang::Bash => "(function_definition name: (word) @fn.name)",
    }
}

/// List all top-level symbols in a file or directory.
pub fn run_symbols(path: &Path, lang: Option<Lang>) -> Result<Vec<Symbol>> {
    let is_file = path.metadata().map(|m| m.is_file()).unwrap_or(false);

    let langs: Vec<Lang> = if let Some(l) = lang {
        vec![l]
    } else if is_file {
        // Auto-detect language from the single file's extension.
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        match Lang::from_extension(ext) {
            Some(l) => vec![l],
            None => return Ok(Vec::new()),
        }
    } else {
        // Walk directory and query all supported languages.
        vec![
            Lang::Bash,
            Lang::C,
            Lang::Cpp,
            Lang::Go,
            Lang::JavaScript,
            Lang::Python,
            Lang::Rust,
            Lang::Swift,
            Lang::TypeScript,
        ]
    };

    let mut symbols: Vec<Symbol> = Vec::new();

    for l in langs {
        let query_str = symbols_query(l);
        let matches = run_query(query_str, l, path)?;
        for m in matches {
            // Capture name is "<kind>.name"; strip the leading `@` added by run_query.
            let cap = m.capture.trim_start_matches('@');
            let kind_prefix = cap.split('.').next().unwrap_or("unknown");
            symbols.push(Symbol {
                name: m.text,
                kind: SymbolKind::from_capture_prefix(kind_prefix),
                file: m.file,
                line: m.start_line,
            });
        }
    }

    symbols.sort_by(|a, b| {
        a.file
            .cmp(&b.file)
            .then(a.line.cmp(&b.line))
            .then(a.name.cmp(&b.name))
            .then(symbol_kind_priority(&b.kind).cmp(&symbol_kind_priority(&a.kind)))
    });
    symbols.dedup_by(|a, b| a.file == b.file && a.line == b.line && a.name == b.name);
    Ok(symbols)
}

fn symbol_kind_priority(kind: &SymbolKind) -> u8 {
    match kind {
        SymbolKind::Method => 1,
        _ => 0,
    }
}

/// Parse symbols from a specific list of files (already identified as stale/new).
///
/// Returns `(path, mtime, symbols)` for each file that could be parsed.
/// Runs synchronously; call inside `tokio::task::spawn_blocking`.
pub fn run_symbols_for_files(
    files: &[(PathBuf, Lang, SystemTime)],
) -> Result<Vec<(PathBuf, SystemTime, Vec<Symbol>)>> {
    let mut out = Vec::with_capacity(files.len());
    for (path, lang, mtime) in files {
        let syms = run_symbols(path, Some(*lang))?;
        out.push((path.clone(), *mtime, syms));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    fn assert_symbol(symbols: &[Symbol], name: &str, kind: SymbolKind) {
        assert!(
            symbols
                .iter()
                .any(|symbol| symbol.name == name && symbol.kind == kind),
            "expected {kind:?} {name} in {:?}",
            symbols
                .iter()
                .map(|symbol| (&symbol.kind, &symbol.name))
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn symbol_kind_union_round_trips() {
        assert_eq!(
            SymbolKind::from_str(SymbolKind::Union.as_str()),
            SymbolKind::Union
        );
    }

    #[test]
    fn symbols_bash_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("sample.sh");
        std::fs::write(&path, "hello() { :; }\nfunction world { :; }\n").expect("write");

        let syms = run_symbols(&path, None).expect("run_symbols");
        assert_symbol(&syms, "hello", SymbolKind::Fn);
        assert_symbol(&syms, "world", SymbolKind::Fn);
    }

    #[test]
    fn symbols_rust_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("sample.rs");
        let mut f = std::fs::File::create(&path).expect("create");
        write!(
            f,
            "pub struct Foo;\npub union Value {{ integer: u32, float: f32 }}\n\
             pub fn bar() {{}}\npub enum Baz {{ Ready }}\npub trait Runner {{ fn run(); }}\n\
             impl Foo {{ fn method() {{}} }}\nextern \"C\" {{ fn external(); }}\n"
        )
        .expect("write");

        let syms = run_symbols(&path, None).expect("run_symbols");
        assert_symbol(&syms, "Foo", SymbolKind::Struct);
        assert_symbol(&syms, "Value", SymbolKind::Union);
        assert_symbol(&syms, "bar", SymbolKind::Fn);
        assert_symbol(&syms, "Baz", SymbolKind::Enum);
        assert_symbol(&syms, "Runner", SymbolKind::Trait);
        assert_symbol(&syms, "run", SymbolKind::Method);
        assert_symbol(&syms, "method", SymbolKind::Method);
        assert_symbol(&syms, "external", SymbolKind::Fn);
    }

    #[test]
    fn symbols_python_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("sample.py");
        let mut f = std::fs::File::create(&path).expect("create");
        write!(
            f,
            "def greet(name):\n    pass\n\nasync def fetch():\n    pass\n\n\
             class Dog:\n    def speak(self):\n        pass\n"
        )
        .expect("write");

        let syms = run_symbols(&path, None).expect("run_symbols");
        assert_symbol(&syms, "greet", SymbolKind::Fn);
        assert_symbol(&syms, "fetch", SymbolKind::Fn);
        assert_symbol(&syms, "Dog", SymbolKind::Class);
        assert_symbol(&syms, "speak", SymbolKind::Method);
    }

    #[test]
    fn symbols_c_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("sample.c");
        std::fs::write(
            &path,
            "int plain(void) { return 0; }\nint *pointer_return(void) { return 0; }\n\
             struct Record {};\nunion Value { int integer; float decimal; };\n\
             enum State { Ready };\ntypedef unsigned long Size;\n\
             typedef struct { int x; } AnonymousRecord;\n",
        )
        .expect("write");

        let syms = run_symbols(&path, None).expect("run_symbols");
        assert_symbol(&syms, "plain", SymbolKind::Fn);
        assert_symbol(&syms, "pointer_return", SymbolKind::Fn);
        assert_symbol(&syms, "Record", SymbolKind::Struct);
        assert_symbol(&syms, "Value", SymbolKind::Union);
        assert_symbol(&syms, "State", SymbolKind::Enum);
        assert_symbol(&syms, "Size", SymbolKind::Type);
        assert_symbol(&syms, "AnonymousRecord", SymbolKind::Type);
    }

    #[test]
    fn symbols_cpp_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("sample.cpp");
        std::fs::write(
            &path,
            "namespace Demo {}\nclass Widget { public: void method(); };\n\
             struct Record {};\nunion Value { int integer; float decimal; };\n\
             enum class State { Ready };\nvoid free_function() {}\n\
             void Widget::method() {}\nusing Size = unsigned long;\n\
             template <typename T> T identity(T value) { return value; }\n",
        )
        .expect("write");

        let syms = run_symbols(&path, None).expect("run_symbols");
        assert_symbol(&syms, "Demo", SymbolKind::Namespace);
        assert_symbol(&syms, "Widget", SymbolKind::Class);
        assert_symbol(&syms, "method", SymbolKind::Method);
        assert_symbol(&syms, "Widget::method", SymbolKind::Method);
        assert_symbol(&syms, "Record", SymbolKind::Struct);
        assert_symbol(&syms, "Value", SymbolKind::Union);
        assert_symbol(&syms, "State", SymbolKind::Enum);
        assert_symbol(&syms, "free_function", SymbolKind::Fn);
        assert_symbol(&syms, "Size", SymbolKind::Type);
        assert_symbol(&syms, "identity", SymbolKind::Fn);
    }

    #[test]
    fn symbols_go_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("sample.go");
        std::fs::write(
            &path,
            "package sample\ntype Record struct{}\ntype Runner interface { Run() }\n\
             type Count = int\nfunc Free() {}\nfunc (Record) Method() {}\n",
        )
        .expect("write");

        let syms = run_symbols(&path, None).expect("run_symbols");
        assert_symbol(&syms, "Record", SymbolKind::Type);
        assert_symbol(&syms, "Runner", SymbolKind::Type);
        assert_symbol(&syms, "Run", SymbolKind::Method);
        assert_symbol(&syms, "Count", SymbolKind::Type);
        assert_symbol(&syms, "Free", SymbolKind::Fn);
        assert_symbol(&syms, "Method", SymbolKind::Method);
    }

    #[test]
    fn symbols_javascript_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("sample.js");
        std::fs::write(
            &path,
            "function declared() {}\nfunction* generated() {}\n\
             class Widget { method() {} }\nconst arrow = () => {};\n\
             const expression = function namedExpression() {};\n",
        )
        .expect("write");

        let syms = run_symbols(&path, None).expect("run_symbols");
        assert_symbol(&syms, "declared", SymbolKind::Fn);
        assert_symbol(&syms, "generated", SymbolKind::Fn);
        assert_symbol(&syms, "Widget", SymbolKind::Class);
        assert_symbol(&syms, "method", SymbolKind::Method);
        assert_symbol(&syms, "arrow", SymbolKind::Fn);
        assert_symbol(&syms, "expression", SymbolKind::Fn);
        assert_symbol(&syms, "namedExpression", SymbolKind::Fn);
    }

    #[test]
    fn symbols_typescript_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("sample.ts");
        std::fs::write(
            &path,
            "declare function declared(value: string): void;\n\
             abstract class AbstractWidget { abstract render(): void; }\n\
             interface Runner { run(): void }\ntype Size = number;\n\
             enum State { Ready }\nnamespace Demo {}\nconst arrow = (): void => {};\n",
        )
        .expect("write");

        let syms = run_symbols(&path, None).expect("run_symbols");
        assert_symbol(&syms, "declared", SymbolKind::Fn);
        assert_symbol(&syms, "AbstractWidget", SymbolKind::Class);
        assert_symbol(&syms, "render", SymbolKind::Method);
        assert_symbol(&syms, "Runner", SymbolKind::Interface);
        assert_symbol(&syms, "run", SymbolKind::Method);
        assert_symbol(&syms, "Size", SymbolKind::Type);
        assert_symbol(&syms, "State", SymbolKind::Enum);
        assert_symbol(&syms, "Demo", SymbolKind::Namespace);
        assert_symbol(&syms, "arrow", SymbolKind::Fn);
    }

    #[test]
    fn symbols_swift_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("sample.swift");
        let mut f = std::fs::File::create(&path).expect("create");
        write!(
            f,
            "struct Foo {{}}\nfunc bar() {{}}\nenum Baz {{}}\nclass Qux {{}}\n\
             actor Worker {{}}\nprotocol Runnable {{}}\ntypealias Identifier = String\n\
             extension ExternalType {{}}\n"
        )
        .expect("write");

        let syms = run_symbols(&path, None).expect("run_symbols");
        let names: Vec<&str> = syms.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"Foo"), "expected Foo in {names:?}");
        assert!(names.contains(&"bar"), "expected bar in {names:?}");
        assert!(names.contains(&"Baz"), "expected Baz in {names:?}");
        assert!(names.contains(&"Qux"), "expected Qux in {names:?}");
        assert!(names.contains(&"Worker"), "expected Worker in {names:?}");
        assert!(
            names.contains(&"Runnable"),
            "expected Runnable in {names:?}"
        );
        assert!(
            names.contains(&"Identifier"),
            "expected Identifier in {names:?}"
        );
        assert!(
            names.contains(&"ExternalType"),
            "expected extension ExternalType in {names:?}"
        );
    }
}
