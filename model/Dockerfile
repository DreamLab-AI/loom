# Loom model engine — llama.cpp llama-server, CUDA build for the host's 2x Quadro RTX 6000
# (Turing sm_75 — CMAKE_CUDA_ARCHITECTURES=75, NOT 89/Ada). Serves Qwen3.8-27B on :8085.
#
# Pinned to llama.cpp release b11982 (8a199ad, 2026-10-29). Over the previous pin
# (cc83d7b, 2026-08-29): GDN chunked/state-column CUDA kernels for qwen35 (#29353,
# #30087), CUDA graphs for the MTP draft (#28549), MTP/DFlash image-turn fixes
# (#30257, #28715, #28587), and Turing-relevant FlashAttention race fixes (#30103,
# #30573). Measured on the HP with Hemmingway-1.5 Q8_0, MTP n=3, 2026-10-30: decode
# unchanged (25.8 tok/s technical both), image turns unchanged (27.6 vs 28.2 tok/s),
# prompt processing 874 vs 804 tok/s on an 11k-token prompt (+9%).
# Rollback image: loom-model:local (cc83d7b).
# NOTE: vision + draft-dflash was flaky at cc83d7b; the fixes above are not re-tested
# with DFlash here, so keep SPEC=dflash on text-only profiles (VISION=0).
# CUDA 13.3 matches the host toolkit (driver 610.43.03).
ARG CUDA_VERSION=13.3.0

FROM nvidia/cuda:${CUDA_VERSION}-devel-ubuntu24.04 AS build
RUN apt-get update && apt-get install -y --no-install-recommends \
        git cmake ninja-build build-essential curl ca-certificates libcurl4-openssl-dev \
    && rm -rf /var/lib/apt/lists/*

ARG LLAMA_COMMIT=8a199ad31a7eb27d5a46a6b08026176d82bc9894
RUN git init /src && cd /src \
    && git remote add origin https://github.com/ggml-org/llama.cpp \
    && git fetch --depth 1 origin ${LLAMA_COMMIT} \
    && git checkout FETCH_HEAD

# GGML_NATIVE=OFF: image is host-only today, but don't tie the binary to the build box's
# CPU flags — the GPUs do the work.
RUN cmake -S /src -B /src/build -G Ninja \
        -DCMAKE_BUILD_TYPE=Release \
        -DGGML_CUDA=ON \
        -DCMAKE_CUDA_ARCHITECTURES=75 \
        -DGGML_NATIVE=OFF \
        -DBUILD_SHARED_LIBS=OFF \
    && cmake --build /src/build --target llama-server -j

FROM nvidia/cuda:${CUDA_VERSION}-runtime-ubuntu24.04
RUN apt-get update && apt-get install -y --no-install-recommends \
        curl libcurl4t64 libgomp1 ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=build /src/build/bin/llama-server /usr/local/bin/llama-server
COPY serve-qwen38.sh /usr/local/bin/serve-qwen38.sh
RUN chmod +x /usr/local/bin/serve-qwen38.sh

# Model load (31.5GB weights split across two GPUs) takes a while — generous start period.
HEALTHCHECK --interval=30s --timeout=5s --start-period=300s --retries=5 \
  CMD curl -fsS "http://127.0.0.1:${PORT:-8085}/health" || exit 1

ENTRYPOINT ["/usr/local/bin/serve-qwen38.sh"]
