# Hebdomiam 🍲

Application de planification de repas hebdomadaire : le vendredi, choisis 3–4 recettes
pour la semaine, obtiens la liste d'épicerie consolidée, puis cuisine en lot la fin de
semaine. Voir [PRD.md](PRD.md) pour la vision complète et la feuille de route.

## Stack

Nuxt (front + API Nitro) · TypeScript strict · PostgreSQL + Drizzle ORM · Zod ·
nuxt-auth-utils (sessions scellées, Argon2id) · @nuxtjs/i18n (FR/EN) · nuxt-security ·
Docker · GitHub Actions

## Développement

```bash
cp .env.example .env            # ajuster au besoin
docker compose -f docker-compose.dev.yml up -d   # PostgreSQL local
npm install
npm run db:migrate              # applique les migrations
npm run dev                     # http://localhost:3000
```

## Commandes

| Commande | Rôle |
|---|---|
| `npm run dev` | Serveur de développement |
| `npm run build` | Build de production |
| `npm run lint` / `npm run typecheck` | Qualité statique |
| `npm test` | Tests unitaires (Vitest) |
| `npm run test:integration` | Tests d'intégration (Vitest + DB éphémère `hebdomiam_test`) |
| `npm run test:e2e` | Tests E2E (Playwright — nécessite la DB et un build) |
| `npm run db:generate` | Génère une migration depuis `server/database/schema.ts` |
| `npm run db:migrate` | Applique les migrations |

## Déploiement (home lab)

L'intégration au serveur passe par **[deploy/serveur-perso/](deploy/serveur-perso/README.md)** :
image GHCR taguée (workflow `release.yml` sur tag `v*`), blocs à coller dans le compose
unique du serveur, migrations en job dédié (`node .output/server/migrate.mjs`),
sauvegarde `pg_dump`. L'app est exposée sur `recettes.${DOMAIN}` avec la chaîne de
middlewares standard et `rate-limit-auth@file` sur `/api/auth` ; la base vit sur un
réseau `internal: true` sans accès Internet. Gabarit d'origine : `docs/HOSTING.md`.

Banc d'essai local du même modèle (build sur place) :

```bash
cp .env.example .env   # renseigner DOMAIN, POSTGRES_PASSWORD, NUXT_SESSION_PASSWORD
docker compose up -d --build
```

## Structure

```
app/               Pages, composants, utilitaires client
server/api/        Routes API (Nitro)
server/database/   Schéma Drizzle + migrations
server/utils/      DB, mots de passe (Argon2id), rate limiting, 2FA, courriels,
                   journalisation structurée (logger.ts)
shared/schemas/    Schémas Zod partagés client/serveur
i18n/locales/      Traductions FR/EN
tests/unit/        Tests unitaires
tests/integration/ Tests d'intégration (DB réelle, IDOR)
e2e/               Tests Playwright
```
