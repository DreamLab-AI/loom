# routing-eval: snapshot of the system-one-eval rig

Source snapshot of `agentbox/crates/system-one/system-one-eval/src` at agentbox commit
`5c273c704` (2026-09-21), taken so the routing-seam study written up in
`docs/research/companion-routing/` can be reproduced from this repository alone. The
crate is not built here (it depends on `system-one-core` and `system-one-client` in the
agentbox workspace); the file of interest is `src/copy.rs`, the copy-ceiling procedure for
a choice task with its regression tests:

- breakpoint-enumerating decline-threshold sweep (never a fixed grid);
- exclusion-clause stripping before indexing (`strip_exclusion_clause`);
- flagging of rankers that score every option identically;
- exact two-sided McNemar on the paired judge/ranker outcomes (`gain`).

`UPSTREAM-README.md` is the crate's own README. Data it consumes and the reports it
wrote live in `uplift-results/routing/`. The live rig stays in agentbox because it is
wired to that repository's skills tree; if the two diverge, the agentbox crate is the
implementation and this snapshot is the record of what the paper's numbers came from.
