# syntax=docker/dockerfile:1
# DiffusionGemma 26B-A4B — containerized llama.cpp diffusion fork + self-recovering front-end.
#
# Built FROM SOURCE (the diffusion fork is not in mainline llama.cpp as of 2026-06) inside a
# CUDA-devel image, then copied into a slim CUDA-runtime image. Building in-image keeps the
# binary glibc-consistent with the runtime image (the host's CachyOS glibc 2.43 binary will
# NOT run on Ubuntu's 2.39 — so we don't reuse host artifacts). The 26 GB model is NOT baked;
# it is bind-mounted at runtime. The NVIDIA driver (libcuda.so.1) is injected by the nvidia
# container runtime; only the CUDA runtime libs (cudart/cublas, CUDA 13) come from the image.
ARG CUDA_VERSION=13.0.1
ARG CUDA_ARCH=89

# ---- build stage --------------------------------------------------------------------------
FROM nvidia/cuda:${CUDA_VERSION}-devel-ubuntu24.04 AS build
ARG CUDA_ARCH
RUN apt-get update && apt-get install -y --no-install-recommends \
        build-essential cmake ca-certificates \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /src
# Reproducible source: `git archive HEAD` of the fork (commit pinned at build time).
COPY llama-diffgemma-src.tar.gz .
RUN mkdir -p tree && tar xzf llama-diffgemma-src.tar.gz -C tree && rm llama-diffgemma-src.tar.gz
WORKDIR /src/tree
# The devel image ships only the CUDA driver STUB (libcuda.so, no .so.1). The real driver is
# injected by the nvidia runtime at run time — but link time needs to resolve libcuda.so.1, so
# expose the stub under that name and put the stubs dir on the link path.
RUN ln -sf /usr/local/cuda/lib64/stubs/libcuda.so /usr/local/cuda/lib64/stubs/libcuda.so.1
# Native sm_89 (Ada) codegen; CURL off (no model downloads); build only the visual-server target.
RUN cmake -B build \
        -DCMAKE_BUILD_TYPE=Release \
        -DGGML_CUDA=ON \
        -DCMAKE_CUDA_ARCHITECTURES=${CUDA_ARCH} \
        -DLLAMA_CURL=OFF \
        -DLLAMA_BUILD_EXAMPLES=ON \
        -DLLAMA_BUILD_TESTS=OFF \
        -DLLAMA_BUILD_SERVER=OFF \
        -DCMAKE_EXE_LINKER_FLAGS="-L/usr/local/cuda/lib64/stubs -Wl,-rpath-link,/usr/local/cuda/lib64/stubs" \
        -DCMAKE_SHARED_LINKER_FLAGS="-L/usr/local/cuda/lib64/stubs -Wl,-rpath-link,/usr/local/cuda/lib64/stubs" \
    && cmake --build build --target llama-diffusion-gemma-visual-server -j"$(nproc)"

# ---- runtime stage ------------------------------------------------------------------------
FROM nvidia/cuda:${CUDA_VERSION}-runtime-ubuntu24.04 AS runtime
# libgomp1 = OpenMP runtime (libgomp.so.1) that ggml-cpu links; not in the slim CUDA image.
RUN apt-get update && apt-get install -y --no-install-recommends \
        python3 curl ca-certificates libgomp1 \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /opt/dg
# The visual-server binary + the llama.cpp/ggml shared libs it links against.
COPY --from=build /src/tree/build/bin/llama-diffusion-gemma-visual-server ./bin/
COPY --from=build /src/tree/build/bin/*.so* ./bin/
COPY dg_server.py ./dg_server.py
ENV LD_LIBRARY_PATH=/opt/dg/bin \
    DG_SERVER_BIN=/opt/dg/bin/llama-diffusion-gemma-visual-server \
    DG_MODEL=/models/diffusiongemma-26B-A4B-it-Q8_0.gguf \
    DG_HOST=0.0.0.0 \
    DG_PORT=8084 \
    NGL=99 \
    CUDA_VISIBLE_DEVICES=0 \
    GGML_CUDA_GRAPH_OPT=1
EXPOSE 8084
# Liveness = the backend child is actually alive (not just the HTTP shell up).
HEALTHCHECK --interval=30s --timeout=10s --start-period=240s --retries=3 \
    CMD curl -fsS http://127.0.0.1:8084/health | grep -q '"backend": "alive"' || exit 1
CMD ["python3", "/opt/dg/dg_server.py"]
