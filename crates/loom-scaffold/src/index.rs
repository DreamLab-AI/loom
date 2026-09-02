//! `ScaffoldIndex` — the inverted title-word index, ported 1:1 from Python's
//! `ScaffoldIndex.__init__`, plus the shared tokenisers (`slugify`,
//! `_ref_to_slug`, `_WORD_RE`, `_SLUG_RE`) and the char-precise text helpers
//! (`_est_tokens`, `_truncate`). All string arithmetic is over Unicode code
//! points (Python `str` semantics), never bytes.

use std::collections::HashSet;
use std::sync::OnceLock;

use indexmap::IndexMap;
use regex::Regex;
use serde::Deserialize;

/// The on-disk `scaffold-index.json` class shape (`t,d,dom,q,m,sup,isup,rel,bl`).
/// Every field is optional on disk; `rel` preserves the camelCase predicate keys
/// verbatim (they are emitted into the block unchanged — byte-parity).
#[derive(Debug, Clone, Deserialize)]
pub struct ClassEntry {
    #[serde(default)]
    pub t: Option<String>,
    #[serde(default)]
    pub d: Option<String>,
    #[serde(default)]
    pub dom: Option<String>,
    #[serde(default)]
    pub q: Option<f64>,
    #[serde(default)]
    pub m: Option<String>,
    #[serde(default)]
    pub sup: Vec<String>,
    #[serde(default)]
    pub isup: Vec<String>,
    #[serde(default)]
    pub rel: IndexMap<String, Vec<String>>,
    #[serde(default)]
    pub bl: Vec<String>,
}

/// The `{version, generated, counts, classes}` envelope of `scaffold-index.json`
/// v1 — the SAME shape the golden `fixture.json` uses.
#[derive(Debug, Clone, Deserialize)]
pub struct RawIndex {
    #[serde(default)]
    pub version: Option<i64>,
    #[serde(default)]
    pub generated: String,
    #[serde(default)]
    pub classes: IndexMap<String, ClassEntry>,
}

/// In-memory scaffold index with an inverted title-word index. Load is
/// O(index size); `match_` touches only inverted-surfaced classes plus one
/// slug-substring pass.
#[derive(Debug)]
pub struct ScaffoldIndex {
    pub generated: String,
    /// slug -> class entry, INSERTION-ORDERED (Python dict order; `by_title`
    /// first-wins and the `slugs` scan both depend on it).
    pub classes: IndexMap<String, ClassEntry>,
    /// slugified-title -> slug (exact title lookup; first insertion wins).
    by_title: IndexMap<String, String>,
    /// title word -> set of slugs (inverted index).
    inverted: IndexMap<String, HashSet<String>>,
    /// slug -> number of title words (>=1), for overlap normalisation.
    title_len: IndexMap<String, usize>,
    /// slug list in insertion order (the substring scan iterates this).
    slugs: Vec<String>,
}

// --- shared regexes (compiled once) -----------------------------------------

fn word_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"[a-z0-9]+").unwrap())
}

fn slug_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"[^a-z0-9]+").unwrap())
}

fn slug_full_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^[a-z0-9-]+$").unwrap())
}

/// `_WORD_RE.findall(s.lower())` — lowercase, then every `[a-z0-9]+` run.
#[must_use]
pub fn find_words(s: &str) -> Vec<String> {
    let lower = s.to_lowercase();
    word_re()
        .find_iter(&lower)
        .map(|m| m.as_str().to_owned())
        .collect()
}

/// Kebab-case slug, identical to the index build rule:
/// `_SLUG_RE.sub("-", s.lower()).strip("-")`.
#[must_use]
pub fn slugify(s: &str) -> String {
    let lower = s.to_lowercase();
    let replaced = slug_re().replace_all(&lower, "-");
    replaced.trim_matches('-').to_owned()
}

/// Map a slug or `urn:ngm:class:<slug>` IRI to its slug (`_ref_to_slug`).
#[must_use]
pub fn ref_to_slug(reference: &str) -> String {
    let tail = if reference.contains(':') {
        reference.rsplit(':').next().unwrap_or(reference)
    } else {
        reference
    };
    if slug_full_re().is_match(tail) {
        tail.to_owned()
    } else {
        slugify(tail)
    }
}

/// Cheap token estimate: `(char_count + 3) // 4` (Python `len` is code points).
#[must_use]
pub fn est_tokens(text: &str) -> usize {
    text.chars().count().div_ceil(4)
}

/// `_truncate` — code-point-precise. Prefer a sentence boundary, then a word
/// boundary; append `…` only on the word/hard-cut branch (never the sentence
/// branch), exactly as Python.
#[must_use]
pub fn truncate(text: &str, limit: usize) -> String {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= limit {
        return text.to_owned();
    }
    let cut = &chars[..limit];
    let half = limit / 2;

    // rfind(". ") over the cut slice → char index of the '.'.
    if let Some(dot) = rfind_seq(cut, &['.', ' ']) {
        if dot >= half {
            return cut[..=dot].iter().collect();
        }
    }
    // rfind(" ") → char index of the last space.
    let mut end = limit;
    if let Some(space) = rfind_char(cut, ' ') {
        if space >= half {
            end = space;
        }
    }
    let mut out: String = cut[..end].iter().collect();
    // Python str.rstrip() strips all trailing Unicode whitespace.
    let keep = out.trim_end().len();
    out.truncate(keep);
    out.push('…');
    out
}

fn rfind_char(hay: &[char], needle: char) -> Option<usize> {
    hay.iter().rposition(|&c| c == needle)
}

fn rfind_seq(hay: &[char], needle: &[char]) -> Option<usize> {
    if needle.is_empty() || hay.len() < needle.len() {
        return None;
    }
    let last = hay.len() - needle.len();
    (0..=last)
        .rev()
        .find(|&i| &hay[i..i + needle.len()] == needle)
}

impl ScaffoldIndex {
    /// Build from a parsed `RawIndex`. Rejects any version != 1 (Python raises).
    pub fn from_raw(raw: RawIndex) -> Result<Self, String> {
        if raw.version != Some(1) {
            return Err(format!(
                "unsupported scaffold-index version: {:?}",
                raw.version
            ));
        }
        let classes = raw.classes;
        let mut by_title: IndexMap<String, String> = IndexMap::new();
        let mut inverted: IndexMap<String, HashSet<String>> = IndexMap::new();
        let mut title_len: IndexMap<String, usize> = IndexMap::new();
        let slugs: Vec<String> = classes.keys().cloned().collect();

        for (slug, entry) in &classes {
            let title = entry
                .t
                .clone()
                .filter(|t| !t.is_empty())
                .unwrap_or_else(|| slug.clone());
            // setdefault: first insertion wins.
            by_title
                .entry(slugify(&title))
                .or_insert_with(|| slug.clone());
            let words = find_words(&title);
            title_len.insert(slug.clone(), words.len().max(1));
            for w in words {
                inverted.entry(w).or_default().insert(slug.clone());
            }
        }

        Ok(Self {
            generated: raw.generated,
            classes,
            by_title,
            inverted,
            title_len,
            slugs,
        })
    }

    /// Parse from a JSON string (scaffold-index.json OR the golden fixture.json).
    pub fn from_json_str(s: &str) -> Result<Self, String> {
        let raw: RawIndex = serde_json::from_str(s).map_err(|e| e.to_string())?;
        Self::from_raw(raw)
    }

    // -- accessors used by the matcher/serialiser -----------------------------

    #[must_use]
    pub fn by_title(&self, key: &str) -> Option<&str> {
        self.by_title.get(key).map(String::as_str)
    }

    #[must_use]
    pub fn inverted(&self, word: &str) -> Option<&HashSet<String>> {
        self.inverted.get(word)
    }

    #[must_use]
    pub fn title_len(&self, slug: &str) -> usize {
        self.title_len.get(slug).copied().unwrap_or(1)
    }

    #[must_use]
    pub fn slugs(&self) -> &[String] {
        &self.slugs
    }

    #[must_use]
    pub fn contains(&self, slug: &str) -> bool {
        self.classes.contains_key(slug)
    }

    #[must_use]
    pub fn get(&self, slug: &str) -> Option<&ClassEntry> {
        self.classes.get(slug)
    }

    /// `title_of`: the class title, else the slug itself (for off-index refs).
    #[must_use]
    pub fn title_of(&self, slug: &str) -> String {
        match self.classes.get(slug) {
            Some(e) => {
                e.t.clone()
                    .filter(|t| !t.is_empty())
                    .unwrap_or_else(|| slug.to_owned())
            }
            None => slug.to_owned(),
        }
    }

    #[must_use]
    pub fn class_count(&self) -> usize {
        self.classes.len()
    }
}
