# External review 3 of paper v9 (24 pp), received 2026-09-21

Verbatim, same reviewer. Verdict: strongest version; credible candidate; remaining work is to
validate and accurately describe the semantic audit.

Four requested items:
1. Correct the false-credit interpretation (the 0/60 are n00, uncredited by construction; the
   three n01 credits all fail relational adjudication, so precision on that census is zero) and
   make the quote-gate decisions symmetric and auditable (one policy for all 29 failed quotes:
   obtain a valid span; else mark unresolved; report sensitivity bounds).
2. Audit the 92 raw-arm lexical hits behind the suppression result with the same relational judge.
3. Complete the prepared human check; specify the audit's uncertainty calculation (weights,
   strata, repeated questions/targets, uncertainty in both rates); describe the 0.931 to 0.905
   correction as an estimate under this adjudicator and protocol.
4. Pin the release (exact commit, manifest path, direct link in the manuscript); remove the
   remaining contradictory terminology.

Smaller items: page 1 says relational correctness is unmeasured (now measured on a sample under
model adjudication); "answer-completeness" survives in §13 where "name-completeness" is meant;
the run-together-title mechanism is overstated (58 credited examples exist); "informative only
when gold and context derive from the same source" is too restrictive; Table 4 omits n11 and
n00; attrition also changes which questions the estimate describes, not only power; §6
regenerated contexts are not shown byte-identical, so call it a re-score on reconstructed
contexts and state the equivalence check performed.
