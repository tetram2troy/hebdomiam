---
name: livraison
description: Livrer un changement d'Hebdomiam — commits Conventional Commits en français découpés par sujet, push de la branche et pull request GitHub (gh), puis suivi de la CI. Utiliser quand Fabrice demande de committer, pousser, livrer ou ouvrir une PR, ou à l'étape livraison de `nouvelle-feature`. Chaque commit, push et PR demande son accord explicite.
---

# Commit, push et pull request

## 0. Préflight

- La relecture est faite et son verdict est « prêt à committer » ; sinon appliquer la
  skill `relecture` d'abord.
- **Git passe par PowerShell** sur ce poste (il gèle dans Bash) ; messages multilignes
  en here-string `@'…'@`, fermeture en colonne 0.
- Branche courante : `git branch --show-current`. Sur `main` (protégée, push direct
  interdit) → créer une branche courte `feat/<sujet>` / `fix/<sujet>` / `docs/<sujet>`.
  **Ne jamais changer de branche sans accord** : le dossier est partagé avec d'autres
  sessions qui peuvent avoir du travail en cours.
- Accord explicite de Fabrice avant chaque commit, push et création de PR — un accord
  ne vaut que pour l'action nommée.

## 1. Commits

Format **Conventional Commits, en français** :

```
type(portée): description courte

Corps : le pourquoi, les choix de sécurité, en puces si besoin.

Co-Authored-By: <ligne d'attribution fournie par la session>
```

- `type` : `feat`, `fix`, `test`, `docs`, `refactor`, `chore`, `perf`, `ci`.
- `portée` optionnelle, d'après la zone touchée : `api`, `reglages`, `recettes`,
  `planifier`, `epicerie`, `cuisine`, `admin`, `e2e`, `deps`…
- Description < 72 caractères, minuscule initiale, sans point final.
- **Découper par sujet** : `feat`, `test`, `docs` séparés quand ils se lisent seuls ; un
  bug trouvé en route = son propre commit `fix:` avec l'explication.
- Le trailer `Co-Authored-By` de Claude est **conservé** (convention du dépôt).
- `git add` des fichiers du sujet seulement, nommés un par un — jamais `git add -A` :
  pas de `.env`, de capture, de `.claude/settings.local.json`, ni de fichier d'une autre
  session. Vérifier `git status` avant et après.
- Ne jamais `--no-verify`, `--amend` un commit poussé, ni `reset --hard`.

## 2. Push

- `git push -u origin <branche>`.
- Jamais de force push sans accord explicite (et jamais sur `main`).

## 3. Pull request

`gh` n'est pas dans le PATH des sessions : `& "$env:ProgramFiles\GitHub CLI\gh.exe"`
(compte authentifié `fabriceC67`, dépôt `tetram2troy/hebdomiam`).

- Base : `main`. Titre = titre du commit principal.
- Description courte :

```markdown
## Changements
- (3 à 6 puces)

## Sécurité
- (surface nouvelle, garde-fous ajoutés — ou « Aucun impact »)

## Tests
- Unitaires / intégration / IDOR / e2e : ajoutés, existants, ou aucun (raison)
- Vérification visuelle : écrans capturés (skill `verifier-ui`)
- Manuel : validé par Fabrice

🤖 Generated with [Claude Code](https://claude.com/claude-code)
```

- Créer avec `gh pr create --base main --head <branche> --title "…" --body-file <fichier>`
  (fichier de corps dans le scratchpad, jamais dans le repo).
- Donner le lien de la PR en fin de réponse.

## 4. CI

La PR déclenche `ci.yml` (front, API, e2e, image) et `security.yml` (CodeQL, audits,
Trivy). Ne pas sonder la CI en boucle : utiliser le suivi de PR de l'app quand il est
disponible. Échec → diagnostiquer (e2e : skill `e2e-stable`), corriger dans un nouveau
commit, pousser.

Ne pas fusionner la PR soi-même sans demande de Fabrice. Une release (tag `vX.Y.Z` →
image GHCR) est une décision à part, jamais un effet de bord de la livraison.
