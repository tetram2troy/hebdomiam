# Design system Hebdomiam — direction « Primeur »

> Version 1.0 — 2026-07-02 · Direction validée : **B — Primeur** (planche comparative
> du 2026-07-02). Structure conforme au standard **Material Design 3** de Google
> (m3.material.io) : tokens `ref` → `sys`, rôles de couleur, type scale, state layers.
> Les valeurs sont propres à Hebdomiam ; MD3 fournit l'architecture, pas l'apparence.
>
> Tokens CSS : [`docs/tokens.css`](./tokens.css) (à terme `app/assets/css/tokens.css`).
> Wireframes : [`docs/wireframes/`](./wireframes/).

---

## 1. Intention visuelle

**La fraîcheur d'un étal de marché le samedi matin — franc, vitaminé, efficace, une
main sur le panier.**

- **Matériaux de référence :** cagettes de bois, papier de boucherie, sacs
  réutilisables, autocollants de fruits, signalétique de marché public.
- **Ce que ça refuse :** les pastels mous, les dégradés SaaS, le gris bleuté froid,
  l'esthétique « site de recettes générique » (photo pleine page + serif précieuse).
- **Comment ça se traduit :** aplats sans ombre (ou presque), contours épais (2 px)
  sur l'interactif, pastilles franches, neutres **teintés de vert** (jamais de gris
  pur), une seule couleur d'action (basilic), l'énergie (mandarine) réservée aux
  accents non textuels.

---

## 2. Couleur

### 2.1 Rôles (MD3 `md.sys.color.*`)

Le code applicatif ne consomme **que** les rôles. Les palettes tonales (`md.ref.*`)
servent à dériver de nouveaux rôles si besoin.

| Rôle | Clair | Sombre | Usage Hebdomiam |
|---|---|---|---|
| `primary` | `#1E7A3D` basilic | `#52C273` | La seule couleur d'action : « Planifier ma semaine », ✓ Valider, cases cochées |
| `on-primary` | `#FFFFFF` | `#06301A` | Texte/icône sur primary |
| `primary-container` | `#CDEFD2` | `#145A2D` | Fond de sélection, item de nav actif |
| `on-primary-container` | `#0C3D1E` | `#CDEFD2` | Texte sur le container |
| `secondary` | `#4E6B57` sauge | `#A9C4AE` | Soutien discret : icônes secondaires, compteurs |
| `secondary-container` | `#DFF0D9` | `#33473A` | **Chips de tags et filtres** |
| `on-secondary-container` | `#17301F` | `#DFF0D9` | Texte des chips |
| `tertiary` | `#A6431C` mandarine sombre | `#FF9C73` | Accent énergie **textuel** (lien « Échanger », badge) |
| `tertiary-container` | `#FFDBCB` | `#7A3517` | Badge « brouillon extrait », pastille import |
| `error` | `#B3382C` tomate | `#FFB4A6` | Erreurs de formulaire, suppression |
| `error-container` | `#FFDAD4` | `#8C2F22` | Bandeaux d'erreur |
| `surface` | `#F7FAF4` | `#101812` | Fond de page |
| `on-surface` | `#16221A` | `#E0EADC` | Texte principal |
| `on-surface-variant` | `#40513F` | `#C0CFBE` | Texte secondaire (méta, légendes) |
| `surface-container-lowest` | `#FFFFFF` | `#0B120D` | Cartes posées sur le fond |
| `surface-container-low → highest` | `#F1F6EC → #DFE8D9` | `#171F18 → #303B31` | Hiérarchie de surfaces sans ombre |
| `outline` | `#6F7F70` | `#8A9A8A` | Contours de champs, séparateurs forts |
| `outline-variant` | `#C0CFBE` | `#40513F` | Séparateurs discrets |
| `inverse-surface` | `#2B342C` | `#E0EADC` | **Toasts/snackbars** |
| `hb-citron` (custom) | `#F2C53D` | `#F5CE55` | Pastille « nouveau », étoile — jamais porteuse de texte |

### 2.2 Règles d'usage

1. **Une seule couleur d'action.** Tout ce qui est cliquable-principal est basilic ;
   si deux éléments basilic se disputent un écran, l'un des deux est de trop.
2. **Mandarine = énergie, pas alerte.** Le rouge d'erreur est `error` (tomate),
   jamais la mandarine, et réciproquement.
3. **Citron jamais textuel** (contraste insuffisant) : pastilles, icônes
   décoratives, surlignage d'illustration uniquement.
4. **Contrastes vérifiés (WCAG AA)** : `primary/on-primary` 5,0:1 ;
   `on-surface/surface` 13,9:1 ; `on-surface-variant/surface` 7,1:1 ;
   `on-secondary-container/secondary-container` 9,3:1. Les paires sombres
   équivalentes tiennent ≥ 4,5:1. Toute nouvelle paire passe par un vérificateur
   avant d'entrer dans ce fichier.

### 2.3 Thèmes

- Clair par défaut ; sombre via `[data-theme="dark"]` (bascule dans Réglages) ou
  `prefers-color-scheme` quand l'utilisateur laisse « Système » (défaut).
- Les deux thèmes sont conçus **à parité** : chaque écran est validé dans les deux.
- Le mode cuisine et le mode magasin suivent le thème courant.

---

## 3. Typographie

Deux familles libres (SIL OFL), **auto-hébergées** (subset latin, WOFF2, ~120 ko
au total — aucun CDN, cohérent avec l'hébergement home lab) :

| Famille | Graisses | Rôle |
|---|---|---|
| **Bricolage Grotesque** | 500, 700 | `display`, `headline`, `title-large` — titres, chiffres forts |
| **Figtree** | 400, 600 | `title-medium/small`, `body`, `label` — texte courant, boutons, chips |

Type scale (MD3, rem — base 16 px) — définie dans `tokens.css`
(`--md-sys-typescale-*`) :

| Token | Style | Usage type |
|---|---|---|
| `display-large` | Bricolage 700 · 56/64 | Chiffre de portions en mode cuisine |
| `display-small` | Bricolage 700 · 36/44 | Accueil : « Semaine du 6 juillet » |
| `headline-large/medium/small` | Bricolage 700 · 32/28/24 | Titres de page et de section |
| `title-large` | Bricolage 500 · 22 | Titre de carte recette, de dialogue |
| `title-medium/small` | Figtree 600 · 16/14 | Titres d'item, rayons d'épicerie |
| `body-large/medium/small` | Figtree 400 · 16/14/12 | Texte courant, ingrédients, méta |
| `label-large/medium/small` | Figtree 600 · 14/12/11 | Boutons, chips, onglets, légendes |
| `hb-kitchen-step` (custom) | Figtree 400 · 22/33 | Étapes du mode cuisine (lisibles à distance de bras) |

Règles : texte courant ≤ 65 ch (`--hb-reading-max`) ; chiffres alignés en colonnes en
`font-variant-numeric: tabular-nums` (quantités d'épicerie, minuteries) ; jamais de
graisse simulée (les 4 fichiers de fonte couvrent tous les besoins) ; les libellés
doivent survivre à ±30 % de longueur (FR ↔ EN) — pas de largeur fixe sur les boutons.

---

## 4. Forme, élévation, espacement

### Forme (`md.sys.shape.corner.*`)
| Token | Valeur | Composants |
|---|---|---|
| `extra-small` | 4 px | Tooltips, surlignage de champ douteux (brouillon) |
| `small` | 8 px | Champs de formulaire |
| `medium` | 12 px | Items de liste, vignettes, cases à cocher |
| `large` | 16 px | Cartes recette et suggestion |
| `extra-large` | 28 px | Dialogues, feuilles de bas d'écran |
| `full` | pastille | **Boutons, chips, badges, stepper** |

### Élévation
Primeur est plat : la hiérarchie vient des `surface-containers` et des **contours de
2 px** (`--hb-sys-border-interactive`) sur l'interactif. Trois niveaux d'ombre
seulement (MD3 en définit 6 ; les niveaux 4–5 sont non utilisés) :
`level1` cartes au repos · `level2` éléments collants (barre de rayon, barre
d'actions) · `level3` dialogues et menus.

### Espacement
Grille de 4 px : `--hb-space-1…16` (4, 8, 12, 16, 20, 24, 32, 40, 48, 64).
Gouttières de grille : 16 px mobile, 24 px desktop. Marges de page : 16 / 24 / 32 px
selon le point de rupture.

### Points de rupture
- `< 768 px` : 1 colonne, navigation par barre d'onglets basse.
- `768–1023 px` : 2 colonnes (grilles de cartes), nav basse conservée.
- `≥ 1024 px` (`--hb-breakpoint-lg`) : navigation haute, grilles 3–4 colonnes,
  planification en 2 volets.

---

## 5. États (MD3 state layers)

Calque de la couleur de contenu posé sur le composant :

| État | Opacité du calque | Note |
|---|---|---|
| hover | 8 % | Desktop seulement |
| focus | 10 % | + **anneau de focus** : 2 px `primary`, décollé de 2 px (`--hb-sys-focus-ring`) — jamais supprimé |
| pressed | 10 % | + retour haptique là où l'API le permet (cochage épicerie) |
| dragged | 16 % | Réordonner les étapes de recette |
| disabled | contenu 38 %, conteneur 12 % | Jamais de gris figé : dérivé de la couleur du composant |
| erreur (champ) | contour 2 px `error` + message `body-small` sous le champ | Le message nomme la correction (« Au moins 12 caractères »), pas la faute |

---

## 6. Iconographie et placeholders

- **Icônes :** [Material Symbols](https://fonts.google.com/icons) *Rounded*, graisse
  500, auto-hébergées (sous-ensemble des ~40 icônes utilisées, pas la fonte entière).
  Taille par défaut 24 px, 28 px en modes magasin/cuisine.
- **Placeholders de recette (décision validée) :** beaucoup de recettes n'auront pas
  de photo. Chaque recette sans image reçoit un placeholder illustré **dérivé de son
  premier tag** : fond `secondary-container`, pictogramme culinaire au trait 2 px
  (casserole, bol, feuille, poisson, flamme…) en `on-secondary-container`.
  Déterministe (même recette → même placeholder), jamais aléatoire au rendu.
- **Photos réelles :** ratio 3:2 dans les cartes, recadrage centré, coin `large`.

---

## 7. Marque

- **Wordmark :** « Hebdomiam » en Bricolage Grotesque 700, interlettrage −1,5 %,
  en `primary` sur fond clair, en `#96E3AB` sur fond sombre.
- **Picto « coche-feuille » :** cercle basilic plein, coche blanche épaisse
  (le geste central de l'app : valider une suggestion, cocher un article), feuille
  mandarine en apex. Décliné : favicon 32 px, icône PWA 512 px (fond `#1E7A3D`,
  coche + feuille), monogramme « H » en secours texte.
- **Ton rédactionnel :** québécois assumé (« souper », « épicerie », « plats »),
  tutoiement, phrases courtes orientées action (« Planifier ma semaine », jamais
  « Génération du plan hebdomadaire »).

---

## 8. Inventaire des composants

Chaque composant liste ses variantes et ses états obligatoires
(repos / hover / focus / pressed / disabled / erreur le cas échéant), tous deux thèmes.
Rendu de référence : planche « Composants » du projet Claude Design.

### 8.1 Boutons
| Variante | Anatomie | Usage |
|---|---|---|
| **Rempli** (filled) | pastille, fond `primary`, `label-large` | Une action principale par écran |
| **Contour** (outlined) | pastille, contour 2 px `outline`, texte `on-surface` | Actions secondaires (↻ Échanger) |
| **Texte** (text) | pastille invisible, texte `primary` | Actions tertiaires, liens d'action |
| **Tonal** | fond `secondary-container` | Actions groupées (filtres appliqués) |
| **Destructif** | rempli ou texte en `error` | Supprimer recette/compte (avec confirmation) |
| **Icône** | 48 px min, pastille | ↻, ✕, ⋯ |
- Hauteurs : 40 px desktop, 48 px mobile, **56 px** modes magasin/cuisine.
- Un libellé de bouton = un verbe (« Valider », « Échanger », « Générer la liste »).

### 8.2 Champs de formulaire
- Contour 2 px `outline` (repos) → `primary` (focus) → `error` (erreur), coin `small`,
  étiquette **toujours visible au-dessus** (jamais placeholder-seulement), aide et
  erreur sous le champ en `body-small`.
- Variantes : texte, multiligne (étapes), nombre avec stepper intégré, sélection
  (unité, rayon), recherche (icône loupe + effacement), téléversement (zone pointillée).
- Groupe ingrédient : `[quantité][unité][nom]` sur une ligne (desktop) ; empilé mobile.

### 8.3 Carte recette
- Anatomie : image ou placeholder illustré (3:2) · titre `title-large` (2 lignes max,
  ellipse) · méta (`body-medium`, `on-surface-variant`) : temps total, portions de
  base · chips de tags (2 max + « +n ») · badge ❄ « se congèle » (icône + info-bulle).
- Variantes : grille (liste de recettes), rangée compacte (résultats de remplacement),
  mini (récap de plan validé).
- États : repos (`level1`), hover (`level2` + translation 2 px), squelette.

### 8.4 Carte suggestion (planification) — le composant signature
- Carte recette **+ barre d'actions** : `✓ Valider` (rempli, pleine largeur relative),
  `↻ Échanger` (contour), `⋯ Remplacer` (texte, ouvre la recherche dans la collection).
- États du cycle : **proposée** (contour `outline-variant`) → **validée** (contour
  2 px `primary`, coin supérieur badgé ✓, actions remplacées par « Annuler ») →
  **en cours d'échange** (contenu en squelette, actions désactivées).
- L'échange anime un glissement horizontal (`motion-duration-medium`,
  désactivé si `prefers-reduced-motion`).

### 8.5 Item de liste d'épicerie (mode magasin)
- Hauteur ≥ **56 px**, pleine largeur tapable (pas seulement la case).
- Anatomie : case 28 px (coin `medium`) · libellé `title-medium` · quantité
  `body-large` `tabular-nums` alignée à droite.
- États : à acheter → **coché** (case remplie `primary`, texte barré à 38 %,
  l'item glisse dans la section « Dans le panier » en bas) ; édition (appui long :
  corriger rayon/quantité) ; article libre (icône ✎ discrète).
- Regroupement par rayon : en-tête de section collant (`title-small` majuscules,
  compteur d'articles restants).

### 8.6 Stepper de quantité
- `− [valeur] +` : boutons 48 px pastille contour 2 px, valeur `headline-small`
  `tabular-nums`, saisie directe possible au toucher de la valeur.
- Utilisé : nb de plats, nb de personnes (planification), portions (fiche recette).
- Bornes annoncées (aria) et boutons désactivés aux limites (1–9).

### 8.7 Tags et filtres
- Chip tag : `secondary-container`, `label-large`, pastille ; suppression ✕ en édition.
- Chip filtre : contour 2 px, état sélectionné = `secondary-container` + coche.
- Rangée horizontale défilante sur mobile (avec dégradé d'indication), enveloppée
  sur desktop.

### 8.8 Toasts / snackbars
- Fond `inverse-surface`, texte `inverse-on-surface`, coin `medium`, `level3` ;
  au-dessus de la barre d'onglets sur mobile.
- Variantes : info (« Recette enregistrée »), avec action (« Article coché — Annuler »),
  erreur (bordure gauche 4 px `error`).
- Auto-fermeture 5 s, jamais pour les erreurs bloquantes (bandeau dans la page).

### 8.9 Navigation
- **Desktop (≥ 1024 px) :** barre haute — wordmark · Accueil · Recettes · Épicerie ·
  [Planifier ma semaine] (bouton rempli, seul élément coloré) · avatar → Réglages.
  Item actif : soulignement 3 px `primary`.
- **Mobile :** barre d'onglets basse 4 destinations — Accueil, Recettes, Épicerie,
  Réglages — icône + `label-medium`, item actif en `primary-container` (pastille).
  Le CTA « Planifier » vit dans la page Accueil, pas dans la barre.
- **Modes plein écran** (magasin, cuisine) : la barre s'efface ; en-tête minimal
  avec ← retour et titre.

### 8.10 États vides
- Illustration au trait (même langage que les placeholders) + titre `headline-small`
  + phrase d'aide + **une** action.
- Prévus : aucune recette (→ « Ajouter une recette » + entrées d'import), aucune
  semaine planifiée (→ « Planifier ma semaine »), recherche sans résultat
  (→ effacer les filtres), liste d'épicerie vide.

### 8.11 Squelettes de chargement
- Blocs `surface-container-high` animés en balayage 1,5 s (statiques si
  `prefers-reduced-motion`), formes = celles du contenu réel (carte, rangée d'item).
- Prévus : grille de recettes, carte suggestion en échange, liste d'épicerie,
  brouillon d'extraction en cours (avec libellé d'étape : « Lecture de la page… »).

---

## 9. Accessibilité (WCAG AA)

- Contrastes : voir §2.2 ; vérification systématique des nouvelles paires.
- Cibles tactiles : ≥ 48 px partout, ≥ 56 px en modes magasin/cuisine (une main,
  panier dans l'autre).
- Focus visible : anneau `--hb-sys-focus-ring` sur **tout** l'interactif ;
  ordre de tabulation = ordre visuel ; pièges à focus interdits (dialogues gérés).
- Formulaires : `<label>` explicites, erreurs liées par `aria-describedby`,
  messages d'erreur = clés i18n traduites côté client.
- Cochage d'épicerie et validation de suggestion annoncés (`aria-live="polite"`).
- Wake Lock (mode cuisine) : demandé après geste explicite, état affiché, libérable.
- `prefers-reduced-motion` respecté sur toute animation (voir tokens de mouvement).

---

## 10. Décisions design à trancher plus tard

1. **Icônes définitives** : Material Symbols Rounded proposé — valider le rendu à
   l'usage face aux pictos culinaires au trait des placeholders (cohérence).
2. **Bibliothèque de placeholders** : dessiner les ~10 pictogrammes culinaires
   (mapping tag → picto) ; en attendant, picto générique casserole.
3. **Photographie** : quand des photos existent, style de recadrage et traitement
   (aucun filtre vs légère chaleur) — à trancher sur cas réels au jalon M1.
4. **Composant date/semaine** : représentation de « Semaine du 6 juillet »
   (sélecteur de semaine pour replanifier une semaine passée ?) — jalon M2.
5. **Densité desktop de la grille de recettes** : 3 vs 4 colonnes à ≥ 1280 px —
   décider avec de vraies données (longueur des titres réels).
6. **Toast vs bandeau pour les erreurs d'import** (URL inaccessible, extraction
   partielle) — tester les deux au jalon M4.
7. **Icône PWA / favicon final** : décliner le picto coche-feuille en pixel-perfect
   16/32 px (V1.5, avec la PWA).
8. **Mode cuisine, très grands écrans** : affichage 2 recettes côte à côte sur
   tablette paysage ? — observer l'usage réel d'abord.
