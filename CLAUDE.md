# Hebdomiam

Application de planification de repas hebdomadaire (batch cooking). **Toujours lire
`PRD.md`** : vision, décisions structurantes (§4), modèle de données (§6),
architecture (§7), exigences de sécurité (§8) et jalons (§11).

Architecture : front **Nuxt en SPA** (`app/`, généré par `nuxt generate`) + **API Rust**
(`api/`, axum + sqlx) — un seul binaire sert l'API et le front en production.

## Langue

Communiquer avec l'utilisateur **en français**. L'interface est bilingue FR/EN :
toute chaîne visible passe par i18n (`i18n/locales/*.json`), jamais en dur.

## Commandes

**Aucun compilateur Rust sur le poste** : toute commande cargo passe par Docker
(`api/Dockerfile.dev`, service `api` de `docker-compose.dev.yml`).

- `docker compose -f docker-compose.dev.yml up -d` — DB + API Rust (:8080, migrations
  puis rechargement à chaud) ; `npm run dev` — front (:3000, relaie `/api`)
- `npm run lint` / `npm run typecheck` / `npm test` — front, à faire passer avant tout commit
- `npm run api:lint` / `npm run api:test` — API (fmt + clippy `-D warnings` ; tests
  unitaires + intégration, base éphémère par test), à faire passer avant tout commit
- `npm run api:cargo -- <args>` — toute autre commande cargo
- `npm run test:e2e` — Playwright contre le binaire Rust (après `npm run generate` ;
  l'image `api-runtime` est reconstruite par `e2e/global-setup.ts`)
- Nouvelle migration : fichier SQL numéroté dans `api/migrations/` (jamais modifier
  une migration publiée)

## Skills projet (`.claude/skills/`)

- `/nouvelle-feature` — workflow complet d'une fonctionnalité (PRD → conventions →
  pyramide de tests → validation → doc → commits). À suivre pour toute demande de feature.
- `/verifier-ui` — capture Playwright inspectée avant d'annoncer un changement visuel.
- `/e2e-stable` — pièges connus de la suite Playwright et méthode de diagnostic.

## Conventions

- **Toute entrée serveur est validée** par les extracteurs `ValidJson` / `ValidPath` /
  `ValidQuery` (`api/src/validation.rs`) ; les formes JSON renvoyées sont miroitées
  dans `shared/types/` pour le front.
- **Isolation multi-utilisateur** : toute requête DB sur des données métier filtre par
  le `user_id` de la session (extracteur `CurrentUser`), dans `api/src/repo/`. Écrire
  un test IDOR (`api/tests/`) pour chaque nouvelle ressource.
- **Rôles** : toute route `/api/admin/*` passe par l'extracteur `AdminUser`
  (`api/src/session.rs`) — le rôle est relu **en base**, jamais décidé sur le cookie
  seul (le `role` en session ne sert qu'à l'affichage). La désactivation de compte est
  appliquée par le middleware `account_status` (`api/src/app.rs`).
- **Inscription publique fermée** (bootstrap seulement) : les comptes de test e2e se
  créent via `registerFreshUser` de `e2e/helpers.ts` (admin seedé par
  `e2e/global-setup.ts`), jamais par la page /register — voir la skill `e2e-stable`.
- Mots de passe : Argon2id via `crypto::hash_password` / `verify_password`
  (`api/src/crypto.rs`, mêmes paramètres et format PHC que l'ancien backend).
- Messages d'erreur API = clés i18n (ex. `errors.emailTaken`), traduites côté client
  (`app/utils/error-message.ts`).
- Quantités en `numeric` (`rust_decimal`), jamais en `float` en base.
- Routes d'auth : rate limiting applicatif (`state.rate_limit`) + `rate-limit-auth@file`
  côté Traefik en prod.
- **Journalisation** : toute action critique (mutation, événement de sécurité) passe par
  `log_event`/`log_warn`/`log_error` (`api/src/logger.rs`) — une ligne JSON nommée
  `domaine.action` ; jamais de secret ni de contenu utilisateur, PII minimale (userId).
- Commits : Conventional Commits (`feat:`, `fix:`, `chore:`…), messages en français.
- Le compose de prod cible le home lab (réseau Traefik externe `proxy`) — ne pas y
  ajouter de reverse proxy ni publier de ports. L'intégration réelle au serveur passe
  par `deploy/serveur-perso/` (image GHCR, gabarit dans `docs/HOSTING.md`) ; les
  migrations de prod s'exécutent en job dédié : commande `migrate` de l'image.
