# syntax=docker/dockerfile:1

# ── Front : SPA Nuxt générée (.output/public) ──────────────────────────────
# Images de base épinglées par empreinte (Dependabot les fait suivre) : une
# release est reproductible et un tag amont déplacé n'entre pas dans l'image.
FROM node:24-slim@sha256:dc26ae92c9fb15cf6b1d63e20f0944b2efe0f115b20e00be48fb23fceb061c54 AS front
WORKDIR /app
COPY package.json package-lock.json ./
# --ignore-scripts : le postinstall (nuxt prepare) a besoin des sources,
# copiées à l'étape suivante ; nuxt generate refait le prepare de toute façon.
RUN npm ci --ignore-scripts
COPY . .
RUN npm run generate

# ── API : binaire Rust en release ───────────────────────────────────────────
FROM rust:1.100-slim-bookworm@sha256:bb875b0f8ce726053e259034848c471840e96bfe591fd52556178ae9a05e6126 AS api-build
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
# distroless/base-nossl : glibc + certificats + tzdata, ni shell ni
# gestionnaire de paquets, et PAS d'OpenSSL (le binaire fait son TLS avec
# rustls : libssl ne serait qu'une surface d'attaque à patcher — vécu avec
# CVE-2026-84782 remonté par Trivy). Utilisateur non-root (uid 65532).
FROM gcr.io/distroless/base-nossl-debian12:nonroot@sha256:be40c00dfabd86576d92666e87e406714d5618342de1a0c213ad232de255172e AS api-runtime
WORKDIR /app
# Seule bibliothèque hors glibc requise par le binaire (déroulement de pile).
COPY --from=api-build /lib/x86_64-linux-gnu/libgcc_s.so.1 /lib/x86_64-linux-gnu/libgcc_s.so.1
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
