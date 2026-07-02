# Prompt design — à donner à Claude pour le design system et les wireframes

> Copie tout le bloc ci-dessous dans une session Claude dédiée au design
> (idéalement lancée dans ce repo pour qu'il puisse lire PRD.md).

---

Tu es designer produit senior. Je veux que tu crées le **design system** et les
**wireframes** d'Hebdomiam, une application web de planification de repas
hebdomadaire. Lis d'abord `PRD.md` à la racine du repo (si tu n'y as pas accès,
demande-le-moi) — en résumé :

**Le produit.** Un utilisateur seul (parfois un couple/famille) cuisine une fois par
semaine, la fin de semaine, 3 à 4 recettes en plusieurs portions pour couvrir ses
dîners et soupers. Le vendredi, l'app lui propose des recettes (rotation intelligente
qui évite ce qu'il a cuisiné récemment), il les valide ou les échange une à une, puis
l'app génère la liste d'épicerie consolidée, regroupée par rayon, qu'il coche en
magasin sur son téléphone. Le samedi, un « mode cuisine » affiche les recettes avec
les quantités mises à l'échelle. Les recettes se saisissent à la main ou s'importent
par URL ou photo (l'extraction arrive en brouillon à relire).

**Le rituel à honorer** (c'est LE parcours qui doit être fluide) :
vendredi 17 h → « Planifier ma semaine » → choisir nb de plats (défaut 4) et nb de
personnes (défaut 1) → valider/échanger chaque suggestion → liste d'épicerie →
samedi en magasin (mobile, une main, panier dans l'autre) → samedi après-midi en
cuisine (mains sales, écran à distance).

## Livrables attendus

1. **Design system** (fichier `docs/design-system.md` + tokens CSS dans un fichier
   d'exemple) :
   - Palette (mode clair ET sombre), typographie, échelle d'espacement, rayons,
     ombres, états (hover/focus/disabled/erreur) — en variables CSS prêtes pour Nuxt.
   - Inventaire des composants nécessaires avec leurs variantes : boutons, champs de
     formulaire, carte recette, carte suggestion (avec actions valider/échanger),
     item de liste d'épicerie cochable, stepper de quantité, tags/filtres, toasts,
     navigation (desktop + mobile), états vides et squelettes de chargement.
   - Ton visuel : chaleureux et appétissant sans être un site de recettes générique ;
     éviter l'esthétique « dashboard SaaS » froide. L'app s'appelle Hebdomiam —
     propose aussi une piste de logo/marque simple (typographique acceptable).
2. **Wireframes** de chaque page (basse/moyenne fidélité, HTML statique autonome dans
   `docs/wireframes/` — un fichier par page, sans framework, juste pour valider
   structure et hiérarchie) :
   - Accueil / tableau de bord (état : semaine planifiée vs rien de prévu)
   - Flux « Planifier ma semaine » (paramètres → suggestions → validation) — c'est
     l'écran le plus important, soigne les actions valider ✓ / échanger ↻ / remplacer
   - Liste des recettes (recherche, filtres par tags) + fiche recette
   - Formulaire recette (saisie manuelle + points d'entrée import URL/photo + écran
     de relecture du brouillon extrait)
   - Liste d'épicerie en **mode magasin mobile** (gros items cochables, cochés vers
     le bas, regroupement par rayon, ajout d'article libre)
   - Mode cuisine (étapes lisibles de loin, cases à cocher par étape, bouton
     « Cuisinée ✓ »)
   - Connexion / inscription, et réglages du compte (langue FR/EN, unités
     métrique/impérial)

## Contraintes

- **Mobile-first** pour l'épicerie et le mode cuisine ; le desktop compte surtout
  pour la planification et la saisie de recettes. Prévois les deux points de rupture.
- Interface **bilingue FR/EN** : prévois des libellés qui survivent à ±30 % de
  longueur de texte ; le français (québécois : « souper », « épicerie ») est la
  langue de référence des wireframes.
- **Accessibilité WCAG AA** : contrastes, cibles tactiles ≥ 44 px (crucial en
  magasin), focus visibles, formulaires étiquetés.
- La stack est Nuxt + CSS vanilla/variables (pas de Tailwind imposé, pas de librairie
  UI décidée) — tes tokens doivent être framework-agnostiques.
- Un seul utilisateur par compte en V1 ; ne dessine pas de fonctionnalités sociales.
- Ne réinvente pas la portée : pas de calendrier repas-par-repas, pas de nutrition.

## Méthode

Commence par me poser tes questions (max 5), puis propose 2 directions visuelles
contrastées (moodboard décrit + palette) et attends mon choix avant de produire
l'ensemble. Termine par une liste des décisions design à trancher plus tard.

---
