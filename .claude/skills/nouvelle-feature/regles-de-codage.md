# Règles de codage Hebdomiam

Référentiel de l'étape 2 (développement) de la skill `nouvelle-feature` et de la
revue de la skill `relecture`. Chaque règle vient d'une leçon du projet.

## API (Rust, `api/`)

- [ ] Entrée validée par un extracteur `ValidJson` / `ValidPath` / `ValidQuery`
      (`api/src/validation.rs`) : struct brute `Deserialize` + `impl Validate`
      qui normalise (trim, minuscules) et borne → forme métier. Jamais de
      `Json<T>` nu. Le type de sortie JSON est miroité dans `shared/types/`.
- [ ] Accès DB dans `api/src/repo/*.rs`, fonction **scopée sur `user_id`**
      (chaîne de jointures jusqu'à `users`, `None` si l'objet n'appartient pas
      à l'utilisateur — jamais de vérification dans le handler). Requêtes
      paramétrées (`bind`), jamais de SQL concaténé avec une entrée.
- [ ] Résultats métier = **enums** (`DeleteAccount::InvalidPassword`, `PlanError`…)
      que le handler mappe vers statut HTTP + clé i18n ; la logique reste
      testable en intégration sans HTTP.
- [ ] Route `/api/admin/*` : extracteur `AdminUser` (`api/src/session.rs`) — le
      rôle est relu en base, jamais décidé sur le cookie seul.
- [ ] Action critique (mutation, événement de sécurité) journalisée :
      `log_event`/`log_warn` (`api/src/logger.rs`), nom `domaine.action`,
      jamais de secret, jeton ni contenu utilisateur, PII minimale (userId, IP
      via l'extracteur `ClientIp`).
- [ ] Route sensible (auth, coût IA, envoi de courriel) : `state.rate_limit(...)`,
      bucket suffixé `:{user.id}` quand la route est authentifiée.
- [ ] Erreurs API = clés i18n (`errors.*`) traduites côté client — la clé existe
      dans `fr.json`, `en.json` **et** l'allowlist `KNOWN_KEYS` de
      `app/utils/error-message.ts` (sinon elle dégénère en `errors.unknown`).
- [ ] Quantités en `numeric` (`rust_decimal`), jamais en `float` en base ;
      schéma modifié → nouveau fichier `api/migrations/NNNN_description.sql`
      (jamais modifier une migration déjà publiée).
- [ ] Mots de passe : `crypto::hash_password` / `verify_password` (Argon2id,
      hors du runtime async).
- [ ] Fetch sortant → `SafeFetcher` (garde SSRF + résolveur DNS verrouillé),
      jamais un `reqwest::Client` nu.
- [ ] `npm run api:lint` propre (`cargo fmt` + `clippy -D warnings`) ; tests
      d'intégration dans `api/tests/` (chaque test a sa base, `TestDb::new()`).

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
