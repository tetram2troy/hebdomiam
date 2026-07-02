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
| `npm run test:e2e` | Tests E2E (Playwright — nécessite la DB et un build) |
| `npm run db:generate` | Génère une migration depuis `server/database/schema.ts` |
| `npm run db:migrate` | Applique les migrations |

## Déploiement (home lab)

Le `docker-compose.yml` s'appuie sur le Traefik existant (réseau externe `proxy`) :
l'app est exposée sur `hebdomiam.${DOMAIN}` avec la chaîne de middlewares standard et
`rate-limit-auth@file` sur `/api/auth`. La base de données vit sur un réseau
`internal: true` sans accès Internet. Les migrations tournent dans un job dédié
(`migrate`) avant le démarrage de l'app.

```bash
cp .env.example .env   # renseigner DOMAIN, POSTGRES_PASSWORD, NUXT_SESSION_PASSWORD
docker compose up -d --build
```

## Structure

```
app/               Pages, composants, utilitaires client
server/api/        Routes API (Nitro)
server/database/   Schéma Drizzle + migrations
server/utils/      DB, mots de passe (Argon2id), rate limiting
shared/schemas/    Schémas Zod partagés client/serveur
i18n/locales/      Traductions FR/EN
tests/unit/        Tests unitaires
e2e/               Tests Playwright
```
