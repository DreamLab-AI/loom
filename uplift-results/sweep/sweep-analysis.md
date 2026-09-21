# Cross-provider uplift sweep — analysis

10 models, 510-question set (seed 42), uniform temp=0 / max_tokens=2048.
Sorted by gain-over-copy (recall beyond what a no-op extractor of the injected context would get).

| model | n | raw | scaffold | uplift [naive CI] | domain-clustered CI | copy ceiling | gain over copy | trunc | retried |
|---|---|---|---|---|---|---|---|---|---|
| gemini-2.5-flash-lite | 510 | 0.227 | 0.897 | 0.670 [0.635,0.704] | [0.623,0.716] | 0.964 | **-0.067** | 16 | 0 |
| gpt-4.1-mini | 510 | 0.162 | 0.905 | 0.743 [0.713,0.774] | [0.697,0.787] | 0.964 | **-0.060** | 0 | 0 |
| mistral-small-24b | 510 | 0.285 | 0.906 | 0.621 [0.586,0.657] | [0.558,0.682] | 0.964 | **-0.058** | 0 | 0 |
| qwen-2.5-72b | 510 | 0.309 | 0.914 | 0.605 [0.570,0.641] | [0.562,0.649] | 0.964 | **-0.050** | 0 | 0 |
| llama-3.3-70b | 510 | 0.227 | 0.916 | 0.688 [0.653,0.723] | [0.632,0.738] | 0.964 | **-0.049** | 0 | 0 |
| deepseek-chat | 510 | 0.347 | 0.924 | 0.578 [0.542,0.614] | [0.532,0.622] | 0.964 | **-0.040** | 0 | 0 |
| glm-4.6 | 510 | 0.243 | 0.928 | 0.685 [0.650,0.720] | [0.641,0.727] | 0.964 | **-0.036** | 174 | 5 |
| claude-haiku-4.5 | 510 | 0.151 | 0.934 | 0.783 [0.751,0.812] | [0.741,0.827] | 0.964 | **-0.031** | 0 | 0 |
| gemini-3.5-flash-lite | 510 | 0.323 | 0.938 | 0.615 [0.580,0.650] | [0.565,0.667] | 0.964 | **-0.026** | 0 | 0 |
| gemini-3.7-flash-t0 | 510 | 0.375 | 0.942 | 0.567 [0.532,0.603] | [0.524,0.611] | 0.964 | **-0.022** | 0 | 0 |

## pgfplots — raw vs scaffold vs copy-ceiling (per model)
```
% raw
(gemini-2.5-flash-lite,0.227) (gpt-4.1-mini,0.162) (mistral-small-24b,0.285) (qwen-2.5-72b,0.309) (llama-3.3-70b,0.227) (deepseek-chat,0.347) (glm-4.6,0.243) (claude-haiku-4.5,0.151) (gemini-3.5-flash-lite,0.323) (gemini-3.7-flash-t0,0.375) 
% scaffold
(gemini-2.5-flash-lite,0.897) (gpt-4.1-mini,0.905) (mistral-small-24b,0.906) (qwen-2.5-72b,0.914) (llama-3.3-70b,0.916) (deepseek-chat,0.924) (glm-4.6,0.928) (claude-haiku-4.5,0.934) (gemini-3.5-flash-lite,0.938) (gemini-3.7-flash-t0,0.942) 
% copy ceiling
(gemini-2.5-flash-lite,0.964) (gpt-4.1-mini,0.964) (mistral-small-24b,0.964) (qwen-2.5-72b,0.964) (llama-3.3-70b,0.964) (deepseek-chat,0.964) (glm-4.6,0.964) (claude-haiku-4.5,0.964) (gemini-3.5-flash-lite,0.964) (gemini-3.7-flash-t0,0.964) 
% gain over copy (signed)
(gemini-2.5-flash-lite,-0.067) (gpt-4.1-mini,-0.060) (mistral-small-24b,-0.058) (qwen-2.5-72b,-0.050) (llama-3.3-70b,-0.049) (deepseek-chat,-0.040) (glm-4.6,-0.036) (claude-haiku-4.5,-0.031) (gemini-3.5-flash-lite,-0.026) (gemini-3.7-flash-t0,-0.022) 
```