---
name: nouvelle-feature
description: Workflow séquentiel pour développer une fonctionnalité d'Hebdomiam — plan, code, tests, vérification visuelle, lint, revue de sécurité, revue de conformité aux règles de codage, doc, commits. Utiliser dès que Fabrice demande une nouvelle feature, une modification de comportement ou une correction non triviale.
---

# Développer une fonctionnalité Hebdomiam

Workflow séquentiel — chaque étape est un jalon, ne pas annoncer « fait » avant
la fin de l'étape 7. Les règles de codage détaillées sont dans
[regles-de-codage.md](regles-de-codage.md) (même dossier).

## 1. Plan

- Lire la section pertinente de `PRD.md` (décisions §4, modèle §6, sécurité §8,
  jalons §11). Si la demande contredit une décision structurante, **s'arrêter et
  le signaler** avant d'implémenter.
- Explorer l'existant : la fonctionnalité voisine la plus proche (page, route,
  repo, schéma) est le modèle à suivre — ne pas inventer un nouveau pattern.
- Produire un plan court : fichiers touchés, changement de schéma DB (oui/non),
  routes/API, écrans, tests prévus (unit/intégration/e2e), risques de sécurité.
  Demande ambiguë ou structurante → présenter le plan avant de coder ;
  sinon l'énoncer en une phrase et avancer.

## 2. Code

Suivre [regles-de-codage.md](regles-de-codage.md). Rappel du strict minimum
serveur : entrée validée Zod (`shared/schemas/`), accès DB dans un repo scopé
`userId`, action critique journalisée (`logEvent`/`logWarn`), route sensible
rate-limitée, erreurs = clés i18n, chaînes visibles dans `fr.json` **et**
`en.json`, schéma modifié → `npm run db:generate`.

## 3. Tests

- **Unitaires** (`tests/unit/`) : logique pure, nouveaux schémas Zod (bornes,
  normalisation, refus).
- **Intégration** (`tests/integration/`) : les fonctions repo contre la vraie DB.
  **Toute nouvelle ressource a son test IDOR** (Bob ne lit/modifie pas les
  données d'Alice → `null`, rien de modifié).
- **E2E** (`e2e/`) : le parcours utilisateur réel. Lire la skill `e2e-stable`
  avant d'écrire ou modifier un spec.

## 4. Test visuel (Playwright)

Tout changement d'interface passe par la skill `verifier-ui` : capture d'écran
prise, **lue et inspectée** — un diff CSS qui compile ne prouve rien. Verrouiller
les propriétés mesurables par une assertion `boundingBox()` dans le spec permanent.

## 5. Lint, typecheck et suites complètes

```powershell
npm run lint
npm run typecheck
npm test
npm run test:integration        # DB de dev démarrée
npm run build                   # les e2e servent .output
npm run test:e2e                # .env chargé dans le shell ; port 3000 libre
```

Un échec e2e n'est pas forcément une régression : diagnostiquer avec la skill
`e2e-stable` avant de retoucher l'app ou d'assouplir une assertion.

## 6. Revue de sécurité (PRD §8)

Relire le diff avec cette grille — chaque « non » se corrige avant de continuer :

- [ ] Toute entrée (body, params, query, fichier) passe par un schéma Zod ?
- [ ] Toute requête DB métier remonte à `userId` via la couche repo — et le
      test IDOR le prouve ?
- [ ] Les actions critiques sont journalisées **sans** secret, jeton, code TOTP,
      mot de passe ni contenu utilisateur (PII minimale : userId, IP) ?
- [ ] Route d'auth / coûteuse (IA, courriel) → `assertRateLimit` ?
- [ ] Messages d'erreur non énumérants (pas de « ce compte existe ») ?
- [ ] Fetch sortant → garde SSRF (`fetchExternalPage`), jamais `fetch` nu ?
- [ ] Upload → type réel vérifié (magic bytes) + ré-encodage ?
- [ ] Aucun secret en dur ; configuration par variables d'environnement ?

## 7. Revue de conformité aux règles de codage

Relire le diff contre [regles-de-codage.md](regles-de-codage.md) : nommage,
structure des composants et des repos, tokens CSS (jamais de couleur en dur),
i18n bilingue complet, `decimal` jamais `float`, imports explicites entre
`server/utils/*`, `data-testid` sur les éléments testés, commentaires en
français qui expliquent le *pourquoi*.

## 8. Documentation

Mettre à jour ce qui a changé : statut du `PRD.md`, `README.md`, conventions de
`CLAUDE.md`, et les skills elles-mêmes si une leçon nouvelle a été apprise.
La doc fait partie de la feature.

## 9. Commits

Commits conventionnels **en français**, découpés par sujet (`feat:`, `fix:`,
`test:`, `docs:` séparés), corps expliquant le *pourquoi* et les choix de
sécurité. Un bug trouvé en route = son propre commit `fix:` avec l'explication.
