---
name: nouvelle-feature
description: Workflow complet pour développer une nouvelle fonctionnalité d'Hebdomiam — de la lecture du PRD aux commits conventionnels, avec la pyramide de tests et les conventions du projet. Utiliser dès que Fabrice demande une nouvelle feature, une modification de comportement ou une correction non triviale.
---

# Développer une fonctionnalité Hebdomiam

Suivre ces étapes dans l'ordre. Ne jamais annoncer « fait » avant l'étape 6.

## 1. Comprendre avant d'écrire

- Lire la section pertinente de `PRD.md` (décisions §4, modèle §6, sécurité §8, jalons §11).
  Si la demande contredit une décision structurante, le signaler avant d'implémenter.
- Explorer l'existant : la fonctionnalité voisine la plus proche (page, route, repo,
  schéma) est le modèle à suivre. Ne pas inventer un nouveau pattern s'il en existe un.

## 2. Implémenter selon les conventions

Chaque nouvelle surface serveur coche ces cases :

- [ ] Entrée validée par Zod (`readValidatedBody` / `getValidatedRouterParams`),
      schéma dans `shared/schemas/` (partagé via `#shared`).
- [ ] Accès DB dans une fonction de `server/utils/*-repo.ts` **scopée sur `userId`**
      (chaîne de jointures jusqu'à `users`, retourne `null` si l'objet n'appartient
      pas à l'utilisateur — jamais de vérification dans le handler).
- [ ] Action critique (mutation, événement de sécurité) journalisée :
      `logEvent`/`logWarn` (`server/utils/logger.ts`), nom `domaine.action`,
      jamais de secret ni de contenu utilisateur, PII minimale (userId, IP via `clientIp`).
- [ ] Route sensible (auth, coût IA, envoi de courriel) : `assertRateLimit`.
- [ ] Erreurs API = clés i18n (`errors.*`) traduites côté client.
- [ ] Toute chaîne visible : clé dans `i18n/locales/fr.json` **et** `en.json`.
- [ ] Quantités en `decimal` Drizzle ; schéma modifié → `npm run db:generate`.
- [ ] Dans `server/utils/`, imports **explicites** entre utils (l'auto-import Nitro
      ne joue pas dans Vitest — un import manquant ne casse qu'aux tests).

## 3. Tester à chaque étage

- **Unitaires** (`tests/unit/`) : logique pure et nouveaux schémas Zod (bornes,
  normalisation, refus).
- **Intégration** (`tests/integration/`) : les fonctions repo contre la vraie DB.
  **Toute nouvelle ressource a son test IDOR** (Bob ne lit/modifie pas les données
  d'Alice → `null`, rien de modifié).
- **E2E** (`e2e/`) : le parcours utilisateur réel. Lire la skill `e2e-stable`
  avant d'écrire ou modifier un spec — les pièges du projet y sont recensés.

## 4. Vérifier visuellement

Tout changement d'interface passe par la skill `verifier-ui` : capture d'écran
Playwright **lue et inspectée** avant d'annoncer le résultat. Un diff CSS qui
compile ne prouve rien.

## 5. Valider la suite complète

```powershell
npm run lint
npm run typecheck
npm test
npm run test:integration        # DB de dev démarrée
npm run build                   # les e2e servent .output
npm run test:e2e                # charger les variables de .env dans le shell d'abord
```

Avant les e2e : vérifier qu'aucun `node .output` résiduel n'écoute sur le port 3000
(`Get-NetTCPConnection -LocalPort 3000`) — Playwright réutiliserait un vieux build.
Un échec e2e n'est pas forcément une régression de code : diagnostiquer (skill
`e2e-stable`) avant de retoucher l'app.

## 6. Documenter et committer

- Mettre à jour ce qui a changé : statut du `PRD.md`, `README.md`, conventions de
  `CLAUDE.md`. La doc fait partie de la feature.
- Commits conventionnels **en français**, découpés par sujet (`feat:`, `fix:`,
  `test:`, `docs:` séparés), corps expliquant le *pourquoi* et les choix de
  sécurité. Un bug trouvé en route = son propre commit `fix:` avec l'explication.
