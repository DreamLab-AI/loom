//! Shared scanning helpers for the ontomark migration tests.
//!
//! Nothing here imports ontomark. Manifests and `Cargo.lock` are parsed as TOML
//! (never grepped); Rust sources are lexed with `proc-macro2`, which drops every
//! comment and doc comment and keeps string literals as their own tokens, so
//! a comment can never count as adoption and whitespace cannot hide a
//! declaration.
#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use proc_macro2::{Delimiter, TokenStream, TokenTree};

/// The workspace root (two levels above a crate manifest directory).
pub fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Read a workspace-relative file; a missing file reads as empty.
pub fn read(rel: &str) -> String {
    fs::read_to_string(workspace().join(rel)).unwrap_or_default()
}

// ---------------------------------------------------------------- Rust lexing

/// One lexical token of Rust code. Comments are never produced.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Tok {
    Ident(String),
    Punct(char),
    /// The decoded value of a string or byte-string literal.
    Str(String),
    /// Any other literal (number, char), as written.
    Lit(String),
    Open(char),
    Close(char),
}

fn flatten(ts: TokenStream, out: &mut Vec<Tok>) {
    let trees: Vec<TokenTree> = ts.into_iter().collect();
    let mut i = 0;
    while i < trees.len() {
        // Drop `#[doc = ...]` / `#![doc = ...]`: doc comments are comments.
        if let TokenTree::Punct(p) = &trees[i] {
            if p.as_char() == '#' {
                let mut j = i + 1;
                if matches!(trees.get(j), Some(TokenTree::Punct(b)) if b.as_char() == '!') {
                    j += 1;
                }
                if let Some(TokenTree::Group(g)) = trees.get(j) {
                    if g.delimiter() == Delimiter::Bracket {
                        let inner: Vec<TokenTree> = g.stream().into_iter().collect();
                        if matches!(inner.first(), Some(TokenTree::Ident(d)) if d == "doc")
                            && matches!(inner.get(1), Some(TokenTree::Punct(e)) if e.as_char() == '=')
                        {
                            i = j + 1;
                            continue;
                        }
                    }
                }
            }
        }
        match &trees[i] {
            TokenTree::Ident(id) => out.push(Tok::Ident(id.to_string())),
            TokenTree::Punct(p) => out.push(Tok::Punct(p.as_char())),
            TokenTree::Literal(l) => {
                let text = l.to_string();
                match syn::parse_str::<syn::Lit>(&text) {
                    Ok(syn::Lit::Str(s)) => out.push(Tok::Str(s.value())),
                    Ok(syn::Lit::ByteStr(b)) => {
                        out.push(Tok::Str(String::from_utf8_lossy(&b.value()).into_owned()));
                    }
                    _ => out.push(Tok::Lit(text)),
                }
            }
            TokenTree::Group(g) => {
                let (o, c) = match g.delimiter() {
                    Delimiter::Parenthesis => ('(', ')'),
                    Delimiter::Brace => ('{', '}'),
                    Delimiter::Bracket => ('[', ']'),
                    Delimiter::None => {
                        flatten(g.stream(), out);
                        i += 1;
                        continue;
                    }
                };
                out.push(Tok::Open(o));
                flatten(g.stream(), out);
                out.push(Tok::Close(c));
            }
        }
        i += 1;
    }
}

/// Lex Rust source into tokens with comments removed.
pub fn lex(src: &str) -> Vec<Tok> {
    let ts = TokenStream::from_str(src).unwrap_or_else(|e| panic!("cannot lex Rust source: {e}"));
    let mut out = Vec::new();
    flatten(ts, &mut out);
    out
}

fn skip_balanced(toks: &[Tok], mut i: usize) -> usize {
    let mut depth = 0usize;
    while i < toks.len() {
        match toks[i] {
            Tok::Open(_) => depth += 1,
            Tok::Close(_) => {
                depth -= 1;
                if depth == 0 {
                    return i + 1;
                }
            }
            _ => {}
        }
        i += 1;
    }
    i
}

/// Remove every item annotated `#[cfg(test)]` (inline test modules and helpers).
pub fn strip_cfg_test(toks: &[Tok]) -> Vec<Tok> {
    use Tok::{Close, Ident, Open, Punct};
    let pat = [
        Punct('#'),
        Open('['),
        Ident("cfg".into()),
        Open('('),
        Ident("test".into()),
        Close(')'),
        Close(']'),
    ];
    let mut out = Vec::new();
    let mut i = 0;
    while i < toks.len() {
        if toks[i..].starts_with(&pat) {
            i += pat.len();
            // further attributes on the same item
            while matches!(toks.get(i), Some(Punct('#')))
                && matches!(toks.get(i + 1), Some(Open('[')))
            {
                i = skip_balanced(toks, i + 1);
            }
            // the item itself: up to a top-level `;`, or through its `{ .. }` body
            while i < toks.len() {
                match toks[i] {
                    Punct(';') => {
                        i += 1;
                        break;
                    }
                    Open('{') => {
                        i = skip_balanced(toks, i);
                        break;
                    }
                    Open(_) => i = skip_balanced(toks, i),
                    _ => i += 1,
                }
            }
            continue;
        }
        out.push(toks[i].clone());
        i += 1;
    }
    out
}

/// A source file under some crate's `src/`, lexed.
pub struct Source {
    pub rel: String,
    pub toks: Vec<Tok>,
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = fs::read_dir(dir) else { return };
    let mut entries: Vec<_> = rd.flatten().map(|e| e.path()).collect();
    entries.sort();
    for p in entries {
        if p.is_dir() {
            walk(&p, out);
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p);
        }
    }
}

/// Every `crates/*/src/**/*.rs`, lexed. Test directories (`crates/*/tests`)
/// are never scanned, so a migration test cannot scan itself.
pub fn all_sources() -> Vec<Source> {
    let mut files = Vec::new();
    let mut crates: Vec<_> = fs::read_dir(workspace().join("crates"))
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .collect();
    crates.sort();
    for c in crates {
        walk(&c.join("src"), &mut files);
    }
    files
        .into_iter()
        .map(|f| Source {
            rel: f.strip_prefix(workspace()).unwrap().display().to_string(),
            toks: lex(&fs::read_to_string(&f).unwrap_or_default()),
        })
        .collect()
}

/// Sources of one crate (`crates/<name>/src`).
pub fn crate_sources(name: &str) -> Vec<Source> {
    let prefix = format!("crates/{name}/src/");
    all_sources()
        .into_iter()
        .filter(|s| s.rel.starts_with(&prefix))
        .collect()
}

/// Lex one workspace-relative file (empty if missing).
pub fn source(rel: &str) -> Source {
    Source {
        rel: rel.to_owned(),
        toks: lex(&read(rel)),
    }
}

/// Whether `toks` contains the path `a::b` (adjacent identifiers).
pub fn has_path(toks: &[Tok], a: &str, b: &str) -> bool {
    toks.windows(4).any(|w| {
        matches!(&w[0], Tok::Ident(x) if x == a)
            && w[1] == Tok::Punct(':')
            && w[2] == Tok::Punct(':')
            && matches!(&w[3], Tok::Ident(x) if x == b)
    })
}

/// Whether `toks` contains a code path rooted at crate `root` (`root::...`).
pub fn uses_crate(toks: &[Tok], root: &str) -> bool {
    toks.windows(3).any(|w| {
        matches!(&w[0], Tok::Ident(x) if x == root)
            && w[1] == Tok::Punct(':')
            && w[2] == Tok::Punct(':')
    })
}

/// Whether `toks` mentions `root::...name` inside a single path or `use` group
/// (`name` within 16 tokens after `root ::`, stopping at `;`).
pub fn uses_item_of(toks: &[Tok], root: &str, name: &str) -> bool {
    for (i, w) in toks.windows(3).enumerate() {
        if matches!(&w[0], Tok::Ident(x) if x == root)
            && w[1] == Tok::Punct(':')
            && w[2] == Tok::Punct(':')
        {
            for t in toks[i + 3..].iter().take(16) {
                if *t == Tok::Punct(';') {
                    break;
                }
                if matches!(t, Tok::Ident(x) if x == name) {
                    return true;
                }
            }
        }
    }
    false
}

/// A function found in a token stream.
pub struct Func {
    pub name: String,
    pub body: Vec<Tok>,
}

/// Every `fn name ... { body }` (including methods and nested functions).
pub fn functions(toks: &[Tok]) -> Vec<Func> {
    let mut out = Vec::new();
    let mut i = 0;
    while i + 1 < toks.len() {
        if matches!(&toks[i], Tok::Ident(k) if k == "fn") {
            if let Tok::Ident(name) = &toks[i + 1] {
                let mut j = i + 2;
                while j < toks.len() {
                    match toks[j] {
                        Tok::Punct(';') => break,
                        Tok::Open('{') => {
                            let end = skip_balanced(toks, j);
                            out.push(Func {
                                name: name.clone(),
                                body: toks[j + 1..end - 1].to_vec(),
                            });
                            break;
                        }
                        Tok::Open(_) => j = skip_balanced(toks, j),
                        _ => j += 1,
                    }
                }
            }
        }
        i += 1;
    }
    out
}

/// Field names and string literals of every `struct Name { .. }` body.
pub fn struct_bodies(toks: &[Tok]) -> Vec<(String, Vec<Tok>)> {
    let mut out = Vec::new();
    let mut i = 0;
    while i + 1 < toks.len() {
        if matches!(&toks[i], Tok::Ident(k) if k == "struct") {
            if let Tok::Ident(name) = &toks[i + 1] {
                let mut j = i + 2;
                while j < toks.len() {
                    match toks[j] {
                        Tok::Punct(';') => break,
                        Tok::Open('{') => {
                            let end = skip_balanced(toks, j);
                            out.push((name.clone(), toks[j + 1..end - 1].to_vec()));
                            break;
                        }
                        Tok::Open(_) => j = skip_balanced(toks, j),
                        _ => j += 1,
                    }
                }
            }
        }
        i += 1;
    }
    out
}

/// Whether a token slice mentions `word` as an identifier or string literal.
pub fn mentions(toks: &[Tok], word: &str) -> bool {
    toks.iter()
        .any(|t| matches!(t, Tok::Ident(x) | Tok::Str(x) if x == word))
}

/// String literals that are not just an argument to a pure query such as
/// `starts_with("..")`, `strip_prefix`, `ends_with` or `contains`.
pub fn constructing_strings(toks: &[Tok]) -> Vec<&str> {
    const QUERIES: [&str; 5] = [
        "starts_with",
        "strip_prefix",
        "ends_with",
        "contains",
        "find",
    ];
    let mut out = Vec::new();
    for (i, t) in toks.iter().enumerate() {
        if let Tok::Str(s) = t {
            let queried = i >= 2
                && toks[i - 1] == Tok::Open('(')
                && matches!(&toks[i - 2], Tok::Ident(n) if QUERIES.contains(&n.as_str()));
            if !queried {
                out.push(s.as_str());
            }
        }
    }
    out
}

/// Every string literal in `toks`.
pub fn strings(toks: &[Tok]) -> impl Iterator<Item = &str> {
    toks.iter().filter_map(|t| match t {
        Tok::Str(s) => Some(s.as_str()),
        _ => None,
    })
}

// ------------------------------------------------------------ Cargo manifests

fn toml_of(rel: &str) -> toml::Table {
    read(rel)
        .parse::<toml::Table>()
        .unwrap_or_else(|e| panic!("{rel} is not valid TOML: {e}"))
}

/// A normal dependency of a crate, resolved through `package = ".."` renames and
/// workspace inheritance.
#[derive(Debug)]
pub struct Dep {
    /// The key in the manifest.
    pub key: String,
    /// The real package name (`package = ".."` if renamed).
    pub package: String,
    /// A registry version requirement, from the member or the workspace entry.
    pub version: Option<String>,
    pub optional: bool,
    /// Effective `default-features` (Cargo ignores a member-level `false` when
    /// the workspace entry leaves defaults on).
    pub default_features: bool,
}

fn dep_fields(v: &toml::Value) -> (Option<String>, Option<String>, bool, Option<bool>, bool) {
    match v {
        toml::Value::String(s) => (None, Some(s.clone()), false, None, false),
        toml::Value::Table(t) => (
            t.get("package").and_then(|x| x.as_str()).map(str::to_owned),
            t.get("version").and_then(|x| x.as_str()).map(str::to_owned),
            t.get("optional")
                .and_then(toml::Value::as_bool)
                .unwrap_or(false),
            t.get("default-features")
                .or_else(|| t.get("default_features"))
                .and_then(toml::Value::as_bool),
            t.get("workspace")
                .and_then(toml::Value::as_bool)
                .unwrap_or(false),
        ),
        _ => (None, None, false, None, false),
    }
}

/// `[dependencies]` and `[target.*.dependencies]` of a member manifest. Never
/// `[dev-dependencies]`, `[build-dependencies]`, `[package.metadata]` or comments.
pub fn normal_dependencies(manifest_rel: &str) -> Vec<Dep> {
    let m = toml_of(manifest_rel);
    let root = toml_of("Cargo.toml");
    let ws = root
        .get("workspace")
        .and_then(|w| w.get("dependencies"))
        .and_then(|d| d.as_table())
        .cloned()
        .unwrap_or_default();
    let mut tables: Vec<&toml::Table> = Vec::new();
    if let Some(t) = m.get("dependencies").and_then(|d| d.as_table()) {
        tables.push(t);
    }
    if let Some(targets) = m.get("target").and_then(|t| t.as_table()) {
        for cfg in targets.values() {
            if let Some(t) = cfg.get("dependencies").and_then(|d| d.as_table()) {
                tables.push(t);
            }
        }
    }
    let mut out = Vec::new();
    for t in tables {
        for (key, v) in t {
            let (package, version, optional, df, inherits) = dep_fields(v);
            let (mut pkg, mut ver, mut default_features) = (package, version, df.unwrap_or(true));
            if inherits {
                if let Some(w) = ws.get(key) {
                    let (wp, wv, _, wdf, _) = dep_fields(w);
                    pkg = pkg.or(wp);
                    ver = ver.or(wv);
                    default_features = wdf.unwrap_or(true);
                }
            }
            out.push(Dep {
                key: key.clone(),
                package: pkg.unwrap_or_else(|| key.clone()),
                version: ver,
                optional,
                default_features,
            });
        }
    }
    out
}

/// The normal dependency on package `package` (after renames), if any.
pub fn dependency_on(manifest_rel: &str, package: &str) -> Option<Dep> {
    normal_dependencies(manifest_rel)
        .into_iter()
        .find(|d| d.package == package)
}

/// Whether `Cargo.lock` resolves `crate_name` to a dependency on `package`,
/// and `package` itself is a locked package.
pub fn locked_dependency(crate_name: &str, package: &str) -> bool {
    let lock = toml_of("Cargo.lock");
    let pkgs = lock
        .get("package")
        .and_then(|p| p.as_array())
        .cloned()
        .unwrap_or_default();
    let name_of = |p: &toml::Value| {
        p.get("name")
            .and_then(|n| n.as_str())
            .unwrap_or("")
            .to_owned()
    };
    let present = pkgs.iter().any(|p| name_of(p) == package);
    let linked = pkgs.iter().any(|p| {
        name_of(p) == crate_name
            && p.get("dependencies")
                .and_then(|d| d.as_array())
                .is_some_and(|d| {
                    d.iter()
                        .filter_map(|x| x.as_str())
                        .any(|x| x.split_whitespace().next() == Some(package))
                })
    });
    present && linked
}

/// Assert `crate_name` genuinely depends on `package` as a normal, non-optional
/// registry-versioned dependency that `Cargo.lock` resolves. Returns the
/// dependency for further checks.
pub fn assert_depends_on(row: &str, crate_name: &str, package: &str) -> Dep {
    let manifest = format!("crates/{crate_name}/Cargo.toml");
    let dep = dependency_on(&manifest, package).unwrap_or_else(|| {
        panic!("{row}: {manifest} [dependencies] has no dependency on package `{package}` (renames are resolved)")
    });
    assert!(
        !dep.optional,
        "{row}: the `{package}` dependency must not be optional"
    );
    assert!(
        dep.version.is_some(),
        "{row}: the `{package}` dependency needs a version (registry or workspace-inherited)"
    );
    assert!(
        locked_dependency(crate_name, package),
        "{row}: Cargo.lock does not resolve {crate_name} -> {package}"
    );
    dep
}
