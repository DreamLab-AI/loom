# v4 Rewrite Report

- Source: `docs/research/paper-v2/main.tex`
- Rewriteable paragraphs (total): **48**
- Protected blocks: **52** (1 protected-with-note)
- Processed this run (limit=48): **48**
- Accepted: **47**  |  Fallback (kept original): **1**
- Diversity warnings (>0.9 token-identical): **0**

## Per-paragraph

| idx | section | status | attempts | len-ratio | diversity |
|----:|---------|--------|---------:|----------:|----------:|
| 0 | Abstract | accepted | 1 | 0.989 | 0.889 |
| 1 | Introduction | accepted | 1 | 0.937 | 0.477 |
| 2 | Introduction | accepted | 1 | 1.011 | 0.559 |
| 3 | Introduction | accepted | 1 | 1.103 | 0.664 |
| 4 | The Private-Corpus Setting | accepted | 1 | 1.095 | 0.715 |
| 5 | The ecosystem in brief | accepted | 1 | 1.017 | 0.826 |
| 6 | The ecosystem in brief | accepted | 2 | 1.086 | 0.38 |
| 7 | Related Work | accepted | 1 | 1.064 | 0.742 |
| 8 | Related Work | accepted | 1 | 1.124 | 0.736 |
| 9 | Related Work | accepted | 1 | 1.011 | 0.643 |
| 10 | Related Work | accepted | 2 | 0.997 | 0.467 |
| 11 | The Ontology Scaffold | accepted | 1 | 1.1 | 0.726 |
| 12 | The Ontology Scaffold | accepted | 1 | 1.182 | 0.519 |
| 13 | The Ontology Scaffold | accepted | 1 | 1.109 | 0.506 |
| 14 | The Copy Ceiling | accepted | 1 | 1.11 | 0.632 |
| 15 | Definition and assumptions | accepted | 2 | 1.129 | 0.428 |
| 16 | Gain over copy | accepted | 3 | 1.205 | 0.7 |
| 17 | Gain over copy | accepted | 1 | 1.023 | 0.587 |
| 18 | Negative controls: what would move the g | accepted | 1 | 1.089 | 0.205 |
| 19 | Negative controls: what would move the g | accepted | 1 | 1.028 | 0.582 |
| 20 | Method | accepted | 1 | 1.112 | 0.592 |
| 21 | Method | accepted | 2 | 1.057 | 0.717 |
| 22 | Method | accepted | 3 | 1.089 | 0.736 |
| 23 | Method | accepted | 1 | 1.086 | 0.787 |
| 24 | Method | accepted | 1 | 1.055 | 0.629 |
| 25 | Method | accepted | 1 | 1.052 | 0.665 |
| 26 | Results | accepted | 1 | 1.079 | 0.694 |
| 27 | In-domain: delivery fidelity | accepted | 1 | 1.053 | 0.499 |
| 28 | Ten models: gain over copy | accepted | 1 | 1.122 | 0.432 |
| 29 | Ten models: gain over copy | accepted | 1 | 1.076 | 0.779 |
| 30 | Ten models: gain over copy | fallback | 3 | 1.0 | 1.0 |
| 31 | The production-node paired study | accepted | 1 | 1.02 | 0.681 |
| 32 | The production-node paired study | accepted | 1 | 1.055 | 0.646 |
| 33 | The production-node paired study | accepted | 1 | 0.998 | 0.722 |
| 34 | Negative controls: attributing the gain | accepted | 1 | 1.115 | 0.747 |
| 35 | Negative controls: attributing the gain | accepted | 3 | 1.039 | 0.563 |
| 36 | Negative controls: attributing the gain | accepted | 1 | 1.024 | 0.669 |
| 37 | Negative controls: attributing the gain | accepted | 1 | 1.076 | 0.412 |
| 38 | Out-of-domain: the gate holds | accepted | 2 | 0.991 | 0.714 |
| 39 | Analysis: Exposure, not Reasoning | accepted | 1 | 1.044 | 0.493 |
| 40 | Analysis: Exposure, not Reasoning | accepted | 1 | 1.049 | 0.236 |
| 41 | Analysis: Exposure, not Reasoning | accepted | 1 | 1.003 | 0.6 |
| 42 | Analysis: Exposure, not Reasoning | accepted | 1 | 1.012 | 0.667 |
| 43 | Analysis: Exposure, not Reasoning | accepted | 1 | 0.995 | 0.311 |
| 44 | Analysis: Exposure, not Reasoning | accepted | 1 | 1.025 | 0.89 |
| 45 | The Multivariate Bar | accepted | 2 | 1.211 | 0.167 |
| 46 | The Multivariate Bar | accepted | 1 | 1.112 | 0.455 |
| 47 | Conclusion | accepted | 1 | 1.018 | 0.643 |

## Footnote-bearing paragraphs (footnote prose unchecked; eyeball these)

- idx 2 (Introduction): status accepted
- idx 4 (The Private-Corpus Setting): status accepted

## Protected-with-note blocks

- protected: inline environment/display-math in prose: `Fix a question $q_i$ with gold target set $G_i$ (the \{slug, title\} pairs the g...`

## Fallbacks (last violation)

- idx 30 (Ten models: gain over copy): VIOLATION: missing math span(s): $g$. Return ONLY the corrected LaTeX paragraph.
