# Règles de codage Hebdomiam

Référentiel des étapes 2 (code) et 7 (revue de conformité) de la skill
`nouvelle-feature`. Chaque règle vient d'une leçon du projet.

## Serveur

- [ ] Entrée validée par Zod (`readValidatedBody` / `getValidatedRouterParams`),
      schéma dans `shared/schemas/` (partagé via `#shared`), types de sortie
      dans `shared/types/`.
- [ ] Accès DB dans une fonction de `server/utils/*-repo.ts` **scopée sur `userId`**
      (chaîne de jointures jusqu'à `users`, retourne `null` si l'objet n'appartient
      pas à l'utilisateur — jamais de vérification dans le handler).
- [ ] Résultats métier = **unions de statuts** (`'ok' | 'invalidPassword' | …`) que
      le handler mappe vers `statusCode` + clé i18n ; la logique reste testable
      en intégration sans HTTP.
- [ ] Route `/api/admin/*` : `requireAdminSession` (`server/utils/session.ts`) — le
      rôle est relu en base, jamais décidé sur le cookie seul.
- [ ] Action critique (mutation, événement de sécurité) journalisée :
      `logEvent`/`logWarn` (`server/utils/logger.ts`), nom `domaine.action`,
      jamais de secret, jeton ni contenu utilisateur, PII minimale (userId, IP
      via `clientIp`).
- [ ] Route sensible (auth, coût IA, envoi de courriel) : `assertRateLimit`,
      bucket suffixé `:${user.id}` quand la route est authentifiée.
- [ ] Erreurs API = clés i18n (`errors.*`) traduites côté client — la clé existe
      dans `fr.json`, `en.json` **et** l'allowlist `KNOWN_KEYS` de
      `app/utils/error-message.ts` (sinon elle dégénère en `errors.unknown`).
- [ ] Quantités en `decimal` Drizzle, jamais en `float` ; schéma modifié →
      `npm run db:generate` (jamais de SQL de schéma écrit à la main ; migration
      de données → `npx drizzle-kit generate --custom`).
- [ ] Mots de passe : `hashUserPassword`/`verifyUserPassword` (Argon2id) — jamais
      les `hashPassword`/`verifyPassword` (scrypt) auto-importés par nuxt-auth-utils.
- [ ] Dans `server/utils/`, imports **explicites** entre utils (l'auto-import Nitro
      ne joue pas dans Vitest — un import manquant ne casse qu'aux tests).
- [ ] Fetch sortant → garde SSRF (`fetchExternalPage`), jamais `fetch` nu.

## Front

- [ ] Toute chaîne visible : clé dans `i18n/locales/fr.json` **et** `en.json` —
      jamais en dur, dans les deux langues d'un même commit.
- [ ] Couleurs, espacements, rayons : **tokens CSS** (`app/assets/css/tokens.css`),
      jamais de valeur en dur ; classes globales de `base.css` (`.btn*`, `.chip`,
      `.card`, `.error`, `.hint`) avant tout style nouveau.
- [ ] Page protégée : `definePageMeta({ middleware: 'auth' })` (+ `'admin'` pour
      l'administration) ; le middleware client n'est qu'un garde d'affichage,
      la vraie autorisation est côté serveur.
- [ ] `data-testid` en kebab-case sur tout élément qu'un test cible.
- [ ] Modèle de page : la fonctionnalité voisine la plus proche (ex. `reglages.vue`
      pour un écran à sections `.panel`/`.srow`) — pas de nouveau pattern quand un
      existe.

## Style

- [ ] Commentaires **en français**, qui expliquent le *pourquoi* (contrainte,
      décision, piège), jamais le *quoi*.
- [ ] Nommage : fichiers kebab-case ; composants `Hb*` pour le générique ;
      repos plats `fn(db, userId, ...)` ; événements de log `domaine.action`.
- [ ] Commits : Conventional Commits en français, découpés par sujet, corps
      expliquant le pourquoi et les choix de sécurité.
