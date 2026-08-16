# Research notes — KG augmentation, retrieval, and the Loom's position

*Verified-citation notes for the paper's related-work / architecture sections. NOT paper prose —
a grounded reference. Every arXiv ID below was surfaced via perplexity + web-researcher and should
be re-checked against the primary record before it goes into a bibliography.*

## 1. What "KG grounding improves results" actually means in the literature — the methods

The claim is almost never "dump ontology prose into context." The winning methods are **precise,
question-scoped, structured retrieval**:

- **Entity-linked triple retrieval.** KAPING (Baek et al. 2023, arXiv:2306.04136): link the
  question's entities to KG nodes, retrieve top-k *answer-relevant triples*, verbalise, prepend.
  The answer triple is usually *in* the retrieved set.
- **KG + Personalized PageRank (hybrid).** HippoRAG (Gutiérrez et al., NeurIPS 2024,
  arXiv:2405.14831): OpenIE triples → schemaless KG; a dense encoder embeds entities and adds
  synonymy edges (cos > 0.8); query entities are dense-linked to seed nodes, then **PPR from the
  seeds** does single-step multi-hop retrieval, scores aggregated back to passages. Reports **≈+20
  EM/F1 on 2WikiMultiHopQA** over dense/iterative RAG at 6–13× lower cost. HippoRAG 2
  (arXiv:2502.14802) adds phrase+passage dual nodes.
- **Subgraph retrieval + reranking.** G-Retriever (He et al. 2024, arXiv:2402.07630): embed *all
  node/edge text* (Faiss), top-k, then a **Prize-Collecting Steiner Tree** builds a minimal
  connecting subgraph, GAT-encoded and soft-prompted into a frozen LLM. GNN-RAG / SubgraphRAG are
  cousins.
- **Community-summary GraphRAG.** Edge et al. 2024 (arXiv:2404.16130): LLM-extracted entity graph →
  Leiden communities → **LLM-written community summaries**, map-reduced for global "sensemaking"
  queries. LightRAG (arXiv:2410.05779) folds graph structure into dual-level retrieval.
- **Agentic / iterative traversal.** Think-on-Graph and Reasoning-on-Graphs (RoG): the model
  *follows relations step-by-step with pruning* to find answer paths — precision by construction.
  *(IDs to verify: ToG ~2307.07697; RoG ~2310.01061.)*
- **KG-extended / decomposition RAG.** KG-RAG (arXiv:2504.08893): KG-based question decomposition;
  improves multi-hop MetaQA but reports a **slight drop on single-hop** — the sharpest statement
  that the graph is a *specialised* capability, not a universal upgrade.

**Retrieval unit, across all of them:** compact triples / paths / subgraphs / community summaries —
*not* large prose blobs. Scoping is by entity-linking, dense similarity, or agentic traversal.
Evaluation is mostly KGQA (WebQSP, CWQ, MetaQA, MuSiQue, 2WikiMultiHopQA) where the answer *is* a
KG entity, so precise retrieval trivially contains it.

## 2. When the KG beats vector — and when it doesn't

- **KG wins:** multi-hop / relational / compositional reasoning; completeness / enumerate-all;
  global sensemaking; large interlinked corpora where dense recall collapses at scale
  (HippoRAG 2405.14831; G-Retriever 2402.07630; GraphRAG 2404.16130; gains grow with hop count).
- **Dense (or BM25) is sufficient — KG adds little:** single-hop factual lookup. KG-RAG
  (2504.08893) shows a single-hop *drop* from KG machinery. The graph is overhead here.

**Direct consequence for our study:** our arcane questions are overwhelmingly **single-hop**
("what is AluVM", "what is a single-use seal"). That is exactly the regime the literature says a
KG does *not* beat dense retrieval. Our null is therefore *consistent with the field*, not
evidence against grounding. The KG's demonstrated value is on **multi-hop / relational /
completeness** questions we have not yet posed.

## 3. Over-retrieval hurts — grounds our −0.40 traversal result

- **Lost in the Middle** (Liu et al., TACL 2023, arXiv:2307.03172): U-shaped positional use;
  content in the middle of a long context is under-used; a bigger window does not fix it.
- **Irrelevant inputs skew LLMs** (arXiv:2404.03302): **semantically *related* but answer-irrelevant**
  fragments mislead *more* than random noise, and more of them → worse. This is precisely our 24K
  traversal blob of Bitcoin-adjacent junk.
- **Yoran et al.** (arXiv:2310.01558): naive retrieval can *reduce* accuracy via irrelevant context.
- **Shi et al. GSM-IC** (arXiv:2302.00093): LLMs easily distracted by irrelevant context.
- Prediction-flips under task-irrelevant context (aggregate accuracy hides per-item instability).

Our naive 1-hop neighbourhood preload degraded quality **−0.40 [−0.58,−0.22]** — a documented
distraction effect, worst on the smallest model (claude-haiku −1.30). Argues for **precise,
selective** retrieval (PPR/PCST/agentic pruning), not bulk dumps.

## 4. The Loom's position — a *human-readable* GraphRAG

What we have built, in the field's vocabulary: a GraphRAG variant whose retrieval unit is the
**curated, human-readable research prose** (`dfull`) indexed by ontology IRI, over a corpus that is
**reasoned and consistency-checked** (Whelk EL++), with **confidence-gated injection**. Distinctive
vs the literature:
- GraphRAG injects *LLM-generated community summaries* (opaque, lossy); we inject *human-authored,
  verifiable* prose tied to a checked ontology.
- G-Retriever injects *GNN-encoded* subgraphs (not human-readable); ours is legible and attributable.
- Most KGQA systems retrieve *instance triples*; we serve *schema-level* definitions + typed
  relations + prose — comparatively under-studied.

Honest caveats we must state: (a) our current retrieval is lexical, not semantic — it mis-selects
concepts (the deferred RuVector layer is the fix); (b) our questions are single-hop, the regime
where the KG premium is smallest; (c) private-corpus recall is real but only measurable circularly
(no external referent). The methodological contribution: *verifiability and parametric-gap are
mutually exclusive*, and *KG value is conditional on retrieval precision and question structure*.

## 5. Empirical results map (for the results section)

| condition | retrieval | judged Δ vs bare (independent gold) |
|---|---|---|
| stub harness | 1.5K, single-hop, lexical | ~null (answer often absent) |
| naive traversal preload | 24K, 1-hop neighbourhood, noisy | **−0.40 [−0.58,−0.22]** (distraction; lit-predicted) |
| precise (exact concept) | clean, ~1 concept, lexical-precise | *[running]* — expect ≈ dense (single-hop regime) |
| general knowledge (non-destructive) | gated | −0.05 [−0.11, 0.00] (null; safe) |

Next experiment the literature points to: **multi-hop / relational / completeness questions** with
**precise semantic retrieval** (RuVector) — the regime where a KG is *for*.

## BibTeX to verify + merge (KG-vs-vector / over-retrieval additions)

```bibtex
@inproceedings{gutierrez2024hipporag, title={{HippoRAG}: Neurobiologically Inspired Long-Term Memory for Large Language Models}, author={Guti\'errez, Bernal Jim\'enez and others}, booktitle={NeurIPS}, year={2024}, note={arXiv:2405.14831}}
@article{he2024gretriever, title={{G-Retriever}: Retrieval-Augmented Generation for Textual Graph Understanding and QA}, author={He, Xiaoxin and others}, journal={arXiv preprint arXiv:2402.07630}, year={2024}}
@article{jimenez2025hipporag2, title={From {RAG} to Memory: Non-Parametric Continual Learning for LLMs}, author={Guti\'errez, Bernal Jim\'enez and others}, journal={arXiv preprint arXiv:2502.14802}, year={2025}}
@article{kgrag2025, title={Knowledge Graph-extended Retrieval Augmented Generation for Question Answering}, author={{(verify authors)}}, journal={arXiv preprint arXiv:2504.08893}, year={2025}}
@article{wu2024irrelevantskew, title={How Easily do Irrelevant Inputs Skew the Responses of Large Language Models?}, author={{(verify authors)}}, journal={arXiv preprint arXiv:2404.03302}, year={2024}}
```
