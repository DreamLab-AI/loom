//! Migration test for ontomark step 10 (`ontomark/cli-publish`), EXP-10 rows LM-09 and LM-04
//! (decision D9, ADR-142).
//!
//! Red until:
//! - the publishing runbook builds with an `ontomark` release pinned by
//!   `ONTOMARK_RELEASE_TAG` + `ONTOMARK_SHA256` in `deploy/ontomark-release.env`,
//!   downloaded by tag and verified with `sha256sum --check` before it runs
//!   (LM-09; the estate pin convention of DEPRECATION §9.1);
//! - the operator error text in `loom-vector-ruvector` names `ontomark build`
//!   (LM-09);
//! - the scaffold-index and prose-index readers take their typed structs from
//!   `ontomark-compile` instead of private serde shapes (LM-04, ADR-142 §2,
//!   decision D9: step 10, not step 6).
//!
//! LM-06 (Python reload script, systemd unit) is NOT here: DEPRECATION §9 step 11
//! (`ontomark/reload-generation`) owns it, together with the Rust replacement
//! binary and the 16-behaviour fixture test.
//!
//! Manifests, `Cargo.lock` and the pin file are parsed (TOML, key=value); Rust is
//! lexed (comments gone, literals kept as tokens). Never imports ontomark.

mod support;

use support::{
    all_sources, assert_depends_on, constructing_strings, mentions, strip_cfg_test, struct_bodies,
    uses_item_of, Source, Tok,
};

/// Non-test library and binary code: inline `#[cfg(test)]` items and test-module
/// files are removed.
fn production_code() -> Vec<Source> {
    all_sources()
        .into_iter()
        .filter(|s| !(s.rel.ends_with("/tests.rs") || s.rel.ends_with("_tests.rs")))
        .map(|s| Source {
            toks: strip_cfg_test(&s.toks),
            rel: s.rel,
        })
        .collect()
}

// ------------------------------------------------------------------ LM-09

#[test]
fn lm09_operator_error_text_names_ontomark_build() {
    let mut offenders = Vec::new();
    for s in production_code() {
        for lit in constructing_strings(&s.toks) {
            if lit.contains("vault build") {
                offenders.push(format!(
                    "LM-09: {}: string literal {lit:?} still names `vault build`",
                    s.rel
                ));
            }
        }
    }
    assert!(offenders.is_empty(), "{}", offenders.join("\n"));
    let artifact = support::source("crates/loom-vector-ruvector/src/artifact.rs");
    let code = strip_cfg_test(&artifact.toks);
    assert!(
        constructing_strings(&code)
            .iter()
            .any(|l| l.contains("ontomark build --with-rvdb")),
        "LM-09: crates/loom-vector-ruvector/src/artifact.rs (promote_vault_build refusal, was :184) must tell the operator to run `ontomark build --with-rvdb`"
    );
}

/// `v<semver>[-pre]`, e.g. `v0.1.0` or `v0.1.0-rc.1`.
fn is_release_tag(tag: &str) -> bool {
    let Some(rest) = tag.strip_prefix('v') else {
        return false;
    };
    let (core, pre) = match rest.split_once('-') {
        Some((c, p)) => (c, Some(p)),
        None => (rest, None),
    };
    let parts: Vec<&str> = core.split('.').collect();
    let numeric = parts.len() == 3
        && parts
            .iter()
            .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()));
    let pre_ok = pre.is_none_or(|p| {
        !p.is_empty()
            && p.chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
    });
    numeric && pre_ok
}

/// `KEY=value` lines of the pin file; comments and quotes stripped.
fn pin_file() -> Vec<(String, String)> {
    support::read("deploy/ontomark-release.env")
        .lines()
        .filter_map(|l| {
            let l = l.split(" #").next().unwrap_or(l).trim();
            if l.is_empty() || l.starts_with('#') {
                return None;
            }
            let l = l.strip_prefix("export ").unwrap_or(l);
            let (k, v) = l.split_once('=')?;
            Some((
                k.trim().to_owned(),
                v.trim().trim_matches(['"', '\'']).to_owned(),
            ))
        })
        .collect()
}

fn pin(key: &str) -> String {
    pin_file()
        .into_iter()
        .rev()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v)
        .unwrap_or_default()
}

#[test]
fn lm09_ontomark_release_is_pinned_by_tag_and_digest() {
    let tag = pin("ONTOMARK_RELEASE_TAG");
    assert!(
        is_release_tag(&tag),
        "LM-09: deploy/ontomark-release.env must set ONTOMARK_RELEASE_TAG to a released tag `v<semver>[-pre]` (e.g. v0.1.0-rc.1), got {tag:?}"
    );
    let sha = pin("ONTOMARK_SHA256");
    assert!(
        sha.len() == 64 && sha.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f')),
        "LM-09: deploy/ontomark-release.env must set ONTOMARK_SHA256 to 64 lowercase hex characters, got {sha:?}"
    );
    for key in ["ONTOMARK_RELEASE_REPO", "ONTOMARK_ASSET"] {
        assert!(
            !pin(key).is_empty(),
            "LM-09: deploy/ontomark-release.env must set {key} (the estate pin convention)"
        );
    }
}

/// Commands of the runbook's fenced `bash` blocks: comment lines dropped,
/// backslash continuations joined.
fn runbook_commands() -> Vec<String> {
    let doc = support::read("docs/published-generation-reload.md");
    let mut out = Vec::new();
    let mut in_block = false;
    let mut pending = String::new();
    for line in doc.lines() {
        let t = line.trim_start();
        if t.starts_with("```") {
            if in_block {
                if !pending.is_empty() {
                    out.push(std::mem::take(&mut pending));
                }
                in_block = false;
            } else {
                in_block = t.trim_start_matches('`').trim() == "bash";
            }
            continue;
        }
        if !in_block || t.starts_with('#') {
            continue;
        }
        if let Some(head) = t.strip_suffix('\\') {
            pending.push_str(head);
            pending.push(' ');
        } else {
            pending.push_str(t);
            out.push(std::mem::take(&mut pending));
        }
    }
    out
}

#[test]
fn lm09_runbook_fetches_the_pinned_release_and_verifies_it_before_building() {
    let cmds = runbook_commands();
    let find = |pred: &dyn Fn(&str) -> bool| cmds.iter().position(|c| pred(c));
    let sources = find(&|c| {
        (c.starts_with("source ") || c.starts_with(". ")) && c.contains("ontomark-release.env")
    });
    let download = find(&|c| {
        c.contains("gh release download")
            && (c.contains("$ONTOMARK_RELEASE_TAG") || c.contains("${ONTOMARK_RELEASE_TAG}"))
    });
    let verify = find(&|c| {
        c.contains("sha256sum")
            && c.contains("--check")
            && (c.contains("$ONTOMARK_SHA256") || c.contains("${ONTOMARK_SHA256}"))
    });
    let build = find(&|c| {
        let words: Vec<&str> = c.split_whitespace().collect();
        words
            .iter()
            .any(|w| *w == "ontomark" || w.ends_with("/ontomark"))
            && words.contains(&"build")
    });
    assert!(
        sources.is_some(),
        "LM-09: runbook must `source` deploy/ontomark-release.env in a bash block"
    );
    assert!(download.is_some(), "LM-09: runbook must `gh release download \"$ONTOMARK_RELEASE_TAG\"` (a tag, not a cargo version)");
    assert!(
        verify.is_some(),
        "LM-09: runbook must verify the download with `sha256sum --check` against $ONTOMARK_SHA256"
    );
    assert!(
        build.is_some(),
        "LM-09: runbook must run `ontomark ... build`"
    );
    let (s, d, v, b) = (
        sources.unwrap(),
        download.unwrap(),
        verify.unwrap(),
        build.unwrap(),
    );
    assert!(
        s < d && d < v && v < b,
        "LM-09: runbook order must be source pin -> download -> sha256sum --check -> run `ontomark build` (got {s},{d},{v},{b})"
    );
}

#[test]
fn lm09_runbook_does_not_install_ontomark_with_cargo_or_build_vault() {
    let mut offenders = Vec::new();
    for c in runbook_commands() {
        if c.contains("cargo install") && c.contains("ontomark") {
            offenders.push(format!(
                "LM-09: `{c}`: cargo's --version takes a SemVer requirement, not the release tag `v<semver>`; fetch the pinned asset instead"
            ));
        }
        for stale in [
            "cargo build --release -p vault",
            "vault-target/release/vault",
        ] {
            if c.contains(stale) {
                offenders.push(format!(
                    "LM-09: runbook command `{c}` still builds or runs `vault` ({stale})"
                ));
            }
        }
    }
    assert!(offenders.is_empty(), "{}", offenders.join("\n"));
}

// ------------------------------------------------------------------ LM-04

/// A consumer may keep a public compatibility name while binding it directly
/// to the producer's type, including its application-owned trust parameters.
fn aliases_contract(toks: &[Tok], name: &str, contract: &str) -> bool {
    toks.windows(3).enumerate().any(|(i, declaration)| {
        if !matches!(&declaration[0], Tok::Ident(word) if word == "type")
            || !matches!(&declaration[1], Tok::Ident(word) if word == name)
            || declaration[2] != Tok::Punct('=')
        {
            return false;
        }
        let rhs = &toks[i + 3..];
        let end = rhs
            .iter()
            .position(|t| *t == Tok::Punct(';'))
            .unwrap_or(rhs.len());
        let path_end = rhs[..end]
            .iter()
            .position(|t| !matches!(t, Tok::Ident(_) | Tok::Punct(':')))
            .unwrap_or(end);
        let path = &rhs[..path_end];
        matches!(path.first(), Some(Tok::Ident(root)) if root == "ontomark_compile")
            && matches!(path.last(), Some(Tok::Ident(item)) if item == contract)
            && uses_item_of(path, "ontomark_compile", contract)
    })
}

#[test]
fn lm04_contract_aliases_bind_to_the_producer_without_private_wrappers() {
    assert!(aliases_contract(
        &support::lex("pub type RawIndex = ontomark_compile::bundle::ScaffoldIndex<A,M,C>;"),
        "RawIndex",
        "ScaffoldIndex"
    ));
    for impostor in [
        "type RawIndex = PrivateIndex; use ontomark_compile::bundle::ScaffoldIndex;",
        "type RawIndex = Wrapper<ontomark_compile::bundle::ScaffoldIndex>;",
        "type RawIndex = ontomark_compile::bundle::Unrelated;",
        "type RawIndex = ontomark_compile::bundle::Wrapper<ontomark_compile::bundle::ScaffoldIndex>;",
    ] {
        assert!(!aliases_contract(
            &support::lex(impostor),
            "RawIndex",
            "ScaffoldIndex"
        ));
    }
}

#[test]
fn lm04_scaffold_adopts_the_bundle_contract_without_the_reasoner() {
    let dep = assert_depends_on("LM-04", "loom-scaffold", "ontomark-compile");
    assert!(
        !dep.default_features,
        "LM-04: `ontomark-compile` must be taken with default-features = false in effect (the workspace entry decides when inherited): no reasoner in the serving path"
    );
}

#[test]
fn lm04_no_private_artefact_shape_survives_under_any_name() {
    // The shapes, not their names: the scaffold-index class (sup, isup, bl), its
    // envelope (version + generated + classes) and the prose index (dfull, cl, pages).
    let mut offenders = Vec::new();
    for s in production_code() {
        for (name, body) in struct_bodies(&s.toks) {
            let has = |w: &str| mentions(&body, w);
            let shape = if has("sup") && has("isup") && has("bl") {
                Some("scaffold-index class entry (sup, isup, bl)")
            } else if has("classes") && has("generated") && has("version") {
                Some("scaffold-index envelope (version, generated, classes)")
            } else if has("dfull") && has("cl") {
                Some("prose-index entry (dfull, cl)")
            } else if has("pages") {
                Some("prose-index envelope (pages)")
            } else {
                None
            };
            if let Some(shape) = shape {
                offenders.push(format!(
                    "LM-04: {}: `struct {name}` is a private {shape}",
                    s.rel
                ));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "take the typed artefact structs from ontomark-compile (ADR-142 §2):\n{}",
        offenders.join("\n")
    );
}

#[test]
fn lm04_surviving_importers_resolve_to_ontomark_compile() {
    // Deleting the private structs must not leave importers behind: every old
    // type name still referenced in production code has to be bound by a `use`
    // that comes from ontomark_compile (directly or as a re-export).
    let code = production_code();
    let mut offenders = Vec::new();
    for (name, contract) in [
        ("ClassEntry", "ClassEntry"),
        ("RawIndex", "ScaffoldIndex"),
        ("ProseEntry", "ProseEntry"),
        ("ProseFile", "ProseIndex"),
    ] {
        let referenced: Vec<&str> = code
            .iter()
            .filter(|s| s.toks.contains(&Tok::Ident(name.into())))
            .map(|s| s.rel.as_str())
            .collect();
        if referenced.is_empty() {
            continue;
        }
        let bound = code.iter().any(|s| {
            uses_item_of(&s.toks, "ontomark_compile", name)
                || aliases_contract(&s.toks, name, contract)
        });
        if !bound {
            offenders.push(format!(
                "LM-04: `{name}` is still referenced in {referenced:?} but no `use ontomark_compile::..{name}` binds it"
            ));
        }
    }
    assert!(offenders.is_empty(), "{}", offenders.join("\n"));
    assert!(
        code.iter().any(|s| s.rel.starts_with("crates/loom-scaffold/src/")
            && support::uses_crate(&s.toks, "ontomark_compile")),
        "LM-04: loom-scaffold production code must use `ontomark_compile::` in code (not a comment)"
    );
}
