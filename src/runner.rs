//! Choosing the shell command that `Space + r` (or `:run`) executes for a file.
//!
//! * Languages live in one table (`RULES`): extension(s) -> candidate tools.
//!   The first tool that is installed wins (`python3`, else `python`, ...).
//! * Projects are respected: a Rust file inside a Cargo project uses
//!   `cargo run`, a Go file inside a module uses `go run .`.
//! * Extension-less scripts run through their `#!` line.
//! * Users can add or override languages in `config.ini` with
//!   `run.<ext> = <command>` (placeholders: `{f}` `{dir}` `{stem}` `{out}`).
//!
//! Every placeholder is shell-quoted, so paths with spaces or quotes are safe.
//! `plan()` is a pure function over an `Env`, which keeps it testable.

use std::fs;
use std::path::{Path, PathBuf};

/// Everything `plan()` needs to know about the machine.
pub struct Env<'a> {
    /// Is this program on PATH?
    pub have: &'a dyn Fn(&str) -> bool,
    /// Does this path exist?
    pub exists: &'a dyn Fn(&Path) -> bool,
    /// Private scratch directory for build output (compiled languages).
    pub tmp: PathBuf,
    /// First line of the file when it starts with `#!`.
    pub shebang: Option<String>,
    /// User rules from `run.<ext> = ...` (extension without dot, template).
    pub custom: &'a [(String, String)],
}

/// The decision: what to type into the terminal.
#[derive(Debug, PartialEq, Eq)]
pub struct Planned {
    pub cmd: String,
    /// The command writes into `Env::tmp`, so that directory must exist.
    pub needs_scratch: bool,
    /// Worth telling the user (e.g. the tool was not found on PATH).
    pub note: Option<String>,
}

struct Tool {
    prog: &'static str,
    tpl: &'static str,
}

struct Rule {
    exts: &'static [&'static str],
    /// Whole file names (lowercase) such as `makefile`.
    names: &'static [&'static str],
    tools: &'static [Tool],
}

const fn t(prog: &'static str, tpl: &'static str) -> Tool { Tool { prog, tpl } }

const RULES: &[Rule] = &[
    // ---- scripting ----
    Rule { exts: &["py", "pyw"], names: &[], tools: &[
        t("python3", "python3 {f}"), t("python", "python {f}"), t("py", "py {f}")] },
    Rule { exts: &["js", "mjs", "cjs"], names: &[], tools: &[
        t("node", "node {f}"), t("bun", "bun run {f}"), t("deno", "deno run -A {f}")] },
    Rule { exts: &["ts", "mts", "cts", "tsx"], names: &[], tools: &[
        t("bun", "bun run {f}"), t("deno", "deno run -A {f}"), t("tsx", "tsx {f}"),
        t("ts-node", "ts-node {f}"), t("npx", "npx --yes tsx {f}")] },
    Rule { exts: &["sh", "bash"], names: &[], tools: &[
        t("bash", "bash {f}"), t("sh", "sh {f}")] },
    Rule { exts: &["zsh"], names: &[], tools: &[t("zsh", "zsh {f}")] },
    Rule { exts: &["fish"], names: &[], tools: &[t("fish", "fish {f}")] },
    Rule { exts: &["ps1"], names: &[], tools: &[
        t("pwsh", "pwsh -File {f}"), t("powershell", "powershell -File {f}")] },
    Rule { exts: &["rb"], names: &[], tools: &[t("ruby", "ruby {f}")] },
    Rule { exts: &["php"], names: &[], tools: &[t("php", "php {f}")] },
    Rule { exts: &["lua"], names: &[], tools: &[t("lua", "lua {f}"), t("luajit", "luajit {f}")] },
    Rule { exts: &["pl", "pm"], names: &[], tools: &[t("perl", "perl {f}")] },
    Rule { exts: &["r"], names: &[], tools: &[t("Rscript", "Rscript {f}")] },
    Rule { exts: &["jl"], names: &[], tools: &[t("julia", "julia {f}")] },
    Rule { exts: &["dart"], names: &[], tools: &[t("dart", "dart run {f}")] },
    Rule { exts: &["ex", "exs"], names: &[], tools: &[t("elixir", "elixir {f}")] },
    Rule { exts: &["hs"], names: &[], tools: &[
        t("runghc", "runghc {f}"), t("stack", "stack runghc {f}")] },
    Rule { exts: &["rkt"], names: &[], tools: &[t("racket", "racket {f}")] },
    Rule { exts: &["lisp", "cl"], names: &[], tools: &[t("sbcl", "sbcl --script {f}")] },
    Rule { exts: &["clj", "cljs"], names: &[], tools: &[
        t("clojure", "clojure -M {f}"), t("clj", "clj -M {f}")] },
    Rule { exts: &["groovy"], names: &[], tools: &[t("groovy", "groovy {f}")] },
    Rule { exts: &["kts"], names: &[], tools: &[t("kotlin", "kotlin {f}")] },
    Rule { exts: &["swift"], names: &[], tools: &[t("swift", "swift {f}")] },
    Rule { exts: &["scala", "sc"], names: &[], tools: &[
        t("scala-cli", "scala-cli run {f}"), t("scala", "scala {f}")] },
    Rule { exts: &["awk"], names: &[], tools: &[t("awk", "awk -f {f}")] },
    Rule { exts: &["tcl"], names: &[], tools: &[t("tclsh", "tclsh {f}")] },
    Rule { exts: &["coffee"], names: &[], tools: &[t("coffee", "coffee {f}")] },
    Rule { exts: &["ml"], names: &[], tools: &[t("ocaml", "ocaml {f}")] },
    Rule { exts: &["fsx"], names: &[], tools: &[t("dotnet", "dotnet fsi {f}")] },
    Rule { exts: &["mojo"], names: &[], tools: &[t("mojo", "mojo {f}")] },
    // ---- newer compiled languages with a "run" command ----
    Rule { exts: &["nim"], names: &[], tools: &[t("nim", "nim c -r {f}")] },
    Rule { exts: &["zig"], names: &[], tools: &[t("zig", "zig run {f}")] },
    Rule { exts: &["cr"], names: &[], tools: &[t("crystal", "crystal run {f}")] },
    Rule { exts: &["v"], names: &[], tools: &[t("v", "v run {f}")] },
    Rule { exts: &["odin"], names: &[], tools: &[t("odin", "odin run {f} -file")] },
    Rule { exts: &["d"], names: &[], tools: &[t("rdmd", "rdmd {f}"), t("dmd", "dmd -run {f}")] },
    // ---- compiled: build into the scratch dir, then run ----
    Rule { exts: &["c"], names: &[], tools: &[
        t("gcc", "gcc {f} -o {out} -lm && {out}"),
        t("cc", "cc {f} -o {out} -lm && {out}"),
        t("clang", "clang {f} -o {out} -lm && {out}")] },
    Rule { exts: &["cpp", "cc", "cxx", "c++"], names: &[], tools: &[
        t("g++", "g++ {f} -o {out} && {out}"),
        t("clang++", "clang++ {f} -o {out} && {out}"),
        t("c++", "c++ {f} -o {out} && {out}")] },
    Rule { exts: &["f90", "f95", "f03", "f08", "f", "for"], names: &[], tools: &[
        t("gfortran", "gfortran {f} -o {out} && {out}")] },
    Rule { exts: &["java"], names: &[], tools: &[
        t("javac", "cd {dir} && javac {f} && java {stem}")] },
    Rule { exts: &["kt"], names: &[], tools: &[
        t("kotlinc", "kotlinc {f} -include-runtime -d {out}.jar && java -jar {out}.jar")] },
    // ---- documents & build files ----
    Rule { exts: &["html", "htm"], names: &[], tools: &[
        t("xdg-open", "xdg-open {f}"), t("open", "open {f}"), t("wslview", "wslview {f}")] },
    Rule { exts: &[], names: &["makefile", "gnumakefile"], tools: &[t("make", "cd {dir} && make")] },
    Rule { exts: &[], names: &["justfile"], tools: &[t("just", "cd {dir} && just")] },
];

// ============================================================
// Helpers
// ============================================================

/// Quote for a POSIX-style shell (also fine in fish): `it's` -> `'it'\''s'`.
pub fn quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

fn expand(tpl: &str, path: &Path, env: &Env, used_out: &mut bool) -> String {
    let stem = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let dir = path.parent().map(|d| d.to_path_buf()).unwrap_or_default();
    if tpl.contains("{out}") { *used_out = true; }
    tpl.replace("{f}", &quote(&path.to_string_lossy()))
        .replace("{dir}", &quote(&dir.to_string_lossy()))
        .replace("{stem}", &quote(&stem))
        .replace("{out}", &quote(&env.tmp.join(&stem).to_string_lossy()))
}

/// Nearest ancestor directory (excluding the file itself) containing `name`.
fn find_up(path: &Path, name: &str, env: &Env) -> Option<PathBuf> {
    path.ancestors().skip(1).find(|d| (env.exists)(&d.join(name))).map(|d| d.to_path_buf())
}

fn done(cmd: String, needs_scratch: bool, note: Option<String>) -> Result<Planned, String> {
    Ok(Planned { cmd, needs_scratch, note })
}

/// Pick the first installed tool; if none is installed, still use the first one
/// (the terminal will say "command not found") and add a note.
fn from_tools(tools: &[Tool], path: &Path, env: &Env, what: &str) -> Result<Planned, String> {
    let mut out = false;
    let pick = tools.iter().find(|t| (env.have)(t.prog));
    let (tool, note) = match pick {
        Some(t) => (t, None),
        None => {
            let names: Vec<&str> = tools.iter().map(|t| t.prog).collect();
            (&tools[0], Some(format!("{} not found in PATH (needed to run {})", names.join(" / "), what)))
        }
    };
    let cmd = expand(tool.tpl, path, env, &mut out);
    done(cmd, out, note)
}

fn rust(path: &Path, env: &Env) -> Result<Planned, String> {
    if let Some(root) = find_up(path, "Cargo.toml", env) {
        let rel: Vec<String> = path.strip_prefix(&root).ok()
            .map(|r| r.components().map(|c| c.as_os_str().to_string_lossy().to_string()).collect())
            .unwrap_or_default();
        let stem = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        let extra = match rel.iter().map(|s| s.as_str()).collect::<Vec<_>>().as_slice() {
            ["src", "bin", _] => format!(" --bin {}", quote(&stem)),
            ["src", "bin", dir, "main.rs"] => format!(" --bin {}", quote(dir)),
            ["examples", _] => format!(" --example {}", quote(&stem)),
            _ => String::new(),
        };
        let note = if (env.have)("cargo") { None } else { Some("cargo not found in PATH".to_string()) };
        return done(format!("cd {} && cargo run{}", quote(&root.to_string_lossy()), extra), false, note);
    }
    from_tools(&[t("rustc", "rustc {f} -o {out} && {out}")], path, env, ".rs files")
}

fn go(path: &Path, env: &Env) -> Result<Planned, String> {
    let note = if (env.have)("go") { None } else { Some("go not found in PATH".to_string()) };
    if find_up(path, "go.mod", env).is_some() {
        let dir = path.parent().map(|d| d.to_string_lossy().to_string()).unwrap_or_default();
        return done(format!("cd {} && go run .", quote(&dir)), false, note);
    }
    done(format!("go run {}", quote(&path.to_string_lossy())), false, note)
}

/// `#!/usr/bin/env python3 -u` -> ["python3", "-u"]; `#!/bin/bash` -> ["/bin/bash"].
fn shebang_tokens(line: &str) -> Vec<String> {
    let mut parts: Vec<String> = line.trim_start_matches("#!").split_whitespace().map(String::from).collect();
    if parts.first().map_or(false, |p| p.ends_with("/env")) {
        parts.remove(0);
        if parts.first().map_or(false, |p| p == "-S") { parts.remove(0); }
    }
    parts
}

// ============================================================
// The decision
// ============================================================

pub fn plan(path: &Path, env: &Env) -> Result<Planned, String> {
    let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    let name = path.file_name().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();

    // 1. the user's own rules win
    if !ext.is_empty() {
        if let Some((_, tpl)) = env.custom.iter().find(|(e, _)| *e == ext) {
            let mut out = false;
            let cmd = expand(tpl, path, env, &mut out);
            return done(cmd, out, None);
        }
    }

    // 2. languages that depend on the surrounding project
    match ext.as_str() {
        "rs" => return rust(path, env),
        "go" => return go(path, env),
        _ => {}
    }

    // 3. the table
    for rule in RULES {
        let hit = (!ext.is_empty() && rule.exts.contains(&ext.as_str())) || rule.names.contains(&name.as_str());
        if hit {
            let what = if ext.is_empty() { name.clone() } else { format!(".{} files", ext) };
            return from_tools(rule.tools, path, env, &what);
        }
    }

    // 4. scripts with a #! line
    if let Some(line) = &env.shebang {
        let toks = shebang_tokens(line);
        if let Some(prog) = toks.first() {
            let base = prog.rsplit('/').next().unwrap_or(prog);
            let note = if (env.have)(base) || (env.have)(prog) { None }
                       else { Some(format!("{} not found in PATH", base)) };
            let quoted: Vec<String> = toks.iter().map(|s| quote(s)).collect();
            return done(format!("{} {}", quoted.join(" "), quote(&path.to_string_lossy())), false, note);
        }
    }

    let what = if ext.is_empty() { name } else { format!(".{}", ext) };
    Err(format!("don't know how to run {} (add `run.{} = <command> {{f}}` to config.ini)",
        what, if ext.is_empty() { "ext" } else { ext.as_str() }))
}

// ============================================================
// Real-machine helpers
// ============================================================

/// Is `prog` an executable file on PATH (or a path to one)?
pub fn on_path(prog: &str) -> bool {
    fn is_exec(p: &Path) -> bool {
        match fs::metadata(p) {
            Ok(m) if m.is_file() => {
                #[cfg(unix)]
                { use std::os::unix::fs::PermissionsExt; m.permissions().mode() & 0o111 != 0 }
                #[cfg(not(unix))]
                { true }
            }
            _ => false,
        }
    }
    if prog.contains('/') { return is_exec(Path::new(prog)); }
    std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).any(|d| is_exec(&d.join(prog))))
        .unwrap_or(false)
}

/// Where this Anvil process keeps build output (not created until needed).
pub fn scratch_path() -> PathBuf {
    std::env::temp_dir().join(format!("anvil-run-{}", std::process::id()))
}

/// Create the scratch directory, private to this user. Refuses to reuse a
/// pre-existing path that is a symlink or a foreign file.
pub fn make_scratch() -> Result<(), String> {
    let dir = scratch_path();
    #[cfg(unix)]
    let created = {
        use std::os::unix::fs::DirBuilderExt;
        fs::DirBuilder::new().mode(0o700).create(&dir)
    };
    #[cfg(not(unix))]
    let created = fs::create_dir(&dir);
    match created {
        Ok(_) => Ok(()),
        Err(_) => match fs::symlink_metadata(&dir) {
            Ok(m) if m.is_dir() && !m.file_type().is_symlink() => Ok(()),
            _ => Err(format!("can't use scratch dir {}", dir.display())),
        },
    }
}

/// Remove build output when the editor exits.
pub fn cleanup() {
    let _ = fs::remove_dir_all(scratch_path());
}

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    struct Fake { progs: HashSet<&'static str>, files: HashSet<&'static str> }

    fn run(f: &Fake, path: &str, shebang: Option<&str>, custom: &[(String, String)]) -> Result<Planned, String> {
        let have = |p: &str| f.progs.contains(p);
        let exists = |p: &Path| f.files.contains(p.to_string_lossy().as_ref());
        let env = Env {
            have: &have, exists: &exists, tmp: PathBuf::from("/tmp/anvil-run-1"),
            shebang: shebang.map(String::from), custom,
        };
        plan(Path::new(path), &env)
    }

    fn fake(progs: &[&'static str], files: &[&'static str]) -> Fake {
        Fake { progs: progs.iter().copied().collect(), files: files.iter().copied().collect() }
    }

    fn cmd(f: &Fake, path: &str) -> String { run(f, path, None, &[]).unwrap().cmd }

    #[test]
    fn picks_the_first_installed_tool() {
        assert_eq!(cmd(&fake(&["python3", "python"], &[]), "/p/a.py"), "python3 '/p/a.py'");
        assert_eq!(cmd(&fake(&["python"], &[]), "/p/a.py"), "python '/p/a.py'");
        assert_eq!(cmd(&fake(&["deno", "node"], &[]), "/p/a.js"), "node '/p/a.js'");
        assert_eq!(cmd(&fake(&["deno"], &[]), "/p/a.js"), "deno run -A '/p/a.js'");
        assert_eq!(cmd(&fake(&["tsx"], &[]), "/p/a.ts"), "tsx '/p/a.ts'");
        assert_eq!(cmd(&fake(&["bun", "tsx"], &[]), "/p/a.ts"), "bun run '/p/a.ts'");
    }

    #[test]
    fn missing_tool_still_runs_the_first_choice_with_a_note() {
        let p = run(&fake(&[], &[]), "/p/a.py", None, &[]).unwrap();
        assert_eq!(p.cmd, "python3 '/p/a.py'");
        assert!(p.note.unwrap().contains("python3"));
        assert!(run(&fake(&["python3"], &[]), "/p/a.py", None, &[]).unwrap().note.is_none());
    }

    #[test]
    fn quoting_survives_awkward_names() {
        let f = fake(&["python3"], &[]);
        assert_eq!(cmd(&f, "/p/it's here.py"), r"python3 '/p/it'\''s here.py'");
        assert_eq!(cmd(&f, "/p/$(rm -rf x).py"), "python3 '/p/$(rm -rf x).py'");
        assert_eq!(quote("a'b"), r"'a'\''b'");
    }

    #[test]
    fn extensions_are_case_insensitive() {
        assert_eq!(cmd(&fake(&["python3"], &[]), "/p/A.PY"), "python3 '/p/A.PY'");
    }

    #[test]
    fn compiled_languages_build_into_the_scratch_dir() {
        let p = run(&fake(&["gcc"], &[]), "/p/a.c", None, &[]).unwrap();
        assert_eq!(p.cmd, "gcc '/p/a.c' -o '/tmp/anvil-run-1/a' -lm && '/tmp/anvil-run-1/a'");
        assert!(p.needs_scratch);
        let p = run(&fake(&["clang++"], &[]), "/p/a.cpp", None, &[]).unwrap();
        assert!(p.cmd.starts_with("clang++ '/p/a.cpp' -o "));
        let p = run(&fake(&["python3"], &[]), "/p/a.py", None, &[]).unwrap();
        assert!(!p.needs_scratch);
    }

    #[test]
    fn rust_uses_cargo_inside_a_project() {
        let f = fake(&["cargo", "rustc"], &["/w/proj/Cargo.toml"]);
        assert_eq!(cmd(&f, "/w/proj/src/main.rs"), "cd '/w/proj' && cargo run");
        assert_eq!(cmd(&f, "/w/proj/src/lib.rs"), "cd '/w/proj' && cargo run");
        assert_eq!(cmd(&f, "/w/proj/src/bin/tool.rs"), "cd '/w/proj' && cargo run --bin 'tool'");
        assert_eq!(cmd(&f, "/w/proj/src/bin/tool/main.rs"), "cd '/w/proj' && cargo run --bin 'tool'");
        assert_eq!(cmd(&f, "/w/proj/examples/demo.rs"), "cd '/w/proj' && cargo run --example 'demo'");
    }

    #[test]
    fn rust_alone_uses_rustc() {
        let p = run(&fake(&["rustc"], &[]), "/p/a.rs", None, &[]).unwrap();
        assert_eq!(p.cmd, "rustc '/p/a.rs' -o '/tmp/anvil-run-1/a' && '/tmp/anvil-run-1/a'");
        assert!(p.needs_scratch);
    }

    #[test]
    fn go_uses_the_module_when_there_is_one() {
        assert_eq!(cmd(&fake(&["go"], &["/w/m/go.mod"]), "/w/m/cmd/main.go"), "cd '/w/m/cmd' && go run .");
        assert_eq!(cmd(&fake(&["go"], &[]), "/w/a.go"), "go run '/w/a.go'");
    }

    #[test]
    fn shebang_runs_extensionless_scripts() {
        let f = fake(&["python3"], &[]);
        let p = run(&f, "/p/tool", Some("#!/usr/bin/env python3 -u"), &[]).unwrap();
        assert_eq!(p.cmd, "'python3' '-u' '/p/tool'");
        assert!(p.note.is_none());
        let p = run(&f, "/p/tool", Some("#!/bin/bash"), &[]).unwrap();
        assert_eq!(p.cmd, "'/bin/bash' '/p/tool'");
        let p = run(&f, "/p/tool", Some("#!/usr/bin/env -S node --harmony"), &[]).unwrap();
        assert_eq!(p.cmd, "'node' '--harmony' '/p/tool'");
        assert!(p.note.is_some()); // node isn't installed in this fake
    }

    #[test]
    fn user_rules_override_everything() {
        let custom = vec![("py".to_string(), "uv run {f}".to_string()),
                          ("foo".to_string(), "cd {dir} && foo build {stem} -o {out}".to_string())];
        let f = fake(&["python3"], &[]);
        assert_eq!(run(&f, "/p/a.py", None, &custom).unwrap().cmd, "uv run '/p/a.py'");
        let p = run(&f, "/p/x.foo", None, &custom).unwrap();
        assert_eq!(p.cmd, "cd '/p' && foo build 'x' -o '/tmp/anvil-run-1/x'");
        assert!(p.needs_scratch);
    }

    #[test]
    fn build_files_and_html() {
        assert_eq!(cmd(&fake(&["make"], &[]), "/p/Makefile"), "cd '/p' && make");
        assert_eq!(cmd(&fake(&["just"], &[]), "/p/justfile"), "cd '/p' && just");
        assert_eq!(cmd(&fake(&["xdg-open"], &[]), "/p/i.html"), "xdg-open '/p/i.html'");
        assert_eq!(cmd(&fake(&["open"], &[]), "/p/i.html"), "open '/p/i.html'");
    }

    #[test]
    fn java_runs_from_its_folder() {
        assert_eq!(cmd(&fake(&["javac"], &[]), "/p/Main.java"), "cd '/p' && javac '/p/Main.java' && java 'Main'");
    }

    #[test]
    fn unknown_files_get_a_helpful_error() {
        let e = run(&fake(&[], &[]), "/p/a.xyz", None, &[]).unwrap_err();
        assert!(e.contains(".xyz") && e.contains("run.xyz"));
        assert!(run(&fake(&[], &[]), "/p/notes", None, &[]).is_err());
    }

    #[test]
    fn every_rule_is_well_formed() {
        for r in RULES {
            assert!(!r.exts.is_empty() || !r.names.is_empty());
            assert!(!r.tools.is_empty());
            for tool in r.tools {
                assert!(tool.tpl.split_whitespace().any(|w| w == tool.prog), "template {:?} must call {}", tool.tpl, tool.prog);
                assert!(tool.tpl.contains("{f}") || tool.tpl.contains("{dir}"), "{:?} never mentions the file", tool.tpl);
            }
        }
        // no extension is claimed by two rules
        let mut seen = HashSet::new();
        for r in RULES { for e in r.exts { assert!(seen.insert(*e), "duplicate extension {}", e); } }
    }
}
