# Hebdomiam

Application de planification de repas hebdomadaire (batch cooking). **Toujours lire
`PRD.md`** : vision, décisions structurantes (§4), modèle de données (§6),
architecture (§7), exigences de sécurité (§8) et jalons (§11).

## Langue

Communiquer avec l'utilisateur **en français**. L'interface est bilingue FR/EN :
toute chaîne visible passe par i18n (`i18n/locales/*.json`), jamais en dur.

## Commandes

- `npm run dev` — développement (DB : `docker compose -f docker-compose.dev.yml up -d`)
- `npm run lint` / `npm run typecheck` / `npm test` — à faire passer avant tout commit
- `npm run test:e2e` — Playwright (nécessite DB migrée + `npm run build`)
- `npm run db:generate` — nouvelle migration après modification de `server/database/schema.ts`

## Conventions

- **Toute entrée serveur est validée par Zod** (`readValidatedBody`), schémas dans
  `shared/schemas/` (partagés client/serveur via `#shared`).
- **Isolation multi-utilisateur** : toute requête DB sur des données métier filtre par
  `userId` de la session (`requireUserSession`). Écrire un test IDOR pour chaque
  nouvelle ressource.
- Mots de passe : Argon2id via `hashUserPassword`/`verifyUserPassword`
  (`server/utils/password.ts`) — ne pas utiliser les `hashPassword`/`verifyPassword`
  (scrypt) auto-importés par nuxt-auth-utils.
- Messages d'erreur API = clés i18n (ex. `errors.emailTaken`), traduites côté client
  (`app/utils/error-message.ts`).
- Quantités en `decimal` Drizzle, jamais en `float`.
- Routes d'auth : rate limiting applicatif (`assertRateLimit`) + `rate-limit-auth@file`
  côté Traefik en prod.
- Commits : Conventional Commits (`feat:`, `fix:`, `chore:`…), messages en français.
- Le compose de prod cible le home lab (réseau Traefik externe `proxy`) — ne pas y
  ajouter de reverse proxy ni publier de ports.
