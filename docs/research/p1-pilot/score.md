| stratum | n | arm3 recall | arm4 recall | delta | naive 95% CI | clustered 95% CI | tokens a3 / a4 (median) | turns a4 (median) | wall s a3 / a4 (median) |
|---|---|---|---|---|---|---|---|---|---|
| lookup | 20 | 0.93 | 0.94 | +0.01 | [-0.12, +0.11] | [-0.14, +0.12] | 1130 / 4952 | 3 | 17 / 51 |
| cross | 10 | 1.00 | 1.00 | +0.00 | [+0.00, +0.00] | [+0.00, +0.00] | 1254 / 8958 | 4 | 27 / 89 |
| path | 10 | 0.50 | 0.90 | +0.40 | [+0.10, +0.65] | [+0.06, +0.67] | 3110 / 7797 | 4 | 133 / 84 |
| ALL | 40 | 0.84 | 0.94 | +0.11 | [+0.00, +0.21] | [-0.03, +0.24] | 1242 / 6986 | 3 | 28 / 68 |

runs=80 generations={"visionGraph@45976a8c7ca4305dd38bf66c7c6128732f6de4d7"}
arm3 served verbatim (model never ran): 0
arm3 median injected_tokens (façade estimate, already inside prompt_tokens): 1418
arm4 capped: 1  errors: 0
