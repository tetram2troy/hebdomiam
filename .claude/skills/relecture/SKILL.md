---
name: relecture
description: Relecture d'Hebdomiam avant commit — lint, typecheck et suites, revue du diff par un sous-agent indépendant (correction, sécurité PRD §8, règles de codage, tests IDOR, i18n), corrections, mise à jour de la doc, verdict. Utiliser avant tout commit, à l'étape relecture de `nouvelle-feature`, ou quand Fabrice demande de relire ou vérifier les changements en cours.
---

# Relire avant commit

Périmètre : la branche courante par rapport à `main` — `git diff main...HEAD` (déjà
commité) **et** `git diff` + fichiers non suivis (en cours). Git passe par **PowerShell**
sur ce poste (il gèle dans Bash).

## Style

- Phrases courtes, puces. Un constat = une ligne + `fichier:ligne`.

## 1. Lint, typecheck et suites

```powershell
npm run lint
npm run typecheck
npm test
npm run api:lint                # cargo fmt --check + clippy -D warnings, dans Docker
npm run api:test                # DB de dev démarrée (npm run api:up)
npm run generate                # les e2e servent .output/public
npm run test:e2e                # DATABASE_URL dans le shell ; port 3000 libre
```

- Docker Desktop n'est pas lancé par défaut : le démarrer avant les commandes `api:*`.
- Formatage Rust : `npm run api:fmt` ; front : `npx eslint --fix <fichiers>` sur les
  **seuls fichiers touchés par la branche**, puis vérifier `git status`.
- Relancer jusqu'à zéro erreur. Un échec e2e se diagnostique avec la skill
  `e2e-stable` avant de retoucher l'app ou d'assouplir une assertion.
- Diff purement documentaire (Markdown, skills) : les suites ne sont pas requises.

## 2. Revue du diff

Déléguer à un **sous-agent neuf** (outil Agent, type `general-purpose`) : un regard qui
n'a pas écrit le code. Lui passer en prompt :

- le contenu de [relecteur.md](relecteur.md) (même dossier) ;
- la plage à relire (`main...HEAD`, plus le travail non commité s'il y en a) ;
- l'intention de la modification en 2-3 lignes — pas tes conclusions.

Sans outil Agent : faire la revue soi-même avec la grille de `relecteur.md`, en
relisant le diff complet plutôt que de mémoire.

Vérifier chaque constat avant d'agir : le sous-agent peut se tromper avec aplomb.

## 3. Corriger

- **Corriger directement** : lint, bug évident, constat de sécurité simple, clé i18n
  manquante, test IDOR absent.
- **Ne pas corriger sans accord** : changement de conception, refactor large,
  constat incertain, écart au PRD → le lister dans « À décider » avec une proposition.
- Relancer les suites concernées après correction.

## 4. Documentation

La doc fait partie du changement :

- `PRD.md` : statut de la fonctionnalité (§5), entrée datée au journal (§12) si une
  décision a été prise ;
- `README.md` (mise en route, variables d'environnement) ;
- conventions de `CLAUDE.md` ;
- les skills elles-mêmes si une leçon nouvelle a été apprise (piège e2e → `e2e-stable`,
  règle → `nouvelle-feature/regles-de-codage.md`).

Ne pas créer de nouveau document sans demande.

## Format de sortie

```
## Relecture
- Suites : lint ✅ · typecheck ✅ · vitest ✅ · api:lint ✅ · api:test ✅ · e2e ✅ (ou ❌ + détail, ou « non lancé : raison »)
- Corrigé : (puces, fichier:ligne)
- À décider : (puces numérotées, fichier:ligne, proposition)
- Doc mise à jour : (fichiers) | Aucune
- Verdict : prêt à committer | bloquant (raison)
```

Un constat de **sécurité** non corrigé est toujours bloquant.
