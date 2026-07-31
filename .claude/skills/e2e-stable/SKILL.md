---
name: e2e-stable
description: Pièges connus de la suite Playwright d'Hebdomiam (hydratation Nuxt, rate limiting, v-model number, serveur résiduel) et méthode de diagnostic d'un échec e2e. Utiliser avant d'écrire ou modifier un spec e2e, et dès qu'un test e2e échoue.
---

# Écrire et déboguer les e2e d'Hebdomiam

La suite tourne à 2 workers contre un seul serveur (`node .output`), timeouts
60 s/test et 10 s/assertion. Chaque piège ci-dessous a réellement causé un échec.

## Pièges à l'écriture

1. **Hydratation Nuxt** : après tout `page.goto()` ou `page.reload()` suivi d'un
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
   .51 reglages · .52 securite-2fa · .53 courriel · .54 i18n · .61-.62 admin ·
   .63+ captures/diagnostics · .71 epicerie · .95 import · .98 cuisine.
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
7. Les jetons de courriel sont stockés hachés : pour tester un lien, **insérer un
   jeton connu en DB** depuis le spec (pattern dans `e2e/courriel.spec.ts`).
8. Les codes TOTP se génèrent avec `generateSync({ secret })` d'otplib, secret
   récupéré depuis l'écran d'activation (pattern dans `e2e/securite-2fa.spec.ts`).
9. **L'inscription publique est fermée** : tout compte de test naît par
   `registerFreshUser(page, name?)` de `e2e/helpers.ts` (API admin + invitation),
   jamais par la page /register. L'admin de la suite est seedé par
   `e2e/global-setup.ts` (upsert idempotent, DB directe). Un compte ainsi créé
   est **déjà vérifié** — pour tester le flux « courriel non vérifié », remettre
   l'état par UPDATE direct (pattern `markEmailUnverified` dans `courriel.spec.ts`).
10. **Cookie de session Secure vs contexte API** : le build de prod émet
    `nuxt-session` avec l'attribut Secure ; le jar d'`APIRequestContext` le
    refuse sur http://127.0.0.1 (le navigateur, lui, l'accepte). Pour un appel
    API authentifié depuis un spec : `page.evaluate(fetch(...))` (cookies natifs
    du navigateur) ; côté helper : transporter le cookie explicitement
    (en-tête `cookie:` manuel, `addCookies` sans attribut Secure).
11. **Locale du navigateur** : `locale: 'fr-CA'` dans `playwright.config.ts` —
    sans elle, la détection de langue redirige le premier passage à la racine
    vers /en et tous les sélecteurs français échouent.

## Avant de lancer

- Charger `.env` dans le shell (PowerShell) puis `npm run build` — les e2e
  servent `.output`, pas les sources.
- Port 3000 libre ? `Get-NetTCPConnection -LocalPort 3000 -State Listen` :
  un `node .output` résiduel (smoke test, session précédente) serait réutilisé
  par Playwright (`reuseExistingServer`) avec un **vieux build** — le tuer.

## Diagnostic d'un échec

1. Lire `test-results/<test>/error-context.md` : le snapshot yaml dit dans quel
   état l'écran était vraiment (alerte affichée ? champ rempli ? page attendue ?).
2. Relancer le test **seul** : s'il passe isolément, chercher un problème de
   charge/timing ou d'état partagé, pas un bug d'app.
3. `npx playwright test <spec> --trace on`, puis dépouiller
   `0-trace.network` (dézippé de trace.zip) : les requêtes réellement émises et
   leurs statuts. « Submit déclenché mais aucune requête » = exception dans le
   handler ; « 200 mais mauvais rendu » = course de réponses côté client.
4. En dernier recours, instrumenter par un spec temporaire :
   `page.evaluate` (checkValidity, listeners) + `page.waitForResponse`.
5. Un échec peut révéler un vrai bug d'app (course de requêtes, exception
   avalée) : ne pas assouplir l'assertion avant d'avoir compris la cause.
