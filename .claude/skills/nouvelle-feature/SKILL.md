---
name: nouvelle-feature
description: Workflow complet d'une fonctionnalité d'Hebdomiam, avec deux arrêts — qualification, conception (arrêt 1), développement sans interruption, tests, vérification visuelle, tests manuels (arrêt 2), relecture, livraison. Utiliser dès que Fabrice demande une nouvelle feature, une modification de comportement ou une correction non triviale.
argument-hint: <description de la fonctionnalité ou du correctif>
---

# Développer une fonctionnalité Hebdomiam

Workflow séquentiel qui enchaîne d'autres skills ; chaque étape s'applique **en entier**.
Ne pas annoncer « fait » avant la fin de l'étape 7. Rappeler en une ligne l'étape en
cours à chaque reprise. Les règles de codage détaillées sont dans
[regles-de-codage.md](regles-de-codage.md) (même dossier).

```
0 Qualifier → 1 Conception ⏸ → 2 Développement → 3 Tests → 4 Test visuel
  → 5 Tests manuels ⏸ → 6 Relecture → 7 Livraison
```

## 0. Qualifier la demande

| Demande | Livrable attendu |
|---|---|
| Fonctionnalité | code + tests de la pyramide (étape 3) + doc |
| Correctif | test qui **échoue sans le correctif** (le vérifier : réintroduire le bug, voir le test rougir) + correctif |
| « Vérifier que c'est réglé » | preuve par le code, `git log` et une passe manuelle jetable (capture, spec temporaire) — **aucun test ajouté** |
| Fonction pure sans écran | tests unitaires + preuve de non-vacuité, pas d'e2e |

## 1. Conception

- **Demande structurante** (schéma DB, sécurité, contrat JSON, clés d'API, décision du
  PRD, plusieurs écrans) → appliquer la skill `conception`.
  ⏸ **Arrêt 1** : attendre la validation du plan et les réponses aux questions bloquantes.
- **Petite demande** : lire la section pertinente de `PRD.md`, trouver la fonctionnalité
  voisine qui sert de modèle, énoncer le plan en une phrase et avancer.
- Une demande qui contredit une décision du PRD (§4) s'arrête **toujours** pour la signaler.

## 2. Développement

Deux phases : **analyse** (questions permises), puis **exécution** (aucune question).

**Analyse**
- Confronter le plan au code actuel : écart, hypothèse fausse, cas non couvert ?
- Lister les fichiers à créer ou modifier.
- Poser **maintenant** toutes les questions restantes, numérotées, chacune avec une
  hypothèse par défaut. Si la conception vient d'être validée, ne poser que les nouvelles.
- Travailler sur la branche courante si elle porte ce sujet ; sinon demander avant d'en
  créer ou d'en changer (dossier partagé avec d'autres sessions).

**Exécution**
- Ordre : migration → repo → routes et validation → `shared/types/` → front → i18n.
- Suivre [regles-de-codage.md](regles-de-codage.md). Strict minimum API : entrée validée
  (`ValidJson`/`ValidPath`/`ValidQuery`), accès DB scopé `user_id` dans `api/src/repo/`,
  action critique journalisée, route sensible rate-limitée, erreurs = clés i18n, chaînes
  dans `fr.json` **et** `en.json`, schéma modifié → nouvelle migration SQL.
- Cargo uniquement par Docker (`npm run api:cargo -- <args>`) : aucun compilateur Rust
  sur le poste.
- Documentation des bibliothèques par `ctx7` plutôt que de mémoire.
- Ambiguïté mineure → choisir l'option la plus cohérente avec le code existant et la
  noter pour le récap. **Exceptions — s'arrêter et demander :**
  - migration qui supprime ou transforme des données existantes ;
  - action destructive ou irréversible (`reset --hard`, suppression d'un fichier que la
    session n'a pas créé, force push), ou toute action sur le serveur du home lab ;
  - découverte qui invalide la conception ou contredit le PRD.

## 3. Tests

- **Unitaires** : logique pure et validation des entrées (bornes, normalisation,
  refus) — `#[cfg(test)]` dans le module Rust ; `tests/unit/` pour le front.
- **Intégration** (`api/tests/`) : les fonctions repo contre la vraie DB
  (`TestDb::new()` = base dédiée au test), et la frontière HTTP (`tests/http.rs`).
  **Toute nouvelle ressource a son test IDOR** (Bob ne lit/modifie pas les
  données d'Alice → `null`, rien de modifié).
- **E2E** (`e2e/`) : le parcours utilisateur réel. Lire la skill `e2e-stable`
  avant d'écrire ou modifier un spec.
- Lancer les suites concernées et corriger jusqu'au vert (`npm test`, `npm run api:test`,
  `npx playwright test <spec>`).

## 4. Test visuel (Playwright)

Tout changement d'interface passe par la skill `verifier-ui` : capture d'écran
prise, **lue et inspectée** — un diff CSS qui compile ne prouve rien. Verrouiller
les propriétés mesurables par une assertion `boundingBox()` dans le spec permanent.

## 5. Tests manuels

⏸ **Arrêt 2** : rendre la main à Fabrice, sans committer.

```
## Récap
- Ce qui a été fait (puces)
- Décisions prises sans demander (puces)
- Tests : unitaires X ✅ · intégration/IDOR Y ✅ · e2e Z ✅ · non créés (raison)

## À tester manuellement
1. <écran / scénario> → résultat attendu
2. ...

## Captures
- <chemins dans le scratchpad>
```

Pour tester : `docker compose -f docker-compose.dev.yml up -d` (API :8080) puis le front
(`npm run dev`, configuration `front` de `.claude/launch.json`) sur :3000. Attendre le
« OK » de Fabrice ; s'il renvoie des retours, corriger puis revenir à cet arrêt.

## 6. Relecture

Appliquer la skill `relecture` (suites complètes, revue par sous-agent, corrections, doc).
Verdict bloquant → s'arrêter et présenter les points « à décider ».

## 7. Livraison

Appliquer la skill `livraison` (commits en français découpés par sujet, push, PR) —
chaque commit, push et PR avec l'accord de Fabrice.
