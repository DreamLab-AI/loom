//! The nouns the whole Loom speaks. Serde-serialisable because they cross the
//! HTTP and mirror boundaries, but they carry NO I/O. THE PRIZE — the
//! human-scrutible markdown-with-ontology block — is `CanonicalUnit`; every
//! other type is a projection that resolves back to an `Iri` (Invariant I-P1).

// --- identity ---------------------------------------------------------------

/// A concept-class IRI, e.g. `urn:ngm:class:knowledge-graph`. The addressing key
/// for EVERY `CanonicalUnit`. Newtype so an adapter can never hand back a bare
/// String and pretend it is an answer (I-P1 at the type level).
#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct Iri(String);

impl Iri {
    /// Wrap an arbitrary reference string. Tolerates both the full
    /// `urn:ngm:class:<slug>` form and a bare `<slug>` (the `_ref_to_slug`
    /// leniency carried from Python).
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// The kebab slug after the last `:` — the join key across ttl / scaffold /
    /// prose / HNSW projections. A bare slug (no `:`) returns itself.
    pub fn slug(&self) -> &str {
        self.0.rsplit(':').next().unwrap_or(&self.0)
    }

    /// Build the canonical `urn:ngm:class:<slug>` IRI from a bare slug.
    pub fn from_slug(slug: &str) -> Self {
        Self(format!("urn:ngm:class:{slug}"))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for Iri {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for Iri {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

// --- THE PRIZE, as a type ---------------------------------------------------

/// The canonical served unit — a per-IRI markdown-with-ontology block. Aggregate
/// root of the bounded context. `dfull`, `landscape` and `corpus_nature` are
/// never dropped by a compact serialiser (the frame forbids degrading legibility).
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct CanonicalUnit {
    pub iri: Iri,
    pub title: String,               // scaffold "t"
    pub definition: String,          // scaffold "d" (<=400 chars, truncated)
    pub dfull: Option<String>,       // prose "dfull" — untruncated curated prose (THE PRIZE body)
    pub landscape: Option<String>,   // prose "cl" — Current Landscape research prose
    pub domain: Option<String>,      // "dom"
    pub maturity: Option<String>,    // "m"
    pub quality: Option<f32>,        // "q"
    pub is_a: Vec<Iri>,              // "sup" (direct parents)
    pub ancestors: Vec<Iri>,         // "isup" (inferred ancestors from the reasoned closure)
    pub relations: Vec<Relation>,    // typed ontology-relation header ("rel")
    pub backlinks: Vec<Iri>,         // "bl"
    pub corpus_nature: CorpusNature, // provenance stamp
    pub generation: GenerationId,    // which build this unit belongs to
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Relation {
    pub predicate: RelationKind,
    pub targets: Vec<Iri>,
}

/// The 12 ordered relation types from `REL_ORDER` + an open `Other(String)` tail.
/// Round-trips as the hyphenated scaffold-index predicate strings (`has-part`,
/// `depends-on`, …) via the manual serde impls below — NOT the derive, so the
/// `Other` tail stays a plain string on the wire rather than a tagged object.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RelationKind {
    HasPart,
    Requires,
    Enables,
    DependsOn,
    Implements,
    Uses,
    PartOf,
    RelatedTo,
    BridgesTo,
    Supports,
    StandardizedBy,
    ContrastsWith,
    Other(String),
}

impl RelationKind {
    /// The on-the-wire predicate string. `Other` carries its own literal.
    pub fn as_predicate(&self) -> &str {
        match self {
            Self::HasPart => "has-part",
            Self::Requires => "requires",
            Self::Enables => "enables",
            Self::DependsOn => "depends-on",
            Self::Implements => "implements",
            Self::Uses => "uses",
            Self::PartOf => "part-of",
            Self::RelatedTo => "related-to",
            Self::BridgesTo => "bridges-to",
            Self::Supports => "supports",
            Self::StandardizedBy => "standardized-by",
            Self::ContrastsWith => "contrasts-with",
            Self::Other(s) => s.as_str(),
        }
    }

    /// Parse a predicate string; unknown predicates fall to `Other`.
    pub fn from_predicate(s: &str) -> Self {
        match s {
            "has-part" => Self::HasPart,
            "requires" => Self::Requires,
            "enables" => Self::Enables,
            "depends-on" => Self::DependsOn,
            "implements" => Self::Implements,
            "uses" => Self::Uses,
            "part-of" => Self::PartOf,
            "related-to" => Self::RelatedTo,
            "bridges-to" => Self::BridgesTo,
            "supports" => Self::Supports,
            "standardized-by" => Self::StandardizedBy,
            "contrasts-with" => Self::ContrastsWith,
            other => Self::Other(other.to_owned()),
        }
    }
}

impl serde::Serialize for RelationKind {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_predicate())
    }
}

impl<'de> serde::Deserialize<'de> for RelationKind {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = <String as serde::Deserialize>::deserialize(deserializer)?;
        Ok(Self::from_predicate(&s))
    }
}

/// `corpusNature`: synthetic-ai-generated-human-directed. The provenance the
/// reviewer needs to trust the prose. Never dropped on serialise.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum CorpusNature {
    SyntheticAiGeneratedHumanDirected,
}

// --- the retrieval nouns ----------------------------------------------------

/// One scored candidate from ANY retriever. `iri` is mandatory; `score` is the
/// retriever's own confidence in its own scale. `provenance` records WHICH
/// engine surfaced it (audit + fusion telemetry). Serialises to the §9
/// `{iri, score, provenance}` seed shape (the doc's `Clone`-only derive cannot
/// satisfy `Scaffold`'s derived `Serialize` — minimal deviation).
#[derive(Clone, Debug, serde::Serialize)]
pub struct ConceptMatch {
    pub iri: Iri,
    pub score: f32,
    pub provenance: MatchProvenance,
}

impl ConceptMatch {
    /// Map the retriever's raw score onto the comparable scale the injection
    /// gate reads. Identity for now — the lexical↔cosine normalisation constant
    /// (`LOOM_SEMANTIC_SCORE_SCALE`) is bench-tuned and lives in the facade
    /// config, not baked in here.
    pub fn score_normalised(&self) -> f32 {
        self.score
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub enum MatchProvenance {
    Lexical,
    SemanticHnsw,
}

/// The assembled, budget-clamped `[ONTOLOGY CONTEXT] … [END …]` block — the
/// exact string injected into the system message. Carries the grounding
/// telemetry, now typed.
#[derive(Clone, Debug, serde::Serialize)]
pub struct Scaffold {
    pub block: String, // the markdown; "" ⇒ nothing injected
    pub engaged: bool,
    pub approx_tokens: usize,
    pub seeds: Vec<ConceptMatch>, // which units injected, with scores + provenance
    pub top_score: f32,
    pub effective_budget: usize,
    pub fusion_path: FusionPath, // lexical-only | semantic-fallback | none
    pub generation: GenerationId,
}

impl Scaffold {
    /// The "nothing injected" scaffold — caller falls back to the raw prompt.
    /// `generation` is the (lexical) generation this empty answer is stamped to.
    #[must_use]
    pub fn empty(fusion_path: FusionPath, generation: Generation) -> Self {
        Self {
            block: String::new(),
            engaged: false,
            approx_tokens: 0,
            seeds: Vec::new(),
            top_score: 0.0,
            effective_budget: 0,
            fusion_path,
            generation: generation.id,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub enum FusionPath {
    LexicalHit,
    SemanticFallback,
    NoMatch,
}

/// How the answer was DELIVERED — orthogonal to `FusionPath` (which records how
/// candidates were RETRIEVED). A verbatim serve still arrives via `LexicalHit`
/// retrieval; overloading `FusionPath` with a `Verbatim` variant would conflate
/// the retrieval axis with the delivery axis (and perturb every existing
/// `FusionPath` serialisation/test). A distinct type keeps the two concerns
/// separable in the telemetry — the paper's finding is precisely that these are
/// different decisions (retrieve-and-restate vs serve-the-scaffold).
///
/// Serialises lowercase (`"delegated"`/`"verbatim"`) to sit beside the existing
/// lowercase `loom.mode` telemetry rather than the CamelCase enum-variant form.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ServedMode {
    /// The scaffold was injected and the request delegated to the model backend
    /// (the current, default behaviour).
    Delegated,
    /// The high-confidence scaffold was served verbatim WITHOUT calling the
    /// backend — the paper's serving-regime finding realised (F1).
    Verbatim,
}

/// Exposure telemetry (F2): after an answer returns, how many of the injected
/// scaffold's titles the answer actually restated. `targets` is the count of
/// distinct served titles (class titles + serialised relation-target titles that
/// survived the budget clamp), `delivered` how many of those appear in the
/// answer, and `dropped` the (capped) list of served-but-omitted titles. This is
/// the paper's copy-fidelity deficit made observable per request (~1 in 14
/// exposed items dropped, n10).
#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct ExposureReport {
    pub targets: usize,
    pub delivered: usize,
    pub dropped: Vec<String>,
}

/// Knobs for a scaffold assembly (the Python `scaffold()` arguments, typed).
/// `path` is threaded so the assembled `Scaffold` records which fusion route
/// produced it; use `with_path` to stamp it inside the fusion pipeline.
#[derive(Clone, Debug)]
pub struct ScaffoldOpts {
    pub budget_tokens: usize,
    pub hops: usize,
    pub prose: bool,
    pub confidence_injection: bool,
    pub max_seeds: usize,
    pub k_semantic: usize,
    pub path: FusionPath,
}

impl ScaffoldOpts {
    /// Stamp the fusion route this assembly is taking (consumed builder-style).
    #[must_use]
    pub fn with_path(mut self, path: FusionPath) -> Self {
        self.path = path;
        self
    }
}

impl Default for ScaffoldOpts {
    fn default() -> Self {
        // Defaults mirror the §10 config table: ONTOLOGY_BUDGET=1500,
        // LOOM_SEMANTIC_K=5, LOOM_CONFIDENCE_INJECTION=0.
        Self {
            budget_tokens: 1500,
            hops: 1,
            prose: true,
            confidence_injection: false,
            max_seeds: 6,
            k_semantic: 5,
            path: FusionPath::NoMatch,
        }
    }
}

// --- the graph + backend value types (§4 supporting types) ------------------

/// A read-only SPARQL result, engine-neutral. `boolean` is set for ASK;
/// `columns`/`rows` for SELECT; `truncated` flags a server-side row-cap hit.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct SparqlResult {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub boolean: Option<bool>,
    pub truncated: bool,
}

/// A label/substring hit from the graph store's `rdfs:label` / `skos:prefLabel`
/// / `ngm:title` search. Resolves to an addressable `Iri` (I-P1).
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct LabelHit {
    pub iri: Iri,
    pub label: String,
    pub predicate: String,
}

/// The graph adapter's health, surfaced verbatim in `/health`.
#[derive(Clone, Debug, serde::Serialize)]
pub struct GraphStatus {
    pub available: bool,
    pub triples: usize,
    pub loaded_files: Vec<String>,
    pub error: Option<String>,
}

/// The model backend's reply — HTTP status + the raw OpenAI-shaped JSON body,
/// so the facade can annotate the 200 with its `loom:{…}` block or propagate a
/// labelled failure.
#[derive(Clone, Debug, serde::Serialize)]
pub struct BackendResponse {
    pub status: u16,
    pub body: serde_json::Value,
}

/// Chain-hash id of an attestation-ledger entry (build/CI path only).
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LedgerEntryId(pub String);

// --- the generation boundary (ADR-135 D2.1) ---------------------------------

/// Content-addressed corpus snapshot identity. Two units with different
/// `GenerationId` must NEVER be served together (never-mixed-build).
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct GenerationId(pub String); // commitSha||buildId, or the mirror generation ISO stamp

/// The full generation descriptor the mirror promotes and `/health` reports.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Generation {
    pub id: GenerationId,
    pub source: GenerationSource, // build-manifest | mirror-manifest | scaffold-index
    pub generated_at: Option<String>,
    pub commit_sha: Option<String>,
    pub promoted_at: Option<String>,
    pub cluster_span_seconds: Option<f64>,
    pub artifacts: Vec<ArtifactSha>, // per-artifact sha256 (never-mixed proof)
    pub verified_single_generation: bool,
    pub class_count: Option<usize>,
}

impl PartialEq for Generation {
    /// Generation equality is IDENTITY equality — two descriptors are the same
    /// generation iff their `id` matches. The fusion parity guard (§6) compares
    /// generations this way; the descriptor's mutable metadata is irrelevant.
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ArtifactSha {
    pub name: String,
    pub sha256: String,
    pub bytes: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum GenerationSource {
    BuildManifest,
    MirrorManifest,
    ScaffoldIndex,
    Unavailable,
}

// --- the gate verdict (ADR-136 D5) ------------------------------------------

/// Outcome of a domain-predicate check (SSOT/conflict predicates stay
/// Loom-owned). On the build/CI path this becomes a chain-hashed
/// `AttestationLedger` entry.
#[derive(Clone, Debug, serde::Serialize)]
pub struct GateVerdict {
    pub predicate: String, // e.g. "class_count_parity", "no_mixed_generation"
    pub passed: bool,
    pub detail: Option<String>,
    pub subject: Option<Iri>, // the unit/generation the predicate ran over
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iri_slug_roundtrip() {
        let iri = Iri::from_slug("knowledge-graph");
        assert_eq!(iri.as_str(), "urn:ngm:class:knowledge-graph");
        assert_eq!(iri.slug(), "knowledge-graph");
        // from_slug ∘ slug is stable on the canonical form.
        assert_eq!(Iri::from_slug(iri.slug()), iri);
    }

    #[test]
    fn iri_bare_slug_tolerance() {
        // A bare slug (no `:`) resolves to itself — the _ref_to_slug leniency.
        let bare = Iri::new("rgb-protocol");
        assert_eq!(bare.slug(), "rgb-protocol");
        // Full urn and bare slug agree on the join key.
        assert_eq!(Iri::from_slug("rgb-protocol").slug(), bare.slug());
    }

    #[test]
    fn iri_serde_roundtrip() {
        let iri = Iri::from_slug("colour-channel");
        let json = serde_json::to_string(&iri).unwrap();
        assert_eq!(json, "\"urn:ngm:class:colour-channel\"");
        let back: Iri = serde_json::from_str(&json).unwrap();
        assert_eq!(back, iri);
    }

    #[test]
    fn relation_kind_predicate_roundtrip() {
        let predicates = [
            ("has-part", RelationKind::HasPart),
            ("requires", RelationKind::Requires),
            ("enables", RelationKind::Enables),
            ("depends-on", RelationKind::DependsOn),
            ("implements", RelationKind::Implements),
            ("uses", RelationKind::Uses),
            ("part-of", RelationKind::PartOf),
            ("related-to", RelationKind::RelatedTo),
            ("bridges-to", RelationKind::BridgesTo),
            ("supports", RelationKind::Supports),
            ("standardized-by", RelationKind::StandardizedBy),
            ("contrasts-with", RelationKind::ContrastsWith),
        ];
        for (wire, variant) in predicates {
            // string → variant
            let parsed = RelationKind::from_predicate(wire);
            assert_eq!(parsed, variant, "from_predicate({wire})");
            // variant → string
            assert_eq!(variant.as_predicate(), wire, "as_predicate for {wire}");
            // serde round-trip through JSON keeps the plain string form.
            let json = serde_json::to_string(&variant).unwrap();
            assert_eq!(json, format!("\"{wire}\""));
            let back: RelationKind = serde_json::from_str(&json).unwrap();
            assert_eq!(back, variant);
        }
    }

    #[test]
    fn relation_kind_other_tail() {
        let k: RelationKind = serde_json::from_str("\"mentions\"").unwrap();
        assert_eq!(k, RelationKind::Other("mentions".to_owned()));
        // Other serialises as a bare string, NOT a tagged {"Other": …} object.
        assert_eq!(serde_json::to_string(&k).unwrap(), "\"mentions\"");
    }

    #[test]
    fn scaffold_empty_shape() {
        let gen = Generation {
            id: GenerationId("build-abc".to_owned()),
            source: GenerationSource::ScaffoldIndex,
            generated_at: None,
            commit_sha: None,
            promoted_at: None,
            cluster_span_seconds: None,
            artifacts: Vec::new(),
            verified_single_generation: false,
            class_count: None,
        };
        let s = Scaffold::empty(FusionPath::NoMatch, gen);
        assert!(s.block.is_empty());
        assert!(!s.engaged);
        assert_eq!(s.approx_tokens, 0);
        assert!(s.seeds.is_empty());
        assert_eq!(s.effective_budget, 0);
        assert_eq!(s.fusion_path, FusionPath::NoMatch);
        assert_eq!(s.generation, GenerationId("build-abc".to_owned()));
    }

    #[test]
    fn generation_id_equality() {
        let a = GenerationId("sha-1||b1".to_owned());
        let b = GenerationId("sha-1||b1".to_owned());
        let c = GenerationId("sha-2||b2".to_owned());
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn generation_equality_is_identity() {
        // Two descriptors with the same id but different metadata are equal;
        // different ids are not (the never-mixed-build parity guard).
        let base = |id: &str, count: Option<usize>| Generation {
            id: GenerationId(id.to_owned()),
            source: GenerationSource::BuildManifest,
            generated_at: None,
            commit_sha: None,
            promoted_at: None,
            cluster_span_seconds: None,
            artifacts: Vec::new(),
            verified_single_generation: true,
            class_count: count,
        };
        assert_eq!(base("g1", Some(10)), base("g1", Some(9999)));
        assert_ne!(base("g1", Some(10)), base("g2", Some(10)));
    }

    #[test]
    fn served_mode_serialises_lowercase() {
        assert_eq!(
            serde_json::to_string(&ServedMode::Delegated).unwrap(),
            "\"delegated\""
        );
        assert_eq!(
            serde_json::to_string(&ServedMode::Verbatim).unwrap(),
            "\"verbatim\""
        );
    }

    #[test]
    fn exposure_report_shape() {
        let r = ExposureReport {
            targets: 3,
            delivered: 2,
            dropped: vec!["Graph Database".to_owned()],
        };
        let v = serde_json::to_value(&r).unwrap();
        assert_eq!(v["targets"], 3);
        assert_eq!(v["delivered"], 2);
        assert_eq!(v["dropped"], serde_json::json!(["Graph Database"]));
        // Default is the honest empty report.
        let d = ExposureReport::default();
        assert_eq!(d.targets, 0);
        assert!(d.dropped.is_empty());
    }

    #[test]
    fn concept_match_score_normalised_is_identity() {
        let m = ConceptMatch {
            iri: Iri::from_slug("x"),
            score: 0.87,
            provenance: MatchProvenance::SemanticHnsw,
        };
        assert!((m.score_normalised() - 0.87).abs() < f32::EPSILON);
    }
}
