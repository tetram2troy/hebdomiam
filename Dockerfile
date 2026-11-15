# syntax=docker/dockerfile:1

# ── Front : SPA Nuxt générée (.output/public) ──────────────────────────────
FROM node:24-slim AS front
WORKDIR /app
COPY package.json package-lock.json ./
# --ignore-scripts : le postinstall (nuxt prepare) a besoin des sources,
# copiées à l'étape suivante ; nuxt generate refait le prepare de toute façon.
RUN npm ci --ignore-scripts
COPY . .
RUN npm run generate

# ── API : binaire Rust en release ───────────────────────────────────────────
FROM rust:1.100-slim-bookworm AS api-build
WORKDIR /app
COPY api/Cargo.toml api/Cargo.lock ./
COPY api/src ./src
COPY api/migrations ./migrations
# Caches BuildKit : registre cargo et dossier target survivent entre builds.
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/app/target \
    cargo build --release --locked \
 && cp target/release/hebdomiam /hebdomiam

# ── Runtime de l'API seule (e2e : le front est monté depuis l'hôte) ────────
# distroless/cc : glibc + certificats + tzdata, ni shell ni gestionnaire de
# paquets ; utilisateur non-root (uid 65532).
FROM gcr.io/distroless/cc-debian12:nonroot AS api-runtime
WORKDIR /app
# PORT reste surchargeable au runtime (contrat docs/HOSTING.md §2.2).
# APP_ENV=production active la validation stricte de la configuration.
ENV APP_ENV=production \
    PORT=3000 \
    STATIC_DIR=/app/public
COPY --from=api-build /hebdomiam /app/hebdomiam
USER nonroot
EXPOSE 3000
# Sonde intégrée au binaire (pas de curl dans l'image).
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
  CMD ["/app/hebdomiam", "healthcheck"]
# Migrations en job dédié au déploiement (PRD §7.3) : /app/hebdomiam migrate
ENTRYPOINT ["/app/hebdomiam"]
CMD ["serve"]

# ── Image de production : API + front ──────────────────────────────────────
FROM api-runtime AS runtime
COPY --from=front /app/.output/public /app/public
