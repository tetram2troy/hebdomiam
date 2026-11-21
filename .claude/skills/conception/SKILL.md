---
name: conception
description: Phase de conception d'une fonctionnalité structurante d'Hebdomiam avant d'écrire du code — confronte la demande au PRD, propose modèle de données, contrat JSON, impacts, plan par couche et découpage en commits, et pose les questions numérotées. Utiliser quand Fabrice demande de concevoir, cadrer ou analyser une feature, ou quand une demande touche le schéma, la sécurité ou une décision du PRD. Ne modifie aucun fichier.
argument-hint: <description de la fonctionnalité ou référence PRD (ex. F5)>
---

# Concevoir une fonctionnalité Hebdomiam

Objectif : un **plan validé par Fabrice** avant toute implémentation.

**Aucun fichier du projet n'est modifié pendant cette phase** (ni code, ni PRD).

## Style

- Phrases courtes, puces. Ne pas paraphraser le PRD : le citer par section (§6, F5…).
- Expliquer chaque choix en une ligne.

## 1. Lire le besoin

- `PRD.md` : la fonctionnalité visée (§5), les **décisions structurantes (§4)**, le
  modèle (§6), l'architecture (§7), la sécurité (§8), les jalons (§11) et le journal (§12).
- Demande qui **contredit une décision du PRD** → le dire en tête de réponse ;
  c'est une question bloquante, pas un détail d'implémentation.
- Source manquante (maquette, comportement attendu) → la demander avant de continuer.
  Les maquettes vivent dans `docs/wireframes/` et `docs/design-system/`.

## 2. Explorer l'existant

- La fonctionnalité voisine la plus proche est le modèle : route (`api/src/routes/`),
  repo (`api/src/repo/`), types (`shared/types/`), page (`app/pages/`), spec e2e.
- Lister les conventions de `CLAUDE.md` concernées (validation, isolation `user_id`,
  rôles, clés d'API, sessions, CSP, journalisation).

## 3. Données et modèle

Écrire « Aucun » si rien ne change.

- Tables, colonnes, index, contraintes, `ON DELETE` (cascade depuis `users` ?).
- Quantités en `numeric`, jamais en `float`.
- Nouvelle migration `api/migrations/NNNN_description.sql` (jamais modifier une
  migration publiée). Données existantes à reprendre ?

## 4. Contrat front/back

- Routes : méthode, chemin, extracteur d'auth (`CurrentUser`, `AdminUser`,
  `ApiKeyUser` pour `/api/v1/*` seulement), extracteurs de validation.
- Forme JSON en bloc court (miroir prévu dans `shared/types/`) ; bornes de validation.
- Erreurs : clés i18n `errors.*` nouvelles.
- **Changement incompatible** du contrat (front, clés d'API, skill `importer-recette`)
  → le signaler explicitement.

## 5. Impacts sur l'existant

- Écrans, routes, specs e2e touchés ; risques de régression.
- i18n FR/EN, CSP (ressource externe ? script ?), limitation de débit (auth, IA,
  courriel), journalisation (`domaine.action`), quotas.
- Sécurité (PRD §8) : surface nouvelle, IDOR, SSRF (`SafeFetcher`), upload, énumération.

## 6. Plan technique

Par couche, une ligne de justification par élément, dans l'ordre d'implémentation :

1. Migration SQL
2. Repo (`api/src/repo/`, scopé `user_id`)
3. Routes + validation (`api/src/routes/`, `api/src/validation.rs`)
4. Types partagés (`shared/types/`)
5. Front : pages, composants, composables, `i18n/locales/fr.json` + `en.json`
6. Tests : unitaires, intégration + **IDOR**, e2e (pyramide de `nouvelle-feature`)
7. Doc : PRD (statut, journal §12), README, `CLAUDE.md`, skills

## 7. Découpage

Commits (ou PR) livrables et testables seuls, dans l'ordre :

| # | Sujet | Couches | Dépend de | Critères d'acceptation (2-3 puces) |
|---|---|---|---|---|

Une feature de jalon complète = une branche `feat/<sujet>` et une PR.

## 8. Questions

Toutes les ambiguïtés et décisions à prendre :

- numérotées ;
- séparées en **bloquantes** / **non bloquantes** ;
- chacune avec une **hypothèse par défaut**.

## Format de sortie

```
## Résumé du besoin        (3 à 5 puces)
## Écarts avec le PRD      (ou « Aucun »)
## Données et modèle
## Contrat front/back
## Impacts
## Plan technique
## Découpage
## Questions
### Bloquantes
### Non bloquantes
```

Terminer en demandant la validation du plan. Une fois validé, proposer (sans le faire
d'office) d'inscrire la décision au PRD : statut de la fonctionnalité (§5) et, si un
arbitrage a été rendu, une entrée datée au journal (§12).
