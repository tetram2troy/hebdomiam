---
name: verifier-ui
description: Vérifier visuellement un changement d'interface par capture d'écran Playwright inspectée, avant de l'annoncer comme fait. Utiliser pour tout changement de CSS, de gabarit ou de composant visible — un diff qui compile ne prouve pas le rendu.
---

# Vérifier un changement d'interface par capture

Règle : ne jamais annoncer un changement visuel « fait » sans avoir **vu** une
capture du rendu réel. Précédent : un correctif de largeur de champ visait le
mauvais conteneur — lint, typecheck et build verts, rendu inchangé.

## Procédure

1. `npm run build` — les e2e servent `.output`, pas les sources.
2. Écrire un spec temporaire `e2e/capture-tmp.spec.ts` :
   - `test.use({ extraHTTPHeaders: { 'x-forwarded-for': '203.0.113.6x' } })`
     (IP libre — voir le registre dans la skill `e2e-stable`) ;
   - inscrire un utilisateur frais, semer les données minimales par l'API
     (`page.evaluate` + `fetch('/api/…')`), naviguer jusqu'à l'écran visé ;
   - `await page.screenshot({ path: '<scratchpad>/nom-parlant.png', fullPage: true })`
     — toujours dans le scratchpad de session, jamais dans le repo.
3. Lancer le spec seul : `npx playwright test e2e/capture-tmp.spec.ts`
   (variables de `.env` chargées dans le shell).
4. **Lire le PNG avec l'outil Read et l'inspecter réellement** : le changement
   attendu est-il visible ? Rien d'autre ne s'est cassé autour (débordement,
   chevauchement, libellé absurde du genre « 1 articles ») ?
5. Si la propriété se mesure, la verrouiller dans le spec e2e **permanent** :
   assertion `boundingBox()` (largeur minimale, position sous l'élément parent…).
6. Supprimer `e2e/capture-tmp.spec.ts` — il ne se committe jamais.

## Réflexes

- Une capture par état significatif (vide / rempli / erreur), pas seulement le cas idéal.
- Si la capture révèle un défaut, corriger puis **reprendre une capture** — pas
  d'extrapolation.
