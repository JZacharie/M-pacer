# Image de production M-pacer : binaire Rust dans une base minimale.
# Construction multi-etapes : aucune chaine d'outils dans l'image finale.

FROM rust:1.98-bookworm AS builder
WORKDIR /src

# Le cache de dependances est reutilise tant que les manifestes ne changent pas.
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
RUN cargo build --release --locked -p mpacer-api

FROM debian:bookworm-slim AS runtime
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --uid 10001 --create-home --shell /usr/sbin/nologin mpacer

COPY --from=builder /src/target/release/mpacer-api /usr/local/bin/mpacer-api

# Toutes les donnees vivent sous /data (volume persistant) ; le reste est en lecture seule.
RUN mkdir -p /data/media && chown -R 10001:10001 /data
VOLUME ["/data"]

USER 10001:10001
EXPOSE 8080

# La base PostgreSQL est fournie par l'environnement (secret CloudNativePG) :
#   MPACER_DB_HOST / MPACER_DB_PORT / MPACER_DB_NAME / MPACER_DB_USER / MPACER_DB_PASSWORD
# ou une URL complete via MPACER_DATABASE_URL.
ENV MPACER_BIND=0.0.0.0:8080 \
    MPACER_ENV=production \
    MPACER_COOKIE_SECURE=true \
    RUST_LOG=info,mpacer_api=info,tower_http=warn

HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD curl -fsS http://127.0.0.1:8080/healthz || exit 1

ENTRYPOINT ["/usr/local/bin/mpacer-api"]
