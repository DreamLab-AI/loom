# Measured placebo exposure of the injected block, per arm

Gold targets per question = the titles of its **true** scaffold's seed classes plus the question's declared `topic`. Exposure is the fraction of those titles the injected block matches under `decompose_exposure.gold_hit` (the bench scorer, byte-identical). `true` is the ceiling each placebo arm is meant to fall short of; the token column shows the arms are length-matched.

| arm | n | mean exposure | median | min | max | zero-exposure rows | full-exposure rows | mean injected tokens |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| fluent_noise | 56 | 0.1676 | 0.2000 | 0.0000 | 0.6000 | 24 | 0 | 1074.9 |
| irrelevant | 56 | 0.2946 | 0.2500 | 0.0000 | 0.7500 | 13 | 0 | 1239.8 |
| masked | 56 | 0.1777 | 0.2000 | 0.0000 | 1.0000 | 26 | 1 | 1115.5 |
| shuffled | 56 | 0.8592 | 0.8000 | 0.5000 | 1.0000 | 0 | 23 | 1112.3 |
| true | 56 | 0.8592 | 0.8000 | 0.5000 | 1.0000 | 0 | 23 | 1113.0 |

Non-zero exposure in a placebo arm is expected and is the point of measuring it: the matcher is lexical, and generic class titles recur across an 8,146-class corpus, so a block about unrelated entities can still contain a gold token by coincidence.

