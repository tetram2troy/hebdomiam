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
- `/importer-recette <url>` — structurer une recette d'une page web et l'enregistrer
  par l'API à clé (`HEBDOMIAM_URL`, `HEBDOMIAM_API_KEY` dans l'environnement).

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
- **Clés d'API** : extracteur `ApiKeyUser` (`api/src/api_key.rs`), réservé aux routes
  `/api/v1/*` — une clé n'ouvre jamais une route de session, et une session n'ouvre
  pas `/api/v1/*`. Élargir la portée d'une clé est une décision PRD, pas un détail.
- Mots de passe : Argon2id via `crypto::hash_password` / `verify_password`
  (`api/src/crypto.rs`, mêmes paramètres et format PHC que l'ancien backend).
- **Sessions** : un changement d'état d'authentification (mot de passe, 2FA,
  déconnexion) incrémente `users.session_epoch` et révoque les cookies antérieurs ;
  re-sceller la session courante avec `CurrentSession::resealed` (jamais
  `SessionData::for_user`, qui la rajeunirait). Toute vérification de mot de passe
  derrière une session passe par `reserve_password_attempt`.
- **Liens par courriel** : jeton dans le fragment (`#token=`), lu côté front par
  `useLinkToken()` — jamais dans la query string (journaux d'accès).
- **CSP** : posée par l'API (`content_security_policy`, `api/src/app.rs`) ; aucun
  script inline écrit à la main (`public/*.js` + `app.head.script`), pas de `v-html`
  sur une donnée utilisateur.
- Messages d'erreur API = clés i18n (ex. `errors.emailTaken`), traduites côté client
  (`app/utils/error-message.ts`).
- Quantités en `numeric` (`rust_decimal`), jamais en `float` en base.
- Routes d'auth : rate limiting applicatif par IP (`state.rate_limit`) + verrou par
  compte sur les échecs (`limits.reserve_attempt` / `refund_attempt`) +
  `rate-limit-auth@file` côté Traefik en prod. L'IP client vient de l'extracteur
  `ClientIp` (en-têtes crus seulement depuis `TRUSTED_PROXIES`).
- URL fournie par l'utilisateur : `validation::url` avec `WEB` ou `HTTPS_ONLY` —
  jamais de schéma libre (`javascript:` finirait dans un `href`).
- **Journalisation** : toute action critique (mutation, événement de sécurité) passe par
  `log_event`/`log_warn`/`log_error` (`api/src/logger.rs`) — une ligne JSON nommée
  `domaine.action` ; jamais de secret ni de contenu utilisateur, PII minimale (userId).
- Commits : Conventional Commits (`feat:`, `fix:`, `chore:`…), messages en français.
- Le compose de prod cible le home lab (réseau Traefik externe `proxy`) — ne pas y
  ajouter de reverse proxy ni publier de ports. L'intégration réelle au serveur passe
  par `deploy/serveur-perso/` (image GHCR, gabarit dans `docs/HOSTING.md`) ; les
  migrations de prod s'exécutent en job dédié : commande `migrate` de l'image.
