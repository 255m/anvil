use ratatui::style::Color;
use std::collections::{HashMap, HashSet};
use crate::config::Theme;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Language {
    Rust, Python, JavaScript, TypeScript, Go, C, Cpp,
    Shell, Markdown, Toml, Json, Yaml, Html, Css,
    Java, Kotlin, Ruby, Php, Swift, Lua, Sql,
    Zig, Nim, Haskell, Elixir, None,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TokenKind {
    Normal, Keyword, Type, String, Comment, Number, Function, Macro,
}

impl Language {
    pub fn name(&self) -> &'static str {
        match self {
            Language::Rust => "Rust", Language::Python => "Python",
            Language::JavaScript => "JavaScript", Language::TypeScript => "TypeScript",
            Language::Go => "Go", Language::C => "C", Language::Cpp => "C++",
            Language::Shell => "Shell", Language::Markdown => "Markdown",
            Language::Toml => "TOML", Language::Json => "JSON",
            Language::Yaml => "YAML", Language::Html => "HTML",
            Language::Css => "CSS", Language::Java => "Java",
            Language::Kotlin => "Kotlin", Language::Ruby => "Ruby",
            Language::Php => "PHP", Language::Swift => "Swift",
            Language::Lua => "Lua", Language::Sql => "SQL",
            Language::Zig => "Zig", Language::Nim => "Nim",
            Language::Haskell => "Haskell", Language::Elixir => "Elixir",
            Language::None => "Text",
        }
    }
}

pub fn detect(filename: &str) -> Language {
    let lower = filename.to_lowercase();
    let ext = lower.rsplit('.').next().unwrap_or("");
    if lower == "makefile" || lower == "dockerfile" { return Language::Shell; }
    if lower == "cargo.lock" { return Language::Toml; }
    match ext {
        "rs" => Language::Rust,
        "py" | "pyi" | "pyw" => Language::Python,
        "js" | "mjs" | "cjs" | "jsx" => Language::JavaScript,
        "ts" | "tsx" | "mts" | "cts" => Language::TypeScript,
        "go" => Language::Go,
        "c" | "h" => Language::C,
        "cpp" | "cc" | "cxx" | "c++" | "hpp" | "hxx" | "hh" => Language::Cpp,
        "sh" | "bash" | "zsh" | "fish" | "ksh" | "mk" => Language::Shell,
        "md" | "markdown" | "mdx" => Language::Markdown,
        "toml" => Language::Toml,
        "json" | "jsonc" => Language::Json,
        "yaml" | "yml" => Language::Yaml,
        "html" | "htm" | "xml" | "svg" => Language::Html,
        "css" | "scss" | "sass" | "less" => Language::Css,
        "java" => Language::Java,
        "kt" | "kts" => Language::Kotlin,
        "rb" => Language::Ruby,
        "php" | "phtml" => Language::Php,
        "swift" => Language::Swift,
        "lua" | "luau" => Language::Lua,
        "sql" => Language::Sql,
        "zig" => Language::Zig,
        "nim" => Language::Nim,
        "hs" | "lhs" => Language::Haskell,
        "ex" | "exs" => Language::Elixir,
        _ => Language::None,
    }
}

pub fn token_color(kind: TokenKind, theme: &Theme) -> Color {
    let is_light = is_light_bg(theme.bg);
    match kind {
        TokenKind::Normal => {
            if is_light { Color::Rgb(30, 30, 40) } else { Color::Rgb(230, 230, 240) }
        }
        TokenKind::Keyword => {
            if is_light { Color::Rgb(170, 30, 180) } else { Color::Rgb(255, 120, 220) }
        }
        TokenKind::Type => {
            if is_light { Color::Rgb(0, 130, 170) } else { Color::Rgb(110, 220, 255) }
        }
        TokenKind::String => {
            if is_light { Color::Rgb(150, 90, 0) } else { Color::Rgb(255, 220, 100) }
        }
        TokenKind::Comment => {
            if is_light { Color::Rgb(120, 130, 140) } else { Color::Rgb(130, 140, 160) }
        }
        TokenKind::Number => {
            if is_light { Color::Rgb(200, 80, 30) } else { Color::Rgb(255, 160, 100) }
        }
        TokenKind::Function => {
            if is_light { Color::Rgb(20, 90, 200) } else { Color::Rgb(140, 200, 255) }
        }
        TokenKind::Macro => {
            if is_light { Color::Rgb(140, 70, 190) } else { Color::Rgb(220, 130, 255) }
        }
    }
}

fn is_light_bg(bg: Color) -> bool {
    match bg {
        Color::Rgb(r, g, b) => {
            let lum = (0.299 * r as f32 + 0.587 * g as f32 + 0.114 * b as f32) as u32;
            lum > 140
        }
        Color::Indexed(i) => i >= 7 && i <= 15,
        Color::White | Color::Gray => true,
        _ => false,
    }
}

pub fn keywords(lang: Language) -> &'static [&'static str] {
    match lang {
        Language::Rust => &["as","async","await","break","const","continue","crate","dyn","else","enum","extern","false","fn","for","if","impl","in","let","loop","match","mod","move","mut","pub","ref","return","self","Self","static","struct","super","trait","true","type","unsafe","use","where","while","yield"],
        Language::Python => &["and","as","assert","async","await","break","class","continue","def","del","elif","else","except","False","finally","for","from","global","if","import","in","is","lambda","None","nonlocal","not","or","pass","raise","return","True","try","while","with","yield","match","case"],
        Language::JavaScript | Language::TypeScript => &["async","await","break","case","catch","class","const","continue","debugger","default","delete","do","else","export","extends","false","finally","for","function","if","import","in","instanceof","let","new","null","return","static","super","switch","this","throw","true","try","typeof","undefined","var","void","while","with","yield","interface","type","enum","implements","readonly","public","private","protected","abstract","as","declare","namespace"],
        Language::Go => &["break","case","chan","const","continue","default","defer","else","fallthrough","for","func","go","goto","if","import","interface","map","package","range","return","select","struct","switch","type","var"],
        Language::C => &["auto","break","case","char","const","continue","default","do","double","else","enum","extern","float","for","goto","if","int","long","register","return","short","signed","sizeof","static","struct","switch","typedef","union","unsigned","void","volatile","while","inline","restrict","bool","true","false","NULL"],
        Language::Cpp => &["auto","break","case","catch","char","class","const","constexpr","continue","default","delete","do","double","else","enum","explicit","extern","false","float","for","friend","goto","if","inline","int","long","mutable","namespace","new","noexcept","nullptr","operator","override","private","protected","public","register","return","short","signed","sizeof","static","struct","switch","template","this","throw","true","try","typedef","typename","union","unsigned","using","virtual","void","volatile","while"],
        Language::Shell => &["if","then","else","elif","fi","for","while","do","done","case","esac","function","return","break","continue","in","select","until","local","export","readonly","declare","source","echo","printf","cd","exit","test"],
        Language::Ruby => &["alias","and","begin","break","case","class","def","defined","do","else","elsif","end","ensure","false","for","if","in","module","next","nil","not","or","redo","rescue","retry","return","self","super","then","true","undef","unless","until","when","while","yield"],
        Language::Lua => &["and","break","do","else","elseif","end","false","for","function","if","in","local","nil","not","or","repeat","return","then","true","until","while"],
        Language::Java => &["abstract","assert","boolean","break","byte","case","catch","char","class","const","continue","default","do","double","else","enum","extends","final","finally","float","for","goto","if","implements","import","instanceof","int","interface","long","native","new","package","private","protected","public","return","short","static","strictfp","super","switch","synchronized","this","throw","throws","transient","try","void","volatile","while","true","false","null"],
        Language::Kotlin => &["as","break","class","continue","do","else","false","for","fun","if","in","interface","is","null","object","package","return","super","this","throw","true","try","typealias","val","var","when","while","data","sealed","companion","init","by"],
        Language::Php => &["abstract","and","array","as","break","callable","case","catch","class","clone","const","continue","declare","default","do","echo","else","elseif","empty","endfor","endforeach","endif","endswitch","endwhile","eval","exit","extends","final","finally","fn","for","foreach","function","global","goto","if","implements","include","instanceof","insteadof","interface","isset","list","match","namespace","new","or","print","private","protected","public","readonly","require","return","static","switch","throw","trait","try","unset","use","var","while","xor","yield","true","false","null"],
        Language::Swift => &["associatedtype","class","deinit","enum","extension","fileprivate","func","import","init","inout","internal","let","open","operator","private","protocol","public","static","struct","subscript","typealias","var","break","case","continue","default","defer","do","else","fallthrough","for","guard","if","in","repeat","return","switch","where","while","as","catch","false","is","nil","rethrows","super","self","throw","throws","true","try"],
        Language::Sql => &["SELECT","FROM","WHERE","INSERT","INTO","VALUES","UPDATE","SET","DELETE","CREATE","TABLE","ALTER","DROP","INDEX","JOIN","LEFT","RIGHT","INNER","OUTER","ON","GROUP","BY","ORDER","HAVING","LIMIT","OFFSET","AS","AND","OR","NOT","NULL","IS","IN","LIKE","BETWEEN","CASE","WHEN","THEN","ELSE","END","DISTINCT","UNION","ALL"],
        Language::Zig => &["align","allowzero","and","anyframe","anytype","asm","async","await","break","catch","comptime","const","continue","defer","else","enum","errdefer","error","export","extern","fn","for","if","inline","noalias","noinline","nosuspend","opaque","or","orelse","packed","pub","resume","return","struct","suspend","switch","test","threadlocal","try","union","unreachable","usingnamespace","var","volatile","while"],
        Language::Nim => &["addr","and","as","asm","bind","block","break","case","cast","concept","const","continue","converter","defer","discard","distinct","div","do","elif","else","end","enum","except","export","finally","for","from","func","if","import","in","include","interface","is","isnot","iterator","let","macro","method","mixin","mod","nil","not","notin","object","of","or","out","proc","ptr","raise","ref","return","shl","shr","static","template","try","tuple","type","using","var","when","while","xor","yield"],
        Language::Haskell => &["case","class","data","default","deriving","do","else","foreign","if","import","in","infix","infixl","infixr","instance","let","module","newtype","of","then","type","where"],
        Language::Elixir => &["alias","and","case","catch","cond","def","defmodule","defp","defstruct","do","else","end","fn","for","if","import","in","not","or","quote","raise","receive","require","rescue","super","throw","try","unless","unquote","use","when","with"],
        _ => &[],
    }
}

pub fn types(lang: Language) -> &'static [&'static str] {
    match lang {
        Language::Rust => &["i8","i16","i32","i64","i128","isize","u8","u16","u32","u64","u128","usize","f32","f64","bool","char","str","String","Vec","Option","Result","Box","Rc","Arc","HashMap","HashSet","BTreeMap","BTreeSet","Cow","Path","PathBuf","io","fmt","mem"],
        Language::Python => &["int","float","complex","bool","str","bytes","list","tuple","range","dict","set","frozenset","object","type","None"],
        Language::JavaScript | Language::TypeScript => &["Array","Object","String","Number","Boolean","Function","Promise","Map","Set","Date","RegExp","Error","Symbol","BigInt","null","undefined"],
        Language::Go => &["bool","byte","complex64","complex128","error","float32","float64","int","int8","int16","int32","int64","rune","string","uint","uint8","uint16","uint32","uint64","uintptr","any"],
        Language::C => &["int","char","short","long","float","double","void","signed","unsigned","size_t","ssize_t","int8_t","int16_t","int32_t","int64_t","uint8_t","uint16_t","uint32_t","uint64_t","FILE","bool"],
        Language::Cpp => &["int","char","short","long","float","double","void","signed","unsigned","bool","size_t","string","vector","map","set","unordered_map","unordered_set","shared_ptr","unique_ptr","weak_ptr","auto"],
        Language::Java => &["boolean","byte","char","double","float","int","long","short","void","String","Object","Integer","Long","Double","Float","Boolean","Character","List","Map","Set","ArrayList","HashMap","HashSet"],
        Language::Kotlin => &["Int","Long","Short","Byte","Float","Double","Boolean","Char","String","Any","Unit","Nothing","List","Map","Set","Array","Sequence"],
        Language::Ruby => &["Integer","Float","String","Symbol","Array","Hash","Range","Regexp","Proc","NilClass","TrueClass","FalseClass","Object","Class","Module"],
        Language::Swift => &["Int","Int8","Int16","Int32","Int64","UInt","UInt8","UInt16","UInt32","UInt64","Float","Double","Bool","String","Character","Array","Dictionary","Set","Optional","Any","AnyObject"],
        Language::Lua => &["string","number","boolean","table","function","thread","userdata","nil"],
        _ => &[],
    }
}

/// Common builtin functions/objects per language — used to hint typos.
pub fn builtins(lang: Language) -> &'static [&'static str] {
    match lang {
        Language::Python => &[
            "print", "input", "len", "range", "int", "str", "float", "list",
            "dict", "tuple", "set", "bool", "type", "abs", "all", "any",
            "sum", "min", "max", "sorted", "reversed", "enumerate", "zip",
            "map", "filter", "open", "help", "dir", "vars", "iter", "next",
            "format", "repr", "hash", "isinstance", "issubclass", "super",
            "getattr", "setattr", "hasattr", "delattr", "callable", "eval",
            "exec", "compile", "globals", "locals", "chr", "ord", "hex",
            "oct", "bin", "pow", "round", "divmod", "append", "extend",
            "insert", "remove", "pop", "clear", "copy", "keys", "values",
            "items", "get", "update", "split", "join", "strip", "lower",
            "upper", "replace", "startswith", "endswith", "find", "count",
        ],
        Language::Rust => &[
            "println", "print", "eprintln", "eprint", "format", "vec", "panic",
            "assert", "assert_eq", "assert_ne", "write", "writeln", "dbg",
            "todo", "unimplemented", "unreachable", "matches", "cfg", "env",
            "Some", "None", "Ok", "Err", "String", "Vec", "HashMap", "HashSet",
            "BTreeMap", "BTreeSet", "Option", "Result", "Box", "Rc", "Arc",
            "push", "pop", "insert", "remove", "len", "iter", "into_iter",
            "iter_mut", "map", "filter", "collect", "unwrap", "expect", "clone",
        ],
        Language::JavaScript | Language::TypeScript => &[
            "console", "document", "window", "alert", "prompt", "confirm",
            "parseInt", "parseFloat", "isNaN", "isFinite", "Number", "String",
            "Boolean", "Object", "Array", "JSON", "Math", "setTimeout",
            "setInterval", "clearTimeout", "clearInterval", "fetch", "Promise",
            "log", "warn", "error", "info", "debug", "random", "floor",
            "ceil", "round", "abs", "max", "min", "sqrt", "pow", "length",
            "push", "pop", "shift", "unshift", "slice", "splice", "join",
            "split", "replace", "indexOf", "includes", "filter", "map",
        ],
        Language::C => &[
            "printf", "fprintf", "sprintf", "snprintf", "scanf", "fscanf",
            "sscanf", "getchar", "putchar", "puts", "malloc", "calloc",
            "realloc", "free", "memcpy", "memset", "memmove", "strcpy",
            "strncpy", "strcmp", "strncmp", "strlen", "strcat", "strncat",
            "strstr", "fopen", "fclose", "fread", "fwrite", "fseek", "ftell",
            "exit", "abort", "atoi", "atof", "atol", "abs", "labs", "rand",
            "srand", "sizeof", "NULL",
        ],
        Language::Cpp => &[
            "printf", "cout", "cin", "cerr", "endl", "malloc", "free",
            "size", "length", "substr", "find", "push_back", "pop_back",
            "begin", "end", "insert", "erase", "clear", "empty", "resize",
            "reserve", "c_str", "data", "at", "front", "back",
        ],
        Language::Go => &[
            "Println", "Printf", "Print", "Sprintf", "Errorf", "Fprintln",
            "Fprintf", "len", "cap", "append", "make", "new", "copy",
            "delete", "panic", "recover", "close", "println", "print",
        ],
        Language::Shell => &[
            "echo", "printf", "read", "cd", "pwd", "ls", "cat", "grep",
            "awk", "sed", "sort", "uniq", "head", "tail", "wc", "find",
            "xargs", "exit", "return", "source", "export", "alias", "test",
            "eval", "exec", "set", "unset", "shift", "trap", "wait", "sleep",
        ],
        Language::Ruby => &[
            "puts", "print", "gets", "chomp", "to_s", "to_i", "to_f", "to_a",
            "to_h", "length", "size", "each", "select", "reject", "reduce",
            "inject", "push", "pop", "shift", "unshift", "include",
        ],
        Language::Lua => &[
            "print", "type", "tostring", "tonumber", "ipairs", "pairs",
            "next", "select", "rawget", "rawset", "setmetatable",
            "getmetatable", "assert", "error", "pcall", "xpcall", "require",
        ],
        Language::Php => &[
            "echo", "print", "var_dump", "print_r", "count", "strlen",
            "strtolower", "strtoupper", "substr", "str_replace", "trim",
            "explode", "implode", "array_push", "array_pop", "array_map",
            "array_filter", "array_merge", "isset", "empty", "unset", "die",
            "exit",
        ],
        Language::Java => &[
            "System", "println", "printf", "Integer", "Double", "Long",
            "Boolean", "Character", "Math", "ArrayList", "HashMap", "HashSet",
            "List", "Map", "Set", "String", "out", "abs", "max", "min",
            "sqrt", "pow", "random",
        ],
        Language::Sql => &[
            "select", "insert", "update", "delete", "create", "drop", "alter",
            "join", "where", "group", "order", "having", "limit", "count",
            "sum", "avg", "min", "max", "distinct", "union",
        ],
        _ => &[],
    }
}

pub fn highlight(line: &str, lang: Language) -> Vec<TokenKind> {
    let chars: Vec<char> = line.chars().collect();
    let mut tokens = vec![TokenKind::Normal; chars.len()];
    if matches!(lang, Language::None) { return tokens; }
    match lang {
        Language::Markdown => return highlight_markdown(&chars),
        Language::Json => return highlight_json(&chars),
        Language::Html => return highlight_html(&chars),
        Language::Css => return highlight_css(&chars),
        _ => {}
    }
    let kws = keywords(lang);
    let tys = types(lang);
    let comment_start: Option<&str> = match lang {
        Language::Rust | Language::JavaScript | Language::TypeScript
        | Language::Go | Language::C | Language::Cpp | Language::Java
        | Language::Kotlin | Language::Swift | Language::Zig | Language::Nim => Some("//"),
        Language::Python | Language::Shell | Language::Ruby | Language::Toml
        | Language::Yaml | Language::Elixir => Some("#"),
        Language::Lua | Language::Sql | Language::Haskell => Some("--"),
        _ => None,
    };
    let mut i = 0usize;
    while i < chars.len() {
        if let Some(cs) = comment_start {
            if starts_with(&chars, i, cs) {
                for j in i..chars.len() { tokens[j] = TokenKind::Comment; }
                break;
            }
        }
        if lang == Language::Python && starts_with(&chars, i, "\"\"\"") {
            let mut j = i + 3;
            while j < chars.len() && !starts_with(&chars, j, "\"\"\"") { j += 1; }
            let end = (j + 3).min(chars.len());
            for k in i..end { tokens[k] = TokenKind::String; }
            i = end; continue;
        }
        if lang == Language::Rust && chars[i] == 'r' && i + 1 < chars.len() && (chars[i + 1] == '"' || chars[i + 1] == '#') {
            let mut hashes = 0;
            let mut j = i + 1;
            while j < chars.len() && chars[j] == '#' { hashes += 1; j += 1; }
            if j < chars.len() && chars[j] == '"' {
                tokens[i] = TokenKind::String;
                for k in i + 1..=j { tokens[k] = TokenKind::String; }
                j += 1;
                let closer = format!("\"{}", "#".repeat(hashes));
                while j < chars.len() && !starts_with(&chars, j, &closer) { tokens[j] = TokenKind::String; j += 1; }
                let end = (j + closer.len()).min(chars.len());
                for k in j..end { tokens[k] = TokenKind::String; }
                i = end; continue;
            }
        }
        if chars[i] == '"' || chars[i] == '\'' || (chars[i] == '`' && matches!(lang, Language::JavaScript | Language::TypeScript | Language::Go)) {
            let q = chars[i];
            tokens[i] = TokenKind::String;
            i += 1;
            while i < chars.len() {
                if chars[i] == '\\' && i + 1 < chars.len() {
                    tokens[i] = TokenKind::String;
                    tokens[i + 1] = TokenKind::String;
                    i += 2;
                    continue;
                }
                tokens[i] = TokenKind::String;
                if chars[i] == q { i += 1; break; }
                i += 1;
            }
            continue;
        }
        if lang == Language::Shell && chars[i] == '$' {
            tokens[i] = TokenKind::Type;
            i += 1;
            if i < chars.len() && chars[i] == '{' {
                tokens[i] = TokenKind::Type;
                i += 1;
                while i < chars.len() && chars[i] != '}' { tokens[i] = TokenKind::Type; i += 1; }
                if i < chars.len() { tokens[i] = TokenKind::Type; i += 1; }
            } else {
                while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                    tokens[i] = TokenKind::Type;
                    i += 1;
                }
            }
            continue;
        }
        if lang == Language::Python && chars[i] == '@' && i + 1 < chars.len() && chars[i + 1].is_alphabetic() {
            tokens[i] = TokenKind::Macro;
            i += 1;
            while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_' || chars[i] == '.') {
                tokens[i] = TokenKind::Macro;
                i += 1;
            }
            continue;
        }
        if chars[i].is_ascii_digit() {
            let start = i;
            while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '.' || chars[i] == '_' || chars[i] == 'x') { i += 1; }
            for j in start..i { tokens[j] = TokenKind::Number; }
            continue;
        }
        if chars[i].is_alphabetic() || chars[i] == '_' {
            let start = i;
            while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') { i += 1; }
            let word: String = chars[start..i].iter().collect();
            let kind = if kws.contains(&word.as_str()) { TokenKind::Keyword }
                else if tys.contains(&word.as_str()) { TokenKind::Type }
                else if i < chars.len() && chars[i] == '(' { TokenKind::Function }
                else if i < chars.len() && chars[i] == '!' && lang == Language::Rust { TokenKind::Macro }
                else if i < chars.len() && chars[i] == '?' && lang == Language::Rust { TokenKind::Keyword }
                else { TokenKind::Normal };
            for j in start..i { tokens[j] = kind; }
            continue;
        }
        i += 1;
    }
    tokens
}

fn highlight_markdown(chars: &[char]) -> Vec<TokenKind> {
    let mut tokens = vec![TokenKind::Normal; chars.len()];
    if chars.is_empty() { return tokens; }
    let mut i = 0;
    while i < chars.len() && chars[i] == ' ' { i += 1; }
    if i < chars.len() && chars[i] == '#' {
        for j in i..chars.len() { tokens[j] = TokenKind::Keyword; }
        return tokens;
    }
    if i < chars.len() && chars[i] == '>' {
        for j in i..chars.len() { tokens[j] = TokenKind::Comment; }
        return tokens;
    }
    if i < chars.len() && (chars[i] == '-' || chars[i] == '*' || chars[i] == '+')
        && i + 1 < chars.len() && chars[i + 1] == ' '
    { tokens[i] = TokenKind::Macro; }
    i = 0;
    while i < chars.len() {
        if chars[i] == '`' {
            let start = i;
            i += 1;
            while i < chars.len() && chars[i] != '`' { i += 1; }
            if i < chars.len() { i += 1; }
            for j in start..i { tokens[j] = TokenKind::String; }
            continue;
        }
        if chars[i] == '[' {
            let start = i;
            let mut j = i;
            while j < chars.len() && chars[j] != ']' { j += 1; }
            if j + 1 < chars.len() && chars[j + 1] == '(' {
                while j < chars.len() && chars[j] != ')' { j += 1; }
                j += 1;
                for k in start..j.min(chars.len()) { tokens[k] = TokenKind::Type; }
                i = j;
                continue;
            }
        }
        if chars[i] == '*' && i + 1 < chars.len() {
            let two = chars[i + 1] == '*';
            let marker = if two { "**" } else { "*" };
            let start = i;
            i += marker.len();
            while i < chars.len() && !starts_with(chars, i, marker) { i += 1; }
            if i < chars.len() { i += marker.len(); }
            for j in start..i { tokens[j] = TokenKind::Macro; }
            continue;
        }
        i += 1;
    }
    tokens
}

fn highlight_json(chars: &[char]) -> Vec<TokenKind> {
    let mut tokens = vec![TokenKind::Normal; chars.len()];
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '"' {
            let start = i;
            i += 1;
            while i < chars.len() {
                if chars[i] == '\\' && i + 1 < chars.len() { i += 2; continue; }
                if chars[i] == '"' { i += 1; break; }
                i += 1;
            }
            let mut j = i;
            while j < chars.len() && chars[j].is_whitespace() { j += 1; }
            let kind = if j < chars.len() && chars[j] == ':' { TokenKind::Type } else { TokenKind::String };
            for k in start..i { tokens[k] = kind; }
            continue;
        }
        if chars[i] == '-' || chars[i].is_ascii_digit() {
            let start = i;
            if chars[i] == '-' { i += 1; }
            while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.' || chars[i] == 'e' || chars[i] == 'E' || chars[i] == '+' || chars[i] == '-') { i += 1; }
            for k in start..i { tokens[k] = TokenKind::Number; }
            continue;
        }
        if chars[i].is_alphabetic() {
            let start = i;
            while i < chars.len() && chars[i].is_alphabetic() { i += 1; }
            let word: String = chars[start..i].iter().collect();
            if matches!(word.as_str(), "true" | "false" | "null") {
                for k in start..i { tokens[k] = TokenKind::Keyword; }
            }
            continue;
        }
        i += 1;
    }
    tokens
}

fn highlight_html(chars: &[char]) -> Vec<TokenKind> {
    let mut tokens = vec![TokenKind::Normal; chars.len()];
    let mut i = 0;
    let mut in_tag = false;
    let mut in_string = false;
    let mut is_closing = false;
    let mut tag_start = 0;
    while i < chars.len() {
        if chars[i] == '<' {
            in_tag = true;
            is_closing = i + 1 < chars.len() && chars[i + 1] == '/';
            tag_start = i;
            tokens[i] = TokenKind::Keyword;
            i += 1;
            continue;
        }
        if chars[i] == '>' && in_tag {
            tokens[i] = TokenKind::Keyword;
            in_tag = false;
            i += 1;
            continue;
        }
        if in_tag {
            if in_string {
                tokens[i] = TokenKind::String;
                if chars[i] == '"' { in_string = false; }
                i += 1;
                continue;
            }
            if chars[i] == '"' {
                in_string = true;
                tokens[i] = TokenKind::String;
                i += 1;
                continue;
            }
            if chars[i].is_alphabetic() || chars[i] == '-' || chars[i] == '_' {
                if i == tag_start + 1 || (is_closing && i == tag_start + 2) {
                    while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '-' || chars[i] == '_') {
                        tokens[i] = TokenKind::Keyword;
                        i += 1;
                    }
                } else {
                    while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '-' || chars[i] == '_') {
                        tokens[i] = TokenKind::Type;
                        i += 1;
                    }
                }
                continue;
            }
            i += 1;
            continue;
        }
        if starts_with(chars, i, "<!--") {
            let start = i;
            while i < chars.len() && !starts_with(chars, i, "-->") { i += 1; }
            if i < chars.len() { i += 3; }
            for k in start..i { tokens[k] = TokenKind::Comment; }
            continue;
        }
        i += 1;
    }
    tokens
}

fn highlight_css(chars: &[char]) -> Vec<TokenKind> {
    let mut tokens = vec![TokenKind::Normal; chars.len()];
    let mut i = 0;
    while i < chars.len() {
        if starts_with(chars, i, "/*") {
            let start = i;
            i += 2;
            while i < chars.len() && !starts_with(chars, i, "*/") { i += 1; }
            if i < chars.len() { i += 2; }
            for k in start..i { tokens[k] = TokenKind::Comment; }
            continue;
        }
        if chars[i] == '"' || chars[i] == '\'' {
            let q = chars[i];
            let start = i;
            i += 1;
            while i < chars.len() && chars[i] != q {
                if chars[i] == '\\' && i + 1 < chars.len() { i += 2; continue; }
                i += 1;
            }
            if i < chars.len() { i += 1; }
            for k in start..i { tokens[k] = TokenKind::String; }
            continue;
        }
        if chars[i] == '#' {
            let start = i;
            i += 1;
            while i < chars.len() && chars[i].is_ascii_hexdigit() { i += 1; }
            if i - start > 1 {
                for k in start..i { tokens[k] = TokenKind::Number; }
                continue;
            }
        }
        if (chars[i] == '.' || chars[i] == '#') && i + 1 < chars.len() && chars[i + 1].is_alphabetic() {
            tokens[i] = TokenKind::Function;
            i += 1;
            while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '-' || chars[i] == '_') {
                tokens[i] = TokenKind::Function;
                i += 1;
            }
            continue;
        }
        if chars[i].is_alphabetic() || chars[i] == '-' {
            let start = i;
            while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '-' || chars[i] == '_') { i += 1; }
            let mut j = i;
            while j < chars.len() && chars[j].is_whitespace() { j += 1; }
            if j < chars.len() && chars[j] == ':' {
                for k in start..i { tokens[k] = TokenKind::Type; }
            }
            continue;
        }
        if chars[i].is_ascii_digit() {
            let start = i;
            while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '.' || chars[i] == '%') { i += 1; }
            for k in start..i { tokens[k] = TokenKind::Number; }
            continue;
        }
        i += 1;
    }
    tokens
}

fn starts_with(chars: &[char], i: usize, s: &str) -> bool {
    let needle: Vec<char> = s.chars().collect();
    if i + needle.len() > chars.len() { return false; }
    for (k, c) in needle.iter().enumerate() {
        if chars[i + k] != *c { return false; }
    }
    true
}

// ============================================================
// Diagnostics + typo detection
// ============================================================

/// Build the set of "known" words in the buffer — words that appear
/// on at least two different lines, plus all identifiers that
/// immediately follow `def`/`fn`/`function`/`class`/etc.
pub fn build_known_words(lines: &[String]) -> HashSet<String> {
    let mut freq: HashMap<String, usize> = HashMap::new();
    let mut defs: HashSet<String> = HashSet::new();

    for line in lines {
        let mut on_line: HashSet<String> = HashSet::new();
        for w in extract_raw_identifiers(line) {
            on_line.insert(w);
        }
        for w in on_line {
            *freq.entry(w).or_insert(0) += 1;
        }
        if let Some(name) = extract_def_name(line) {
            defs.insert(name);
        }
    }

    let mut known: HashSet<String> = HashSet::new();
    for (w, c) in freq {
        if c >= 2 { known.insert(w); }
    }
    for w in defs { known.insert(w); }
    known
}

/// Extract identifiers from a line, ignoring strings/comments.
fn extract_raw_identifiers(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut in_str: Option<char> = None;
    let mut escaped = false;
    let chars: Vec<char> = line.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if let Some(q) = in_str {
            if escaped { escaped = false; }
            else if c == '\\' { escaped = true; }
            else if c == q { in_str = None; }
            i += 1;
            continue;
        }
        if c == '#' || (c == '/' && i + 1 < chars.len() && chars[i + 1] == '/') {
            break;
        }
        if c == '"' || c == '\'' || c == '`' {
            if !current.is_empty() {
                if current.len() >= 2 { out.push(std::mem::take(&mut current)); }
                else { current.clear(); }
            }
            in_str = Some(c);
            i += 1;
            continue;
        }
        if !current.is_empty() {
            if c.is_alphanumeric() || c == '_' {
                current.push(c);
            } else {
                if current.len() >= 2 { out.push(std::mem::take(&mut current)); }
                else { current.clear(); }
            }
        } else if c.is_alphabetic() || c == '_' {
            current.push(c);
        }
        i += 1;
    }
    if current.len() >= 2 { out.push(current); }
    out
}

/// Get the identifier right after a definition keyword.
fn extract_def_name(line: &str) -> Option<String> {
    let t = line.trim_start();
    for prefix in &[
        "def ", "fn ", "function ", "class ", "struct ", "enum ",
        "type ", "interface ", "trait ", "impl ",
    ] {
        if let Some(rest) = t.strip_prefix(prefix) {
            let name: String = rest.chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            if !name.is_empty() { return Some(name); }
        }
    }
    None
}

/// Damerau-Levenshtein edit distance (with transposition).
fn damerau(a: &[char], b: &[char]) -> usize {
    let n = a.len();
    let m = b.len();
    if n == 0 { return m; }
    if m == 0 { return n; }
    let mut d = vec![vec![0usize; m + 1]; n + 1];
    for i in 0..=n { d[i][0] = i; }
    for j in 0..=m { d[0][j] = j; }
    for i in 1..=n {
        for j in 1..=m {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            let mut best = (d[i - 1][j] + 1)
                .min(d[i][j - 1] + 1)
                .min(d[i - 1][j - 1] + cost);
            if i > 1 && j > 1 && a[i - 1] == b[j - 2] && a[i - 2] == b[j - 1] {
                best = best.min(d[i - 2][j - 2] + 1);
            }
            d[i][j] = best;
        }
    }
    d[n][m]
}

/// Extract identifier tokens from a line, skipping strings and comments.
fn extract_identifier_tokens(line: &str, lang: Language) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut in_str: Option<char> = None;
    let mut escaped = false;
    let chars: Vec<char> = line.chars().collect();
    let mut i = 0;

    let comment_start: Option<&str> = match lang {
        Language::Rust | Language::JavaScript | Language::TypeScript
        | Language::Go | Language::C | Language::Cpp | Language::Java
        | Language::Kotlin | Language::Swift | Language::Zig | Language::Nim => Some("//"),
        Language::Python | Language::Shell | Language::Ruby
        | Language::Toml | Language::Yaml | Language::Elixir => Some("#"),
        Language::Lua | Language::Sql | Language::Haskell => Some("--"),
        _ => None,
    };

    while i < chars.len() {
        if in_str.is_none() {
            if let Some(cs) = comment_start {
                if starts_with(&chars, i, cs) { break; }
            }
        }
        let c = chars[i];
        if let Some(q) = in_str {
            if escaped { escaped = false; }
            else if c == '\\' { escaped = true; }
            else if c == q { in_str = None; }
            i += 1;
            continue;
        }
        if c == '"' || c == '\'' || c == '`' {
            if !current.is_empty() {
                out.push(std::mem::take(&mut current));
            }
            in_str = Some(c);
            i += 1;
            continue;
        }
        if !current.is_empty() {
            if c.is_alphanumeric() || c == '_' {
                current.push(c);
            } else {
                out.push(std::mem::take(&mut current));
            }
        } else if c.is_alphabetic() || c == '_' {
            current.push(c);
        }
        i += 1;
    }
    if !current.is_empty() { out.push(current); }
    out
}

/// Diagnose a line for issues:
///  - unterminated strings
///  - likely typos of well-known language identifiers
pub fn diagnose_line(
    line: &str,
    lang: Language,
    known: &HashSet<String>,
) -> Vec<String> {
    let mut issues = Vec::new();
    if matches!(lang, Language::None) { return issues; }

    // ── 1) Unterminated string check (unchanged) ─────────────
    {
        let chars: Vec<char> = line.chars().collect();
        let mut i = 0usize;
        let mut in_string: Option<char> = None;
        let mut escaped = false;
        let comment = match lang {
            Language::Rust | Language::JavaScript | Language::TypeScript
            | Language::Go | Language::C | Language::Cpp | Language::Java
            | Language::Kotlin | Language::Swift | Language::Zig | Language::Nim => Some("//"),
            Language::Python | Language::Shell | Language::Ruby
            | Language::Toml | Language::Yaml | Language::Elixir => Some("#"),
            Language::Lua | Language::Sql | Language::Haskell => Some("--"),
            _ => None,
        };
        while i < chars.len() {
            if in_string.is_none() {
                if let Some(cs) = comment {
                    if starts_with(&chars, i, cs) { break; }
                }
            }
            let c = chars[i];
            if let Some(q) = in_string {
                if escaped { escaped = false; }
                else if c == '\\' { escaped = true; }
                else if c == q { in_string = None; }
                i += 1;
                continue;
            }
            match c {
                '"' => in_string = Some('"'),
                '\'' => {
                    if matches!(lang, Language::Rust) {
                        let mut j = i + 1;
                        while j < chars.len()
                            && (chars[j].is_alphanumeric() || chars[j] == '_')
                        { j += 1; }
                        let has_closing = j < chars.len() && chars[j] == '\'';
                        let is_lifetime = !has_closing && j > i + 1;
                        if !is_lifetime { in_string = Some('\''); }
                    } else {
                        in_string = Some('\'');
                    }
                }
                _ => {}
            }
            i += 1;
        }
        if in_string.is_some() {
            issues.push("unterminated string".to_string());
        }
    }

    // ── 2) Typo detection against language's known identifiers ──
    let kws = keywords(lang);
    let tys = types(lang);
    let bis = builtins(lang);
    if kws.is_empty() && tys.is_empty() && bis.is_empty() {
        return issues;
    }

    let tokens = extract_identifier_tokens(line, lang);
    for word in tokens {
        if known.contains(&word) { continue; }
        if kws.contains(&word.as_str()) { continue; }
        if tys.contains(&word.as_str()) { continue; }
        if bis.contains(&word.as_str()) { continue; }
        if word.chars().all(|c| c.is_uppercase() || c == '_') { continue; }
        if word.starts_with('_') { continue; }
        let wchars: Vec<char> = word.chars().collect();
        let len = wchars.len();
        if len < 4 { continue; }

        let max_dist: usize = if len <= 5 { 1 } else { 2 };

        let mut best: Option<(&str, usize)> = None;
        for cand in kws.iter()
            .copied()
            .chain(tys.iter().copied())
            .chain(bis.iter().copied())
        {
            let cchars: Vec<char> = cand.chars().collect();
            let dl = (cchars.len() as isize - len as isize).unsigned_abs();
            if dl > max_dist { continue; }
            let d = damerau(&wchars, &cchars);
            if d > 0 && d <= max_dist {
                match best {
                    Some((_, bd)) if bd <= d => {}
                    _ => best = Some((cand, d)),
                }
            }
        }

        if let Some((sug, _)) = best {
            issues.push(format!("unknown '{}', did you mean '{}'?", word, sug));
        }
    }

    issues
}