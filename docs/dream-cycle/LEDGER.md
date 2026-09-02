| Date | Deep | Finding | Issue | PR | Evaluated? | Verdict | Effect | Witness | Prior-night fates |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 2026-08-15 | graph-engine | PEP 668 blocks pip install outside venv; pyoxigraph availability unconfirmed; venv fix applied | NONE | NONE | no | INCONCLUSIVE | build step fixed to use venv | e395acbc | first loom cycle |
| 2026-08-15 | graph-engine | Given a located triple-loading corpus, when | NONE | NONE | yes | INCONCLUSIVE |  | 30a0a167 |  |
| 2026-08-15 | graph-engine | INCONCLUSIVE — see report | NONE | NONE | yes | INCONCLUSIVE |  | 047e2fbc476e |  |
| 2026-08-15 | graph-engine | Given the venv-isolated environment at commit `cb44bfad` (after the slot-0 PEP 6 | NONE | NONE | yes | ACCEPT |  | c4c3736e8d05 |  |
| 2026-08-16 | scaffold-retrieval | Given the scaffold-index generator runs in a git checkout whose HEAD is the sess | NONE | NONE | yes | INCONCLUSIVE |  | 8375d203095c |  |
| 2026-08-16 | scaffold-retrieval | Given the scaffold-index generator at session HEAD `cb44bfadff796910dd150ddd895d | NONE | NONE | yes | INCONCLUSIVE |  | e2d30c07644a |  |
| 2026-08-17 | model-facade | Given the loom facade whose scaffold-index is generated in a git checkout at ses | NONE | NONE | yes | INCONCLUSIVE |  | 79fb4721fd83 |  |
| 2026-08-17 | model-facade | Given the loom model-facade at session commit `710fb5f4` operating in scaffold m | NONE | NONE | yes | INCONCLUSIVE |  | 377be864b76a |  |
| 2026-08-28 | scaffold-retrieval | Given the scaffold-retrieval workload (facade /health serving ScaffoldIndex + MirrorManifest state), when | NONE | NONE | yes | INCONCLUSIVE |  | 55fa5421 |  |
| 2026-08-29 | model-facade | Given the loom model-facade at session commit `8529aad5` reporting healthy (`/he | NONE | https://github.com/DreamLab-AI/loom/pull/1 | yes | ACCEPT |  | d981dcd3e502 |  |
| 2026-08-30 | confidence-injection | Given the loom facade at session commit `0be6e692` reports healthy at `/health`  | NONE | NONE | yes | ACCEPT |  | 1d6909a9fde5 |  |
| 2026-08-31 | graph-engine | Given the loom facade at session commit `0be6e692` serves the graph store at `/h | NONE | NONE | yes | ACCEPT |  | 711110b06b36 |  |
| 2026-09-01 | model-facade | Given the loom model-facade at session commit `4520948910a2ecfc78594e6ef0873bc77 | NONE | NONE | yes | ACCEPT |  | 87d538d77d6a | #1:MERGED |
| 2026-09-01 | scaffold-retrieval | operator promotion: facade /health served stale unpromoted ScaffoldIndex (8142/2026-08-11, verified_single_generation=false, .generation.json absent) while semantic MirrorManifest was verified (8146/2026-08-17); ran sanctioned app/mirror.sh against canonical HP data dir → atomic promote of upstream single generation 8146/2026-08-22 (.generation.json written, span 6.5s), restarted loom-facade-a → served generation now MirrorManifest verified_single_generation=true, index_classes 8146 | NONE | NONE | no | INCONCLUSIVE |  | operator | #1:MERGED |
| 2026-09-02 | confidence-injection | Given the loom facade at session commit `865e9ea` serving the promoted verified  | NONE | NONE | yes | REJECT |  | 557c05d64d32 |  |
