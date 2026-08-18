# Figures report — paper-v2

Four diagrams-as-code figures, each a self-contained `\begin{figure}...\end{figure}` snippet.
All numbers cross-checked against `uplift-results/paper-v2/analysis.json` (single source of truth).
Every snippet compiles clean (pdflatex + latexmk, TeX Live 2025) against a replica of the paper
preamble; all four also compile together in one document with zero overfull hboxes.

Palette reused exactly as defined in `main.tex`: `teal0` RGB(0,110,110), `burnt` RGB(192,80,0),
`ink` RGB(25,28,32), `softgreen` RGB(46,139,87), `grayln` RGB(200,200,200), plus the `every axis`
styling block. No new colours introduced.

## Required preamble change (ONE line, for the sanitiser/integrator to add to main.tex)

The two pgfplots charts (`fig-forest`, `fig-controls`) need **no** new libraries — pgfplots error
bars are core. The two TikZ diagrams (`fig-ecosystem`, `fig-ceiling`) need TikZ libraries that
main.tex does not currently load. Add exactly:

```latex
\usetikzlibrary{positioning,arrows.meta,fit,calc,backgrounds}
```

(`positioning` = `right=of`/`below=of`; `arrows.meta` = `Stealth` arrow tips; `fit` = the "Loom
serving node" grouping box; `backgrounds` = the `on background layer` scope for that box; `calc` is
included defensively and is harmless.) `\resizebox` is already available because pgfplots pulls in
graphicx, so no `\usepackage{graphicx}` is required.

---

## Figures built

### 1. `fig-forest.tex` — `\label{fig:forest}`  [HIGHEST VALUE]
- **Story:** Study 2 per-set paired deltas (arcane +0.79, thin +0.30, general +0.05) and the pooled
  summary (+0.27, ink diamond), each with its bootstrap 95% CI, on a shared axis. The curation-depth
  gradient is visible at a glance, and the softgreen ±0.25 TOST equivalence band shows the general-set
  interval [0.00,+0.13] sitting wholly inside it — the equivalence reading made graphical.
- **Insertion:** §7.3 (`sec:live`), replacing/adjacent to Table~\ref{tab:live}. Best placed right
  after the paragraph ending "...the opposite of what a uniform prompt-formatting artefact would
  produce." It carries the same numbers as `tab:live`; the table can stay (stats detail) or the figure
  can substitute for it to save space. Recommend: figure after that paragraph, keep the table.
- **Libraries:** none beyond pgfplots.
- **Page budget:** ~6.0 cm tall incl. caption; ~0.30 page.

### 2. `fig-controls.tex` — `\label{fig:controls}`  [DECISIVE CONTROL, currently table-only]
- **Story:** The four negative-control contrasts against the bare model as a dot-with-CI ladder:
  true −raw +0.59 (teal, the one significant arm), masked +0.42, shuffled +0.25, irrelevant +0.04
  (all gray, ns). The descent to the verified-inert placebo at zero is the paper's decisive control,
  and it currently lives only in `tab:controls`.
- **Insertion:** §7.4 (`sec:controls`), after the "Three results triangulate the attribution"
  paragraph and near Table~\ref{tab:controls}. Recommend: figure after that paragraph, keep the table
  (the table also carries loom−true and true−irrelevant rows and the per-set gradient line).
- **Libraries:** none beyond pgfplots.
- **Page budget:** ~5.6 cm incl. caption; ~0.28 page.
- **Note:** could be merged with fig-forest as a two-panel figure, but they belong to different
  subsections (§7.3 vs §7.4), so separate placement reads better. Left as two figures.

### 3. `fig-ecosystem.tex` — `\label{fig:ecosystem}`  [supports §2.1, currently pure prose]
- **Story:** The full data path: knowledgeGraph corpus (8,138 Logseq pages → pure-TBox OWL 2, 8,146
  classes) → CI gate + Whelk EL reasoner → versioned generation (282,492-triple closure) → Loom
  serving node (lexical match 2.02 ms → confidence gate → scaffold injection → delegate to LAN model
  Qwen3.8-27B behind an OpenAI-compatible façade) → consumers. OntoCast enters as a dashed external
  upstream producer through the preview-first staged import. The HNSW semantic fallback is drawn in
  dashed burnt orange feeding the gate, labelled recall 0.816 < 0.87 floor · default-off.
- **Insertion:** §2.1 (`sec:ecosystem`), immediately after the "ecosystem in brief" paragraph
  (the one ending "...rather than a laboratory apparatus."). Gives that prose subsection its diagram.
- **Libraries:** `positioning, arrows.meta, fit, backgrounds` (and `calc`, defensive).
- **Page budget:** wrapped in `\resizebox{\textwidth}{!}{...}` so it never overflows; ~5.5 cm tall
  incl. caption; ~0.30 page.

### 4. `fig-ceiling.tex` — `\label{fig:ceiling}`  [optional, but clarifies the primary contribution]
- **Story:** One panel showing how e_i, c_i, r_i, g_i relate. The same shown context S_i feeds a
  no-op extractor (→ copy ceiling c_i = e_i/|G_i|) and the model (→ answer a_i → grounded recall r_i);
  both scored by the identical matcher m(t,·) against gold G_i (dashed reference); the gain over copy
  g_i = r_i − c_i on the right with the three-sign reading (g≈0 delivery, g<0 lossy, g>0 reasoning).
- **Insertion:** §5 (`sec:ceiling`), after §5.2 "Gain over copy" bullet list, before §5.3 controls.
  §5 is the paper's central definitional section and currently has no visual; the schematic makes the
  two-extractor construction concrete without adding claims.
- **Libraries:** `positioning, arrows.meta` (covered by the union above).
- **Page budget:** wrapped in `\resizebox{\textwidth}{!}{...}`; ~5.5 cm incl. caption; ~0.30 page.
- **Judgement:** kept because it is genuinely clarifying rather than decorative — it visualises the
  "same matcher, two inputs, signed difference" identity that the whole paper turns on. If page budget
  is tight this is the first to cut (the maths in §5.1 is self-contained).

**Total page impact:** ~1.1–1.2 pages across all four (each ~0.3 page). Figures 1–3 earn their space;
figure 4 is a judgement call the integrator can drop if the paper is over length.

---

## Candidates REJECTED

- **Ten-model gain-over-copy as a second view / dumbbell (raw→scaffold vs ceiling per model):**
  already covered by the existing `fig:gain` (horizontal bar) and `tab:sweep`. A dumbbell would
  duplicate the anchor bar `fig:anchor` and the sweep table without adding a reading. Rejected as
  redundant.
- **Telemetry panel (engagement rate 95.8/100/78.3%, injected tokens, latency medians):** real data
  in `analysis.json.telemetry`, but it corroborates the gate rather than carrying an argument, and the
  numbers are already stated inline in §7.3. A chart would be decorative. Rejected.
- **Empty-rate / attrition bar (irrelevant 50%, masked 41%, true 43%, shuffled 36% at 1536 budget):**
  a genuine finding (scaffold-length × reasoning-budget interaction), but it is a reporting-deviation
  caveat, not a result; a figure would over-weight it relative to its role. Left as prose in §7
  "Reporting deviations". Rejected (borderline — could be a small inset if a reviewer asks).
- **Out-of-domain five-model judged arm (`tab:ood`) as a forest:** every interval includes zero and
  the story is "no jaggedness detected"; a forest of six null-spanning intervals communicates less
  than the table and risks implying an effect where the claim is deliberately equivalence-only.
  Rejected; the general-set equivalence reading is already carried graphically by `fig-forest`.
- **Multivariate-bar radar/spider (`tab:bar`, four axes, two measured):** two of four axes are
  explicitly unmeasured; a radar would draw a shape over unmeasured axes and imply a completeness the
  paper is careful to disclaim. Rejected on honesty grounds — the table's "Measured / Specified, not
  measured" column is the correct instrument.

---

## PaperBanana (raster VLM figure) — NOT viable out of the box

`paperbanana` is not on PATH (`command not found`); the skill ships under
`project/agentbox/skills/paperbanana` but exposes no working CLI in this environment. Both
`GOOGLE_API_KEY` and `OPENAI_API_KEY` are set, but per the brief a raster overview is only worth it if
the CLI works with zero setup, and it does not. No attempt spent beyond the PATH check. TikZ vector
source (preferred for arXiv anyway) delivers the ecosystem overview as `fig-ecosystem.tex`.

---

## Verification log

- Scratch wrapper: `.../scratchpad/figtest/header.tex` replicates the main.tex preamble
  (documentclass article 11pt, pgfplots compat 1.18, all five colour defs, the `every axis` /
  rawstyle/scafstyle/copystyle block, and the tikzlibrary line above).
- Each figure built standalone via `pdflatex -halt-on-error`; all four also built together via
  `latexmk -pdf` (2 pages, zero overfull hboxes). Renders visually inspected at 130–135 dpi:
  no label collisions, colours correct (teal significant / gray ns / ink pooled diamond),
  equivalence band and zero lines placed correctly.
- Undefined `\ref`/`\S\ref` in the standalone builds are expected (section labels live in main.tex)
  and resolve once the snippets are `\input` into the paper. No `\cite` used in any caption.
