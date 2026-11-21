# Hebdomiam 🍲

Application de planification de repas hebdomadaire : le vendredi, choisis 3–4 recettes
pour la semaine, obtiens la liste d'épicerie consolidée, puis cuisine en lot la fin de
semaine. Voir [PRD.md](PRD.md) pour la vision complète et la feuille de route.

## Stack

- **Front** : Nuxt 4 en SPA (`ssr: false`, `nuxt generate`) · Vue 3 · TypeScript strict ·
  @nuxtjs/i18n (FR/EN)
- **API** : Rust — axum · sqlx (PostgreSQL) · sessions scellées AES-256-GCM · Argon2id ·
  TOTP · import schema.org / API Claude — un seul binaire qui sert aussi le front
- Docker (image distroless non-root) · GitHub Actions

**Aucun compilateur Rust n'est requis sur le poste** : cargo, rustfmt et clippy tournent
dans l'image de dev `api/Dockerfile.dev` via Docker Compose.

## Développement

```bash
cp .env.example .env                              # ajuster au besoin
docker compose -f docker-compose.dev.yml up -d    # PostgreSQL + API Rust (:8080, rechargement à chaud)
npm install
npm run dev                                       # front sur http://localhost:3000 (relaie /api)
```

Au premier lancement, la compilation de l'API prend quelques minutes (dépendances mises
en cache dans les volumes `cargo-*`). Le service `api` applique les migrations puis
redémarre à chaque modification de `api/` (`npm run api:logs` pour suivre).

## Commandes

| Commande | Rôle |
|---|---|
| `npm run dev` | Front de développement (proxy `/api` → API Rust) |
| `npm run generate` | Génère le front (SPA) dans `.output/public` |
| `npm run lint` / `npm run typecheck` / `npm test` | Front : qualité statique et tests unitaires (Vitest) |
| `npm run api:test` | API : tests unitaires + intégration (base éphémère par test) |
| `npm run api:lint` / `npm run api:fmt` | API : `cargo fmt --check` + `clippy -D warnings` / formatage |
| `npm run api:cargo -- <args>` | N'importe quelle commande cargo, dans Docker |
| `npm run test:e2e` | E2E Playwright contre le binaire Rust (après `npm run generate`) |

Nouvelle migration : ajouter un fichier SQL numéroté dans `api/migrations/` (embarqué
dans le binaire, appliqué par `hebdomiam migrate`).

## Déploiement (home lab)

L'intégration au serveur passe par **[deploy/serveur-perso/](deploy/serveur-perso/README.md)** :
image GHCR taguée (workflow `release.yml` sur tag `v*`), blocs à coller dans le compose
unique du serveur, migrations en job dédié (`migrate`), sauvegarde `pg_dump`. L'app est
exposée sur `recettes.${DOMAIN}` avec la chaîne de middlewares standard et
`rate-limit-auth@file` sur `/api/auth` ; la base vit sur un réseau `internal: true` sans
accès Internet. Gabarit d'origine : `docs/HOSTING.md`.

Banc d'essai local du même modèle (build sur place) :

```bash
cp .env.example .env   # renseigner DOMAIN, POSTGRES_PASSWORD, SESSION_PASSWORD
docker compose up -d --build
```

## Structure

```
app/               Front : pages, composants, composables (session), utilitaires
shared/            Types et énumérations partagés avec l'API (contrat JSON)
i18n/locales/      Traductions FR/EN
api/src/           API Rust
  domain/          Logique métier pure (unités, agrégation, rotation)
  repo/            Accès base — isolation multi-utilisateur garantie ici
  routes/          Handlers HTTP (validation des entrées)
  import/          schema.org, API Claude, fetch durci SSRF, ré-encodage photo
api/migrations/    Migrations SQL (embarquées dans le binaire)
api/tests/         Tests d'intégration (base réelle, IDOR, HTTP de bout en bout)
tests/unit/        Tests unitaires front
e2e/               Tests Playwright
```
