# ── Stage 1: native parsing core wheel (Rust + maturin) ───────────────────────
# Builds rust/metabook_core into an abi3 wheel. The Rust toolchain is copied
# from the official image so the slim Python base keeps its matching glibc.
FROM python:3.13-slim AS rust-builder

COPY --from=rust:1.91-slim-bookworm /usr/local/cargo /usr/local/cargo
COPY --from=rust:1.91-slim-bookworm /usr/local/rustup /usr/local/rustup
ENV CARGO_HOME=/usr/local/cargo \
    RUSTUP_HOME=/usr/local/rustup \
    PATH=/usr/local/cargo/bin:$PATH

RUN apt-get update \
    && apt-get install -y --no-install-recommends gcc libc6-dev \
    && rm -rf /var/lib/apt/lists/* \
    && pip install --no-cache-dir "maturin>=1.8,<2"

WORKDIR /build
COPY rust/metabook_core/ ./
RUN maturin build --release --out /wheels


# ── Stage 2: dependency builder ───────────────────────────────────────────────
FROM python:3.13-slim AS builder

WORKDIR /build

COPY requirements.txt .
COPY --from=rust-builder /wheels /wheels
RUN pip install --no-cache-dir --prefix=/install -r requirements.txt /wheels/*.whl


# ── Stage 3: lean runtime image ───────────────────────────────────────────────
FROM python:3.13-slim AS runtime

# Non-root user for security
RUN useradd -m -u 1000 appuser

WORKDIR /app

# Copy only installed packages from builder
COPY --from=builder /install /usr/local

# Copy application source
COPY --chown=appuser:appuser src/ ./src/

# The image ships the native core; set METABOOK_PARSER=python to opt out.
ENV PYTHONPATH=/app/src \
    METABOOK_PARSER=rust

USER appuser

EXPOSE 8000

HEALTHCHECK --interval=30s --timeout=10s --retries=3 \
    CMD python -c "import httpx; httpx.get('http://localhost:8000/health').raise_for_status()"

CMD ["uvicorn", "metabook_py.main:app", "--host", "0.0.0.0", "--port", "8000", "--workers", "1"]
