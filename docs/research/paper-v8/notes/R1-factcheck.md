# R1 fact-check: line-anchored verification of REVIEW-2026-09-21-external.md against main.tex (v7, reviewed)

Source: `/home/devuser/workspace/loom/docs/research/paper-v6/main.tex` (1071 lines).
Method: full read of main.tex (two passes, lines 1–398 and 399–1071) plus targeted `grep -n` for every term the review names. Section numbers below are the paper's own `\section` order (Intro=1 … Conclusion=20), reconstructed because the manuscript has no printed section numbers; this ordering is what makes the review's own "§9", "§12", "§18", "§19" etc. resolve, and it lines up exactly, so I use it throughout to anchor the review's prose to `\label`s.

Section index (line of `\section`/`\subsection`, with the review's implied number):
1 Intro L50 · 2 Related L60 · 3 Ceiling L75 (3.1 L78 def, 3.2 L98 accounting, 3.3 L114 gain, 3.4 L125 controls, 3.5 L135 prior) · 4 Rescore L141 · 5 Runnable L144 · 6 Paraphrase L153 · 7 Method L191 · 8 Results L205 (8.1 indomain L208, 8.2 tenmodel L227, 8.3 live L329, 8.4 judgefamily L360, 8.5 controls L382, 8.6 ood L440) · 9 Analysis L464 (9.1 suppression L467) · 10 CaseStudy L481 (10.1 pagejudge L554) · 11 Composition L577 (11.1 gemma L632) · 12 Routing L677 · 13 Setting L726 · 14 System L794 · 15 Practitioner L822 · 16 Lifecycle L849 · 17 Roadmap L893 (17.1 bar L896) · 18 Datalake L920 · 19 Limits L938 · 20 Conclusion L958.

---

## 1. Internal-errors table (review's own summary table, lines 253–262)

### 1a. Literal `emphexposure`/`emphcopy`/`emphgain` in the abstract

**VERIFIED.** Line 47, three occurrences, each caused by `\\emph{...}` (double backslash — a LaTeX line-break command consuming the `\emph`, leaving `emph{...}` as literal text, so braces group but do not typeset a command):

> `We propose \\emph{exposure accounting}: ...`
> `The \\emph{copy ceiling}, the recall a verbatim copy ... `
> `... the signed \\emph{gain over copy} its net;`

Confirmed against `main.pdf` rendering intent: `\\` before `emph` breaks the line and drops the backslash that would invoke the command, so the rendered text reads "emphexposure accounting", "emphcopy ceiling", "emphgain over copy" — exactly the review's complaint. Fix: delete the stray backslash (`\\emph` → `\emph`) at all three sites, all on line 47.

### 1b. "8.1 points" arithmetic in §Routing (12)

**VERIFIED — the review is right, the paper is wrong.** Line 691:

> "Together the two defects manufactured $8.1$ of the original $11.6$ points."

But the paper's own numbers (L687, L688) are $11.6\to9.3$ (defect 1) then $9.3\to4.7$ (defect 2), i.e. total shrinkage $11.6-4.7=6.9$, not $8.1$. $8.1$ appears nowhere else and has no other derivation in the section. This is a bare arithmetic error to fix in place at L691.

### 1c. "lower bound" applied to absolute recall (review: §3 vs §§7, 9.1, 19)

**VERIFIED**, three sites, matching the review's own §-numbers exactly:

- §3 (Ceiling, the "correct" framing the review credits) — L91: "Absolute recall is therefore neither a strict lower nor upper bound on semantic correctness" (A2, correctly hedged).
- §7 (Method) — L194: "Because this under-counts paraphrase, **absolute recall is a lower bound**; the signal is the explicitly lexical *paired* delta..."
- §9.1 (Analysis → suppression) — L468: "the shared lexical matcher makes both rates **lower bounds** on paraphrased recovery" (this one is about the *raw/grounded recovery rates* in §9.1, a narrower and arguably defensible use — flag as borderline, not a flat contradiction, since it is scoped to "recovery rate given this matcher", not "recall generally").
- §19 (Limits) — L946: "the sweep's lexical title matcher under-counts paraphrase, making **absolute recall a lower bound**."

Net: L194 and L946 flatly contradict the careful, hedged claim at L91 (a matcher that both over- and under-counts cannot make its output a directional bound). L468 is milder (about recovery rates specifically, still arguably wrong for the same reason — over-counting via the 80%-of-words substring rule breaks the "lower bound" direction there too). **Writer action:** reword L194 and L946 (and reconsider L468) to something like "absolute recall is not a strict bound in either direction (A2); the paired delta is the reliable signal," matching L91's own already-correct language.

### 1d. §18 "refused a plausible enrichment a judge would have waved through" vs §10.1 judges' actual ratings

**VERIFIED — direct contradiction, quoting both sides.**

§18 (Datalake), L923:
> "...the promotion gate refused a plausible enrichment that a judge would have waved through (\S\ref{sec:casestudy});"

§10.1 (Case study → Judged page), L559–560:
> "A cross-family judge (Gemini~3.1~Pro, ...) scored each before/after pair. ... **Every arm degrades judged page quality (means $-1.04$ to $-0.44$)**; the two arms adjacent to a judge's family are flagged in the released analysis."

And L566–567: "The best variant remained negative under both judges ($-0.40$ dev, $-0.90$ held-out)."

These say the opposite of L923: every judged arm, under two independent judges and two rubrics, rated the enrichment **worse**, not better — i.e. a judge would *not* have waved it through; the judge agreed with the gate. L923's claim is unsupported by, and contradicted by, the paper's own §10.1 data. This is the single most important sentence to fix or delete in Datalake (D-property 1, L927, restates the same false premise — see 1e below).

### 1e. §18 D1 — reject-on-non-exceedance conflicts with faithful-extraction account

**VERIFIED.** D1, L927:
> "Candidates enter through a gate that computes the copy ceiling before promotion, so an extraction whose recall does not exceed what its own source already exposes is rejected as restatement (\S\ref{sec:casestudy})."

This is the rule the external review calls out directly (review body, "Second, D1 proposes rejecting an extraction..."): for faithful extraction, recovering what the source already says *is* the objective, so a rule that requires *positive* gain over copy to avoid rejection penalises correct, faithful extraction and would reward invented additions. This sits in tension with the paper's own framing elsewhere that recall of exposed gold ("delivery fidelity") is the desired behaviour (e.g. L117: "for curated-corpus grounding, restatement is the target behaviour"). D1 as written contradicts L117's own stated target. Needs rewording to something like "gate on answer-completeness of the source (ceiling high) rather than on gain over copy," which is closer to what §9's "Consequence 2" (L478–479) actually proposes ("A block that drives the ceiling towards 1 is answer-complete and eligible for promotion").

### 1f. §18 D2 "thirty times worse" (p. 24 in review's pagination)

**VERIFIED**, and the review's objection is correct. D2, L928:
> "Grounding has a cost: on the items the corpus does not expose, recovery falls from $0.121$ bare to $0.004$ grounded (\S\ref{sec:suppression}), **so a delivery-first path makes the model roughly thirty times worse at anything outside the corpus**, and the lake must route out-of-corpus questions away from the scaffold rather than through it."

The underlying 30× figure is itself measured carefully and correctly hedged in §9.1 (L468, "a roughly $\mathbf{30\times}$ reduction," with explicit caveats: "the effect is a property of this confidence-gated injection policy rather than of grounding in the abstract"). But D2's restatement drops every caveat and generalises "items the scaffold does not expose, under this prompt/retrieval/matcher" to "anything outside the corpus" — exactly the overreach the review flags. This is a rewrite, not a deletion: keep the 30× number, restore the scope ("on gold targets from this corpus that the retrieved scaffold happened not to expose, under this policy") and drop "anything outside the corpus."

### 1g. §11.1/§19 conflating macro ceiling 0.964 with exposed-instance fraction 1060/1136≈0.933

**PARTLY.** The paper is in fact unusually careful about this distinction in two places:
- L232 (§8.2/tenmodel): "It is **micro-averaged** over exposed items, whereas recall averages per-question ratios with varying $c_i$; accordingly, the rankings differ once..."
- Table 2 caption, L264–265: "Counts are pooled over gold items (micro-averaged), whereas the headline recall and ceiling average per-question ratios (**macro-averaged**), so these counts do not reconstruct those numbers directly."

So the *general* caveat exists. However, at **§19 (Limits), L952**, the paper restates the exposed fraction without repeating the caveat, in a way a reader could take as treating 0.964 and the item-pooled fraction as the same number:

> "Most in-domain targets ($96.4\%$; 76 per model are not) are verbatim in the scaffold, and beyond-exposure recovery is negligible ($n_{01}=3$ of 11{,}360)."

Reviewer's arithmetic: pooled exposed items $= n_{11}+n_{10} = 9{,}865+735 = 10{,}600$ of $N=11{,}360$ → $0.933$ (equivalently $1{,}060/1{,}136$ per-model average). That is **not** $96.4\%$; it is $93.3\%$. L952's "$96.4\%$ ... are verbatim in the scaffold" restates the macro ceiling as if it were this item-count fraction, without the L232/L264 caveat present at that specific sentence. **Writer action:** either add "(macro-averaged per question; the item-pooled exposed fraction is $0.933$, see \S\ref{sec:tenmodel})" at L952, or drop the "96.4%... are verbatim" framing there in favour of the already-correct pooled-count language used at L232. Also check §11.1 (gemma, L632–651): no occurrence of the 0.964-vs-0.933 conflation there — the section only restates the 0.964 ceiling for the Gemma sweep (L645, L669) without a pooled-item claim, so **NOT FOUND** in §11.1 specifically; the review's "§11.1" citation for this item does not have a matching passage — the actual site is §19 (L952) and, more weakly, the definitional passages at L232/264 (which are the *correct* framing, not the error).

### 1h. Stale cross-reference: multi-hop design "in the wrong section" (review: §§3.3/11 vs §19)

**VERIFIED**, and precisely locatable. Two forward-references to the multi-hop/two-edge composition design point at `\S\ref{sec:limits}` (§19, Limitations) instead of `\S\ref{sec:composition}` (§11, where the design actually lives, starting L577):

- L119 (§3.3, gain-over-copy discussion): "...and only the **multi-hop design of \S\ref{sec:limits}** could separate them." — should be `\S\ref{sec:composition}`.
- L580 (§11, opening sentence of the Composition section itself!): "The **multi-hop design sketched in \S\ref{sec:limits}** does." — self-referentially wrong: the section is *introducing* the design it claims is merely "sketched" elsewhere, and points at Limitations instead of itself or nowhere.

By contrast, §19 (Limits) L952 correctly points the other way: "The multi-hop design separating composition from exposure and guessing is reported in \S\ref{sec:composition}." — i.e. Limits already knows the design lives in §11; L119 and L580 are the two stale/wrong pointers to fix (change both to `\S\ref{sec:composition}`, or at L580 simply remove the cross-reference since the section is defining the design in place).

---

## 2. Reasoning/absence/regime-detector claims — every sentence, with classification

Grep terms: reasoning, not occurring, absent, serving regime, synthesis regime, detector, shows, demonstrably, rules out.

| # | Line | Section | Exact sentence (quoted) | Classification |
|---|------|---------|--------------------------|-----------------|
| 1 | 47 | Abstract | "...reported grounding uplift mixes faithful *delivery* of facts already shown to the model with *reasoning* over injected structure, because gold answers derive from the same corpus." | (a) defensible — states the conflation problem the paper exists to fix, doesn't claim to resolve which is present. |
| 2 | 51 | §1 Intro | "...headline ``grounding uplift'' conflates faithful *delivery* of exposed facts with *reasoning* over injected structure." | (a) defensible, same framing. |
| 3 | 119 | §3.3 Gain over copy | "$g>0$ would evidence *reasoning over structure*... We observe essentially none in-domain ($n_{01}=3$ of 11,360...). A positive $g$ alone would not prove reasoning: parametric leakage, guessing or matcher asymmetry could also produce it, and only the multi-hop design ... could separate them." | (a) defensible — explicitly says a positive g "would not prove reasoning," i.e. the instrument cannot establish it either way; this is the model passage the rest of the paper should be brought into line with. |
| 4 | 121 | §3.3 | "A near-ceiling result shows the measured recall is achievable by delivery alone: the metric **cannot credit reasoning beyond exposure**, and no reasoning is needed to explain the score. It does not measure how the model selected the correct names internally, so it **constrains what the evaluation can attribute, not what the model did**." | (a) defensible — this is the review's own preferred formulation ("does not establish reasoning beyond answer-name exposure"), stated almost verbatim already in the paper. Keep as the anchor sentence; align others to it. |
| 5 | 151 | §5 Runnable, "Reading the number" | "A ceiling near 1 ... identifies a *serving regime*... A ceiling near 0.5 or below is *consistent with* a synthesis regime, in which answers must be composed... but a low ceiling can equally indicate retrieval failure, vocabulary mismatch..., incomplete gold or matcher failure, so **it flags that the serving regime has been left, not that the supplied evidence suffices for synthesis**." | (a) defensible on close reading (hedged with "consistent with", disclaims proving synthesis) but the vocabulary "regime" + "detector"-adjacent framing is exactly what §9 later over-states without the hedge — reword for consistency even though this instance is technically guarded. |
| 6 | 223 | Fig. 1 caption (fig:anchor) | "The $+0.57$ raw$\to$scaffold uplift is almost entirely answer exposure, **not reasoning over structure**." | (a) defensible as written (a comparative claim about what the uplift *is*, backed by the ceiling comparison in the same figure), but terse enough to be quoted out of context — low priority reword. |
| 7 | 232 | §8.2 tenmodel | "...the deficit chiefly measures imperfect copying, including lexical-match under-counting of paraphrase, **rather than reasoning**." | (a) defensible — "chiefly measures X rather than Y" is a scoped comparative claim tied to the $n_{01}\approx0$ evidence just given. |
| 8 | 465 | §9 Analysis, opening | "Both studies indicate that graph-grounding uplift over a curated corpus is **almost entirely *exposure***: faithful delivery of curated facts shown to the model, **rather than reasoning over injected structure**." | (a) defensible, "almost entirely" + explicit paraphrase scope condition in same paragraph. |
| 9 | 476 | §9, "Consequence 1: the copy ceiling is a regime detector. Compute it before you architect." (heading) | Heading itself calls the ceiling a "**regime detector**." Body: "A high ceiling identifies a *serving regime*... A low ceiling identifies a *synthesis regime*... In a high-ceiling setting, **bare-model uplift is delivery masquerading as reasoning**." | (b) overreach to reword — the heading's flat "detector" claim is the exact phrase the review names as overreach ("§9 calls the ceiling a detector of serving versus synthesis regimes"). The body is more hedged ("heuristic," "does not prove... understood") but the heading isn't. Reword heading to something like "the ceiling as a prioritisation heuristic" and soften "masquerading" (a strong, unhedged claim) to "may be indistinguishable from" or similar. |
| 10 | 532 | §10 CaseStudy | "Generation time also roughly halves..., consistent with the scaffold displacing open-ended **reasoning**." | (a) defensible — "consistent with," explicitly hedged, about latency not the recall instrument. |
| 11 | 598 | §11 Composition | "though adding $A$'s block also changes relevance cues and retrieval selection, so $\Delta$ is a controlled contrast, **not a pure readout of internal reasoning**." | (a) defensible — explicitly disclaiming the strong reading. |
| 12 | 620 | §11 | "...guessing from the answer-bearing block alone is substantial (0.31–0.89 recall) and would masquerade as **multi-hop reasoning** without single-premise controls; uncontrolled two-hop benchmarks therefore overstate composition." | (a) defensible — about *other* benchmarks' methodology, not a claim about this paper's own instrument. |
| 13 | 628 | §11, Lineage and scope | multiple uses of "reasoning" citing MuSiQue/DiRe/distractibility literature | (a) defensible, literature framing. |
| 14 | 823–838 | §15 Practitioner | "reasoning model," "reasoning trace," "reasoning-budget exhaustion," "during reasoning," "reasoning backend" | not a claim about the instrument at all — these are about LLM reasoning-token budgets (an engineering/measurement-hazard topic), unrelated to the review's target. No action needed. |
| 15 | 923 | §18 Datalake, "Through-line" | "...a lake that pays for synthesis over a curated corpus is paying for **reasoning the instrument shows is not occurring**;" | (c) overreach to delete/rewrite — this is the review's D7-adjacent flag ("§18 says the instrument shows reasoning is not occurring") almost verbatim. The instrument (per L119/L121, the paper's own careful passage) explicitly cannot show this. Rewrite to "...paying for synthesis the instrument gives no evidence of" or similar, dropping "reasoning ... is not occurring." |
| 16 | 928 | §18, D2 | "...the alternative is paying for reasoning that **the ceiling shows is not happening** (\S\ref{sec:tenmodel}, \S\ref{sec:controls})." | (c) overreach to delete/rewrite — same flat "shows...not happening" claim the careful §3.3 passage explicitly disclaims. Same fix as #15. |
| 17 | 933 | §18, D7 "Stated limits" | "The system delivers vetted knowledge faithfully and attributably at low cost. Discovery is outside its remit... **A lake that promises reasoning over private data is promising the thing the ceiling shows is absent.**" | (c) overreach to delete/rewrite — this is the review's explicit example ("D7 makes a broader claim about promises of reasoning over private data"). "The ceiling shows is absent" is the flat claim to remove; replace with "the thing this instrument does not measure" or similar, and note the D1/D2/D7 overreach cluster is symptomatic — all three are in §18 and should be edited together. |
| 18 | 952 | §19 Limits | "**The delivery studies cannot license reasoning claims.** Most in-domain targets ($96.4\%$...) are verbatim in the scaffold, and beyond-exposure recovery is negligible..." | (a) defensible — this bullet heading and body are exactly the correct, narrow claim ("cannot license reasoning claims" — i.e. absence of evidence, not evidence of absence). Good model sentence; the 96.4%/93.3% conflation issue (see 1g) is a separate, narrower problem within the same bullet. |
| 19 | 959 | §20 Conclusion | "...distinguish a serving regime, where verbatim retrieval wins, from a synthesis regime, where heavier machinery must be measured above the ceiling rather than the bare model." | (a) defensible as an operational recommendation ("must be measured above"), not a claim that reasoning is absent. |

**Summary for this section:** the overreach cluster is concentrated almost entirely in **§18 Datalake** (L923, L928, L933) plus the **§9 heading** at L476 ("regime detector"). Everywhere else (§3.3, §8.2, §9 body, §11, §20) the paper already states the hedged, defensible version the review asks for — often in the review's own words. The fix is narrow: soften/delete four sentences in §18 and the §9 heading; leave the rest.

---

## 3. "faithful"/"faithfully"/"vetted"/"attributabl-" — every occurrence, for scoping to "recall of exposed gold names"

**faithful / faithfully / faithfulness:**
- L47 (abstract): "faithful *delivery*"
- L51 (§1): "faithful *delivery* of exposed facts"
- L55 (§1): "High-fidelity delivery of an authoritative, human-vetted source is the product: answers inherit its trustworthiness, are attributable to a named generation..."
- L67 (§2, paragraph title): "Input-only baselines and **faithfulness** metrics."
- L68 (§2): "**Faithfulness** can be measured separately from correctness~\cite{...}" — citing others' faithfulness metrics, not claiming to measure it.
- L133 (§3.4): "...serving as the direct analogue of a shortcut-robustness check for a **faithfulness** metric~\cite{gao2023alce}." — again citing ALCE's concept, not claiming this paper measures faithfulness.
- L151 (§5): "the target is **faithful** delivery"
- L325 (Fig. 2 caption): "Models nearest zero (top) restate the **vetted** facts most **faithfully**."
- L465 (§9): "faithful delivery of curated facts"
- L476 (§9): "The target is **faithful** delivery, measured by gain over copy."
- L912 (Table 7, bar axis 4): `precision metric to close "faithful" → "attributable"` — the paper's *own* table already flags "faithful" as the pre-precision-metric term needing to become "attributable" once axis 4 closes; this is the review's exact ask, already half-done.
- L933 (§18 D7): "The system **delivers vetted knowledge faithfully and attributably** at low cost."
- L940 (§19 Limits): "``faithful delivery'' here means **high recall of exposed gold**, not verified absence of fabrication." — this is the scoping sentence the review wants generalised to every other use.
- L959 (§20 Conclusion): "...delivers vetted knowledge **faithfully and attributably** at low cost..."

**vetted:**
- L55, L325, L933, L959 (all listed above, "vetted" always paired with "faithfully"/"faithful delivery").

**attributabl- (attributable/attributably/attribution):**
- L55: "are attributable to a named generation"
- L209 (§8.1): "attribution precision remain unmeasured (\S\ref{sec:bar})"
- L770 (§13 Setting): "jointly demand **attributable** recall where the corpus is authoritative"
- L912 (Table 7): "``faithful'' → ``**attributable**''"
- L933 (§18): "faithfully and **attributably**"
- L940 (§19): "Precision and attribution are consequently unmeasured"
- L959 (§20): "faithfully and **attributably**"

**Writer action:** L940 already contains the correct scoping sentence ("`faithful delivery` here means high recall of exposed gold, not verified absence of fabrication"). Every other unscoped use of faithful/vetted/attributable — L47, L51, L55, L151, L325, L465, L476, L770, L933, L959 — should either (i) get a first-use forward pointer to L940's scoping sentence, or (ii) be replaced with "high recall of exposed gold" / "recall-scoped delivery" directly. §2's two uses (L67–68, L133) are citations of other papers' faithfulness metrics and do not need rescoping — they're not claims about this paper's instrument.

---

## 4. "uniform budget" / "uniform-budget" — every use (reviewer: should be "common retry policy")

- L47 (abstract): "A **uniform-budget** control rerun with a length-matched fluent-noise arm shows..."
- L382 (§8.5 heading): "Negative controls: attributing the gain (**uniform-budget** rerun)" `\label{sec:controls}`
- L395 (§8.5 table caption prose, tab:controls caption context): "*new* is the **uniform-budget** intention-to-treat rerun (4096-token budget with 8192-token retries, $0\%$ attrition)."
- L473 (§9, "Live controls attribute less than first appeared"): "A **uniform-budget** rerun with a length-matched fluent-noise floor and a single judge revises our earlier reading..."
- L944 (§19 Limits, bullet heading): "**Control rerun**: what it settles and its own caveats. The **uniform-budget** rerun (\S\ref{sec:controls}) removed the $36$–$50\%$ attrition..."

Contrast with the paper's *own* correct description of the actual policy, already present at L383 and L206:
- L383 (§8.5 body): "under a **common retry policy**: 4096 tokens, with the few stragglers retried at 8192 (**per-question final budgets therefore vary within arms**)."
- L206 (§8 Results, reporting deviations): "The rerun ... under a **common retry policy** (4096 tokens, stragglers retried at 8192)..."

So the paper already has, and uses, the accurate term ("common retry policy") in the body prose (L206, L383) while using the inaccurate "uniform-budget" in the heading (L382), abstract (L47), a caption gloss (L395), an Analysis cross-reference (L473) and a Limits bullet heading (L944) — five sites to rename, none requiring new analysis since the accurate description already exists verbatim elsewhere in the same document. Recommend: rename the section label/heading at L382 to something like "Negative controls: attributing the gain (common-retry-policy rerun)" and propagate the same phrase to L47, L395, L473, L944. Note `\label{sec:controls}` itself need not change (it's an anchor, not visible text), so this is a pure prose fix with no dangling-reference risk.

---

## 5. "verified irrelevant" / "verified-irrelevant" / "correctly specified floor"

- L389 (§8.5, "Placebo-verification lesson stands as method"): "...an unverified placebo would have carried donor vocabulary and produced a misleadingly favourable baseline. **We no longer read the verified-irrelevant arm as evidence of content-specific transfer, only as a correctly specified floor.**"
- L473 (§9): "...but we now read the **verified-irrelevant arm as a correctly specified floor**, not as proof of content transfer."
- L959 (§20 Conclusion): "The reusable method is *the ceiling plus a **verified-irrelevant** placebo*..."

Reviewer's objection (review body, design issue #1): "Seed disjointness does not establish answer disjointness... You recognise this in §3.4, but later describe the placebo as **a correctly specified floor**." The relevant §3.4 acknowledgement is at L131 (Ceiling → Negative controls definition, `irrelevant` arm): "Seed-IRI disjointness is a construction check: it prevents donor and target sharing seed classes, **but does not by itself guarantee the donor block exposes none of the target's gold, ancestors or synonyms**." This is exactly the caveat the review says is later dropped. **VERIFIED**: L131 states the caveat; L389, L473 and L959 all then use "verified-irrelevant" / "correctly specified floor" as an achieved property without re-stating that caveat, and without reporting the measured target-exposure/relation-overlap for the placebo arm that would actually verify it (which the review separately asks for). **Writer action:** either (a) rename "verified-irrelevant" throughout (L389, L473, L959) to "seed-disjoint" or "construction-checked irrelevant" to match what was actually verified (seed/IRI disjointness, not semantic/answer disjointness), or (b) add the missing measurement (exposed-target overlap per placebo arm) and only then keep "verified."

---

## 6. Figures: inventory, historical-control figure, and routing-section dependencies

Figure source files in `figures/`:
- `fig-ceiling.tex` — TikZ diagram of the copy-ceiling computation (inputs → no-op extractor → ceiling; inputs → model → recall); used at L123, Fig. `fig:ceiling`, in §3.3.
- `fig-forest.tex` — forest plot of Table 3 (`tab:live`) paired judged-quality deltas by set (arcane/thin/general/pooled); used at L334, Fig. `fig:forest`, in §8.3 (Production-node paired study).
- `fig-controls.tex` — bar/point chart of the **negative-control contrasts against raw**, with y-axis labels `irrelevant (n=28)`, `shuffled (n=36)`, `masked (n=33)`, `true (n=32)`. **These n's match the paper's own reported *old* (pre-rerun) survivor counts** (§8 Results, L206: attrition-biased 1536-token-budget arms — irrelevant 28/56 survived, i.e. 50.0% attrition; true 32/56 survived (42.9% attrition); masked 33/56 (41.1%); shuffled 36/56 (35.7%)). The figure is used at L385, immediately inside §8.5 (`sec:controls`), right where the text has already moved on to describing the **new**, ITT, 0%-attrition rerun in Table 5 (`tab:controls`, L391–419, with both *old* and *new* columns). **This is the "historical control figure that competes with its replacement"** the review names: Fig. `fig:controls` still plots only the superseded pre-rerun numbers, while Table `tab:controls` right below it (the actual replacement/authoritative source) reports both old and new side by side. Writer action: either regenerate `fig-controls.tex` to plot the *new* ITT contrasts (matching Table 5's "New (ITT)" column), or drop the figure and let Table 5 carry the result, since the figure currently duplicates only the outdated half of the table.
- `fig-composition.tex` — scatter of both-premises recall vs $b$-only recall (composer/anti-composer plot); used at L630, Fig. `fig:composition`, in §11.
- `fig-casestudy.tex` — scatter of ground-rate vs assertion count across the ten serving arms; used at L542, Fig. `fig:casestudy`, in §10.
- `fig-ecosystem.tex` — pipeline/ecosystem diagram; used at L734, Fig. `fig:ecosystem`, in §13.1 (Ecosystem in brief).
- `fig-lifecycle.pdf` (+ `fig-lifecycle-source.html`, a pre-rendered PDF, not a `.tex` source in this tree) — used at L883, Fig. `fig:lifecycle`, in §16 (Write-Path Lifecycle).

**Tables/figures depending on §Routing (sec:routing, L677–725):** the section is self-contained — one table, `tab:routing` (L695–712, "Typed skill routing, $n=86$..."), no dedicated figure. Nothing outside §12 defines a table or figure whose *content* is drawn from the routing data; the only outside dependencies are the six **prose** cross-references into §12 listed in Q8 below (L53, L76, L112, L923, L929, L931, L953, L954, L959 all `\ref{sec:routing}` forward- or back-references — see the full site list in Q8). So moving/shortening §12 (per the review's "separate the routing extension" recommendation) requires only: (i) keep or relocate `tab:routing` and its own in-section prose intact, (ii) fix the nine `\ref{sec:routing}` cross-references (Q8) to point at wherever the section (or a summary of it) ends up, and (iii) no figure needs re-homing.

---

## 7. Section-by-section word/line counts (via `\section` line spans)

| Section | Lines | Line count | Word count |
|---|---|---:|---:|
| 1 Intro | 50–59 | 10 | 578 |
| 2 Related Work | 60–74 | 15 | 561 |
| 3 The Copy Ceiling | 75–140 | 66 | 1,701 |
| 4 Semantic Re-Score | 141–143 | 3 | 253 |
| 5 Making the Copy Ceiling Runnable | 144–152 | 9 | 339 |
| 6 Vocabulary Mismatch | 153–190 | 38 | 635 |
| 7 Method | 191–204 | 14 | 735 |
| **8 Results** | **205–463** | **259** | **3,389** |
| 9 Analysis: Exposure, not Reasoning | 464–480 | 17 | 984 |
| 10 Case Study (Podcast Extraction) | 481–576 | 96 | 1,109 |
| 11 Two-Edge Composition | 577–676 | 100 | 1,054 |
| 12 A Second Domain: Typed Skill Routing | 677–725 | 49 | 1,747 |
| 13 Curated-Corpus Setting | 726–793 | 68 | 886 |
| 14 Ontology Scaffold (System) | 794–821 | 28 | 520 |
| 15 Reasoning-Budget Exhaustion | 822–848 | 27 | 310 |
| 16 Write-Path Lifecycle | 849–892 | 44 | 486 |
| 17 Roadmap and Open Axes | 893–919 | 27 | 328 |
| 18 Ideal State: Private Data Lake | 920–937 | 18 | 1,230 |
| 19 Limitations and Threats to Validity | 938–957 | 20 | 959 |
| 20 Conclusion | 958–960 | 3 | 358 |
| Appendices (rubrics + worked example) | 961–1067 | 107 | ~950 (unmeasured precisely; two `verbatim` blocks) |

**Where the length actually goes, for the writer:** §8 Results (3,389 words) and §3 Ceiling (1,701 words) dominate, as expected for a results-heavy paper — not obviously cuttable. But **§12 Routing (1,747 words) is longer than Introduction+Related Work combined (1,139 words)** and, per the review, is a methodologically separate instrument (a different formal ceiling construction) bundled into the main paper. **§18 Datalake (1,230 words)** is the section the review most wants shortened/removed ("I would remove most of §18"), and per §2 above concentrates nearly all of the "reasoning is absent" overreach. Together, §12 + §18 = 2,977 words (~13% of the ~23,150-word body) are the two highest-value cut/relocate targets the review identifies, and both are self-contained enough (§12 has its own table and no outbound figure dependency per Q6; §18 has zero inbound `\ref{sec:datalake}` citations per Q8) to move to a companion document with modest edits.

---

## 8. `\label`/`\ref` audit for sec:routing, sec:datalake, sec:bar, sec:roadmap, sec:lifecycle, sec:casestudy

### Labels (all six exist exactly once, as section/subsection anchors):
- `\label{sec:routing}` — L677 (`\section{A Second Domain: Typed Skill Routing}`)
- `\label{sec:datalake}` — L920 (`\section{Ideal State: A Private Corporate Data Lake}`)
- `\label{sec:bar}` — L896 (`\subsection{Multivariate bar}`, inside §17 Roadmap)
- `\label{sec:roadmap}` — L893 (`\section{Roadmap and Open Axes}`)
- `\label{sec:lifecycle}` — L849 (`\section{Write-Path Lifecycle}`)
- `\label{sec:casestudy}` — L481 (`\section{A Production Case Study: Podcast Assertion Extraction}`)

### `\ref{sec:routing}` — every site (9 total, excluding the label's own line):
L53, L76, L112, L923, L929, L931, L953, L954, L959. (Full sentences quoted in §1 above under the "Routing" cross-references, or in Q1c/Q2 context — all nine are prose pointers, none inside a table/figure caption.) **Dangling-reference risk if §12 is moved:** all nine must be updated (either to a new external label if §12 becomes a companion-paper section, or removed if the sentence is cut).

### `\ref{sec:datalake}` — every site:
**None found.** `grep -n '\\ref{sec:datalake}' main.tex` returns zero matches. §18 is never cross-referenced from elsewhere in the paper. **This means §18 can be deleted or relocated to a companion document with zero risk of a dangling reference elsewhere** — confirming the review's "I would remove most of §18" is a clean, low-risk cut from a cross-referencing standpoint (the content risk — losing the D1/D2/D7 overreach discussion — is what needs editorial judgement, not the mechanics).

### `\ref{sec:bar}` — every site:
L209 ("...fabrication and attribution precision remain unmeasured (\S\ref{sec:bar})"), L770 ("We reconcile these axes in \S\ref{sec:bar}."), L928 (§18 D2, "...the web-search baseline and attribution precision (\S\ref{sec:bar})"), L940 (§19, "The multivariate bar (\S\ref{sec:bar}) is accordingly a framework..."). 4 sites. Note L928 is inside §18 (Datalake) — if §18 is cut, this one reference needs to move with whatever replaces the D2 bullet, or be dropped.

### `\ref{sec:roadmap}` — every site:
**None found.** Zero cross-references, same as sec:datalake — §17 (which contains §17.1/sec:bar, itself referenced 4×) is not directly referenced as a whole from elsewhere.

### `\ref{sec:lifecycle}` — every site:
One site: L932 (§18 D6, "the lifecycle in \S\ref{sec:lifecycle} is the shape..."). Only reference is itself inside §18 — if §18 is cut/moved, this is the only dangling risk for sec:lifecycle, and it moves with the D6 bullet.

### `\ref{sec:casestudy}` — every site:
Two sites, both inside §18: L923 (Through-line paragraph, "...the promotion gate refused a plausible enrichment... (\S\ref{sec:casestudy})") and L927 (D1, "...is rejected as restatement (\S\ref{sec:casestudy})"). Both of §18's references to §10 (Case Study) are the two passages already flagged as needing rewrite in Q1d/Q1e above — so fixing those two sentences and auditing this cross-reference are the same edit.

**Overall dangling-reference risk assessment:** sec:datalake and sec:roadmap have **zero** inbound references — both sections can be cut or relocated with no cross-reference cleanup elsewhere in the paper. sec:bar (4 refs) and sec:lifecycle/sec:casestudy (1–2 refs each, all *from inside* sec:datalake) only need cleanup if §18 is cut — in which case those refs disappear along with the bullets that contain them, except L209, L770 and L940 (three of sec:bar's four refs), which live *outside* §18 and must be preserved/repointed regardless of what happens to §18. sec:routing (9 refs, scattered across §1, §3.2, §18, §19, §20) is the one section whose removal/relocation genuinely requires a multi-site prose edit.

---

## Summary: the ten findings that most change what the writer must do

1. **§18 (Datalake, L920–937) is the single highest-value edit target.** It contains the abstract's clearest overreach (L923 "reasoning ... not occurring", L928 "reasoning ... not happening", L933 "reasoning ... the ceiling shows is absent"), the D1 rule that contradicts the paper's own faithful-extraction framing (L927), the D2 "thirty times worse" over-generalisation (L928), and the flat contradiction of §10.1's judged-page results (L923 vs L559–560). It has **zero inbound cross-references** (sec:datalake), so it can be cut to a companion note almost mechanically; only sec:bar (3 external refs), sec:lifecycle (1) and sec:casestudy (2) refs inside it need re-homing.
2. **Elsewhere, the paper already contains the hedged, defensible version of every "reasoning" claim** — L119, L121 (§3.3) state almost verbatim what the review asks for ("does not establish reasoning beyond exposure"). The fix is to delete/reword the ~4 overreaching sentences in §18 plus the §9 heading (L476, "regime detector"), not to rewrite the whole paper's epistemics.
3. **Three fabrication-free arithmetic/typesetting bugs need a one-line fix each:** the literal `emphexposure`/`emphcopy`/`emphgain` (L47, stray `\\` before `\emph`), the "8.1 points" (should be 6.9, L691), and two stale `\S\ref{sec:limits}` that should say `\S\ref{sec:composition}` (L119, L580).
4. **"Uniform-budget" (5 sites: L47, L382, L395, L473, L944) should become "common retry policy"** — the paper *already uses the accurate phrase* at L206 and L383, so this is a find-and-replace, not new analysis.
5. **"Verified-irrelevant"/"correctly specified floor" (L389, L473, L959) overclaim what L131's own caveat admits** — seed/IRI disjointness was checked, answer/relation disjointness was not measured. Rename to "seed-disjoint" or add the missing exposure-overlap measurement.
6. **The macro-ceiling (0.964) vs item-pooled exposed-fraction (0.933, i.e. 1060/1136) conflation is narrower than the review implies**: the paper already states the macro/micro distinction carefully at L232 and in the Table 2 caption (L264–265); the one under-caveated restatement is L952 in §19.
7. **`fig-controls.tex` is the stale figure competing with its replacement**: its axis labels (n=28/36/33/32) are the *old*, pre-rerun, attrition-biased survivor counts, plotted right beside (L385) the up-to-date Table 5 (`tab:controls`) that reports both old and new. Regenerate the figure from the ITT column or drop it.
8. **§12 Routing (1,747 words) and §18 Datalake (1,230 words) are the two largest, most separable sections** (§12: one self-contained table, no figure dependency, 9 scattered `\ref`s to fix; §18: zero inbound refs) — together the natural candidates for the review's "separate the routing extension" and "remove most of §18" recommendations, and removing both would cut ~13% of the body.
9. **D1 (L927) is not merely loosely worded — it contradicts L117's own stated target ("for curated-corpus grounding, restatement is the target behaviour")** and needs substantive rewriting (gate on ceiling/answer-completeness, not on gain-over-copy sign), not just softer language.
10. **"Faithful"/"vetted"/"attributable" already have a correct scoping sentence written (L940: "`faithful delivery` here means high recall of exposed gold, not verified absence of fabrication")** — the fix is to propagate that one sentence's scope to the ten other unscoped uses (L47, L51, L55, L151, L325, L465, L476, L770, L933, L959), not to invent new scoping language.
