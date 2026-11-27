---
name: e2e-stable
description: Pièges connus de la suite Playwright d'Hebdomiam (hydratation Nuxt, rate limiting, v-model number, serveur résiduel) et méthode de diagnostic d'un échec e2e. Utiliser avant d'écrire ou modifier un spec e2e, et dès qu'un test e2e échoue.
---

# Écrire et déboguer les e2e d'Hebdomiam

La suite tourne contre un seul serveur — le binaire Rust (conteneur
`hebdomiam-e2e` en local, binaire natif `E2E_API_BIN` en CI), démarré par
`e2e/global-setup.ts` — timeouts
60 s/test et 10 s/assertion. Chaque piège ci-dessous a réellement causé un échec.

## Pièges à l'écriture

1. **Rendu client (SPA)** : après tout `page.goto()` ou `page.reload()` suivi d'un
   `fill`/`click`, insérer `await page.waitForLoadState('networkidle')`.
   Un fill trop tôt est effacé par v-model à l'hydratation ; un clic trop tôt
   n'atteint pas le gestionnaire Vue. Les navigations client (clic sur NuxtLink)
   n'en ont pas besoin.
2. **Course de navigation** : après un clic de lien, si le champ à remplir existe
   aussi sur la page de départ (ex. « Courriel » sur /login ET /mot-de-passe-oublie),
   attendre `await expect(page).toHaveURL(/…/)` avant de remplir.
3. **Requête en vol** : ne jamais `reload()` juste après un clic qui déclenche un
   PATCH — capturer `page.waitForResponse(…)` avant le clic et l'attendre.
4. **Rate limiting (5/min/IP)** : une IP dédiée par fichier via
   `test.use({ extraHTTPHeaders: { 'x-forwarded-for': '203.0.113.X' } })`.
   Registre : .11-.13 recettes · .21 design · .31 auth · .41 planifier ·
   .51 reglages · .52 securite-2fa · .53 courriel · .54 i18n · .55 cles-api · .61-.62 admin ·
   .63+ captures/diagnostics · .71 epicerie · .81 navigation · .95 import · .98 cuisine.
   Prendre une adresse libre et compter les logins du fichier (max 5/min).
   Le helper partagé consomme sur son propre pool (198.51.x.y aléatoire) —
   jamais un compteur par module : il se réinitialise à chaque worker et les
   fichiers tournent en parallèle.
5. **v-model sur `<input type="number">`** : Vue écrit un *number* dans la ref
   (ou `''` si vidé). Côté app : typer `ref<string | number>` et passer par
   `String(x).trim()`. Une méthode string sur un number lève une exception
   silencieuse dans le handler — le formulaire ne soumet jamais, sans erreur visible.
6. **Assertions de géométrie** : pour une exigence de mise en page (largeur,
   position), verrouiller par `boundingBox()` — c'est ce qui distingue « le code
   a changé » de « l'écran a changé ».
7. Les liens de courriel portent le jeton dans le **fragment** (`/reinitialiser#token=…`) :
   un `page.goto` qui ne change que le fragment ne recharge pas le document —
   passer par une autre page avant de rejouer un lien.
   Les jetons de courriel sont stockés hachés : pour tester un lien, **insérer un
   jeton connu en DB** depuis le spec (pattern dans `e2e/courriel.spec.ts`).
8. Les codes TOTP se génèrent avec `generateSync({ secret })` d'otplib, secret
   récupéré depuis l'écran d'activation (pattern dans `e2e/securite-2fa.spec.ts`).
   **Anti-rejeu** : un code déjà accepté (activation comprise) est refusé — pour
   la connexion suivante, attendre la fenêtre de 30 s suivante (`nextTotpCode`).
   L'activation exige aussi le mot de passe (`totp-password`).
9. **L'inscription publique est fermée** : tout compte de test naît par
   `registerFreshUser(page, name?)` de `e2e/helpers.ts` (API admin + invitation),
   jamais par la page /register. L'admin de la suite est seedé par
   `e2e/global-setup.ts` (upsert idempotent, DB directe). Un compte ainsi créé
   est **déjà vérifié** — pour tester le flux « courriel non vérifié », remettre
   l'état par UPDATE direct (pattern `markEmailUnverified` dans `courriel.spec.ts`).
   **Fin de suite** : les comptes de test sont supprimés (`e2e/cleanup.ts`, appelé
   par l'arrêt de `global-setup.ts`). Un spec qui crée un compte sous une NOUVELLE
   forme d'adresse doit ajouter son motif à `GENERATED_EMAIL_PATTERNS`, sinon il
   s'accumule dans la base de dev.
10. **Cookie de session Secure vs contexte API** : l'API émet
    `__Host-hebdomiam-session` avec l'attribut Secure (obligatoire avec ce préfixe) ; le jar d'`APIRequestContext` le
    refuse sur http://127.0.0.1 (le navigateur, lui, l'accepte). Pour un appel
    API authentifié depuis un spec : `page.evaluate(fetch(...))` (cookies natifs
    du navigateur) ; côté helper : transporter le cookie explicitement
    (en-tête `cookie:` manuel, `addCookies` sans attribut Secure).
11. **Locale du navigateur** : `locale: 'fr-CA'` dans `playwright.config.ts` —
    sans elle, la détection de langue redirige le premier passage à la racine
    vers /en et tous les sélecteurs français échouent.
12. **Style calculé sous transition** : `base.css` anime `box-shadow`, `border-color`
    et `background-color` des contrôles. Un `getComputedStyle` lu juste après
    `focus()` ou un survol renvoie parfois la valeur de départ : lire par
    `await expect.poll(() => style(el, 'box-shadow')).toContain(…)`, jamais par une
    lecture unique (vécu avec le test de focus visible de `design.spec.ts`).

## Avant de lancer

- `DATABASE_URL` dans le shell (PowerShell : `$env:DATABASE_URL = ...`), DB de dev
  démarrée, puis `npm run generate` — le serveur e2e sert `.output/public`, pas les
  sources. Docker Desktop démarré (l'image `api-runtime` est reconstruite à chaque
  exécution : jamais de vieux binaire).
- Port 3000 libre ? `Get-NetTCPConnection -LocalPort 3000 -State Listen` (un
  `npm run dev` oublié). Le conteneur `hebdomiam-e2e` résiduel d'une exécution
  interrompue est supprimé automatiquement au lancement suivant.
- Après un `page.goto()`, attendre un élément rendu avant de mesurer quoi que ce
  soit de global (`document.fonts.ready`, styles calculés) : en SPA le HTML initial
  est une coquille vide — vécu avec le test de typographie de `design.spec.ts`.

## Diagnostic d'un échec

1. Lire `test-results/<test>/error-context.md` : le snapshot yaml dit dans quel
   état l'écran était vraiment (alerte affichée ? champ rempli ? page attendue ?).
2. Relancer le test **seul** : s'il passe isolément, chercher un problème de
   charge/timing ou d'état partagé, pas un bug d'app.
   Plusieurs tests sans rapport qui dépassent tous les 60 s dans la même
   exécution = poste chargé (Argon2 dans le conteneur, deux workers, API de dev
   ou sous-agent qui tournent à côté) : `npx playwright test --last-failed
   --workers=1`, puis la suite entière en `--workers=1` (~10 min) pour un
   passage complet propre — vécu : 6 échecs à deux workers, 36/36 à un seul.
3. `npx playwright test <spec> --trace on`, puis dépouiller
   `0-trace.network` (dézippé de trace.zip) : les requêtes réellement émises et
   leurs statuts. « Submit déclenché mais aucune requête » = exception dans le
   handler ; « 200 mais mauvais rendu » = course de réponses côté client.
4. En dernier recours, instrumenter par un spec temporaire :
   `page.evaluate` (checkValidity, listeners) + `page.waitForResponse`.
5. Un échec peut révéler un vrai bug d'app (course de requêtes, exception
   avalée) : ne pas assouplir l'assertion avant d'avoir compris la cause.
