# PRD — Hebdomiam

Application de planification de repas hebdomadaire.

> Version : 0.2 — 2026-07-02
> Statut : décisions structurantes validées ; prêt pour le jalon M0

---

## 1. Vision

Une application web qui simplifie le rituel du week-end : le vendredi ou le samedi,
l'utilisateur reçoit une proposition de 3 ou 4 recettes pour la semaine, l'ajuste selon
ses envies, obtient la liste d'épicerie consolidée, fait ses courses, puis cuisine ses
plats en lot (*batch cooking*) pour toute la semaine.

**Le problème résolu :** planifier les repas de la semaine demande aujourd'hui de
fouiller ses recettes, calculer les quantités, et rédiger une liste d'épicerie à la main
— chaque semaine. L'app automatise ce cycle en 5 minutes.

**Ce que l'app n'est PAS :**
- Pas un calendrier « 1 repas = 1 recette » : on planifie N recettes de plusieurs
  portions qui couvrent la semaine.
- Pas un réseau social de recettes (pas de fil public, pas de commentaires) — au moins en V1.
- Pas un compteur de calories / app de nutrition.

---

## 2. Nom du projet

**Hebdomiam** (hebdo + miam) — choisi le 2026-07-02.
Autres candidats écartés : Popote, Mijote, Tambouille, Semainier, Batchy.

---

## 3. Personas et contexte d'usage

### Persona principal — « Fabrice »
- Vit seul, cuisine **une fois par semaine** (vendredi soir ou samedi).
- Prépare **3 à 4 recettes** en plusieurs portions qui couvrent ses repas de la semaine.
- Veut décider vite : valider/échanger les suggestions en 2 minutes, pas composer un menu de zéro.
- Consulte la liste d'épicerie **sur son téléphone, en magasin**.

### Persona secondaire — « invité(e) occasionnel(le) »
- Autre utilisateur avec son propre compte, ses propres recettes et plans.
- Peut planifier pour plus d'une personne (couple, famille, réception).

### Cycle d'usage type (le « rituel du vendredi »)
1. **Vendredi 17 h** — Ouvre l'app → « Planifier ma semaine ».
2. Choisit les paramètres : **4 plats différents**, **1 personne**, ~14 portions au total.
3. L'app propose 4 recettes (rotation intelligente). Il en échange une (« pas envie de curry »).
4. Il valide → la **liste d'épicerie consolidée** est générée.
5. **Samedi matin** — En magasin, il coche les articles au fur et à mesure sur son téléphone.
6. **Samedi après-midi** — Mode cuisine : les recettes s'affichent avec les quantités
   ajustées au nombre de portions choisi. Il marque chaque recette « cuisinée »
   (ce qui alimente l'historique pour la rotation).

---

## 4. Décisions structurantes ✅ Validées (2026-07-02)

| # | Décision | Choix validé |
|---|---|---|
| D1 | Extraction URL | **Parsing schema.org/Recipe** gratuit, repli IA si absent. Coût uniquement à la création de recette : acceptable. |
| D1b | Extraction photo | **API Claude (vision) par défaut**, derrière une interface de fournisseur interchangeable. L'option auto-hébergée (Ollama) est écartée pour l'instant : le home lab est un mini-PC classe N100 dont l'iGPU Quick Sync ne sert qu'au transcodage vidéo — l'inférence serait CPU-only avec un petit modèle quantifié, trop lente et limitée pour la vision, en concurrence RAM avec Immich/Jellyfin. `immich-ml` n'est pas réutilisable non plus (API interne : CLIP/visages) — voir §7.4. |
| D2 | Multi-utilisateurs | **Comptes privés** en V1 ; le schéma de données prévoit les « foyers » partagés en V2. |
| D3 | Suggestions | **Rotation intelligente** en V1 ; « envies du moment » par IA en V2. |
| D4 | Hébergement | **Home lab**, derrière le **Traefik existant** (TLS et protections déjà en place). On applique quand même le niveau de sécurité « exposé publiquement » dans l'application elle-même (défense en profondeur). |
| D5 | Langues | **Bilingue FR/EN dès la V1** (i18n dès le départ, pas de rattrapage). |
| D6 | Unités | **Métrique + impérial/tasses** supportés dès la V1 (saisie, affichage, agrégation). |
| D7 | Repas couverts | Dîners + soupers seulement pour l'instant (paramétrable plus tard). |
| D8 | Import en masse | Reporté en **V2**. |
| D9 | Coûts IA | À minimiser ; coûts ponctuels à la création de recettes acceptables. Aucun coût récurrent hors création. |

---

## 5. Fonctionnalités

### 5.1 MVP (V1)

#### F1 — Comptes et authentification
- Inscription par courriel + mot de passe (hachage **Argon2id**).
- Connexion par session scellée (cookie httpOnly, secure, sameSite).
- Vérification du courriel à l'inscription ; réinitialisation de mot de passe par courriel.
- Isolation stricte des données : toute requête est filtrée par `userId` côté serveur.

#### F2 — Gestion des recettes (CRUD)
- Champs : titre, description, photo, **nombre de portions de base**, temps de préparation,
  temps de cuisson, instructions (étapes ordonnées), tags/catégories (ex. « pâtes »,
  « asiatique », « végé », « mijoteuse »), notes personnelles.
- **Ingrédients structurés** : chaque ingrédient = quantité + unité + nom
  (ex. `400 | g | tomates en dés`). Indispensable pour la mise à l'échelle et
  l'agrégation de la liste d'épicerie.
- Recherche et filtrage par titre, tag, temps de préparation.
- Une recette indique si elle **se congèle / se conserve bien** (pertinent pour le batch cooking).

#### F3 — Import de recettes par URL
- L'utilisateur colle une URL → le serveur récupère la page et extrait la recette :
  1. **Parsing des données structurées** `schema.org/Recipe` (JSON-LD ou microdata),
     présentes sur la grande majorité des sites de recettes. Gratuit et instantané.
  2. **Repli IA** : si absent ou incomplet, le HTML nettoyé est envoyé au fournisseur
     d'extraction configuré (§7.4) qui retourne la recette dans le schéma structuré de l'app.
- L'extraction arrive en **brouillon pré-rempli** que l'utilisateur relit et corrige
  avant sauvegarde (l'extraction n'écrit jamais directement en base).
- L'URL source est conservée sur la recette (attribution).

#### F4 — Import de recettes par photo
- Upload d'une ou plusieurs photos (livre de cuisine, fiche manuscrite, capture d'écran).
- Envoi au **fournisseur d'extraction configuré** (voir §7.4 : auto-hébergé via Ollama
  vision, ou API Claude) → recette structurée → même flux de brouillon que F3.
- Limites : taille max par image (ex. 10 Mo), formats JPEG/PNG/WebP/HEIC.

#### F5 — Planification hebdomadaire
- Paramètres de session de planification :
  - **Nombre de plats différents** (défaut : 4)
  - **Nombre de personnes** (défaut : 1)
  - **Nombre de jours à couvrir** (défaut : 7 — ex. 5 si on mange parfois à l'extérieur)
  - Repas couverts (défaut : dîners + soupers, ajustable — décision D7)
  → portions totales = jours × repas × personnes ; l'app en déduit le **nombre de
  portions par recette** et met les quantités à l'échelle.
  Chaque paramètre a une **valeur par défaut modifiable dans les réglages**.
- L'app propose N recettes via la **rotation intelligente** :
  - exclut les recettes cuisinées dans les X dernières semaines (configurable, défaut 3) ;
  - maximise la variété de tags dans la même semaine (pas deux plats de pâtes) ;
  - priorise les recettes jamais faites ou pas faites depuis longtemps.
- Pour chaque proposition : **valider ✓**, **échanger ↻** (nouvelle suggestion),
  ou **remplacer manuellement** (chercher dans sa collection).
- Le plan validé est daté (semaine du…) et historisé.

#### F6 — Liste d'épicerie
- Générée à la validation du plan : **agrégation des ingrédients** de toutes les
  recettes, quantités mises à l'échelle et additionnées quand l'unité est compatible
  (2 recettes avec 200 g + 150 g de riz → « riz : 350 g »).
- Normalisation d'unités couvrant **métrique et impérial/tasses** (D6) : g/kg, ml/L,
  oz/lb, tasse/c. à soupe/c. à thé. Les unités de même dimension (masse, volume) sont
  converties et additionnées ; l'affichage respecte la **préférence de l'utilisateur**
  (métrique ou impérial). En cas d'unités incompatibles (« 2 tomates » + « 200 g de
  tomates »), les deux lignes sont conservées.
- Regroupement par **rayon d'épicerie** (fruits et légumes, boucherie, produits secs,
  laitiers, surgelés…), affecté automatiquement avec correction manuelle possible.
- Ajout d'articles libres (« papier essuie-tout ») à la liste.
- **Mode magasin (mobile)** : gros items cochables, les articles cochés passent en bas.

#### F7 — Mode cuisine (le samedi)
- Vue « plan de la semaine » avec les recettes à cuisiner et quantités ajustées.
- Affichage recette adapté à la cuisine : étapes lisibles, écran qui ne se verrouille pas
  (Wake Lock API), cases à cocher par étape.
- Bouton « Cuisinée ✓ » par recette → alimente l'**historique** (utilisé par la rotation).

### 5.2 V1.5
- **PWA** : installable sur le téléphone, liste d'épicerie consultable hors-ligne en magasin.
- Export/partage de la liste d'épicerie (texte brut, lien).
- Duplication et versionnage simple de recettes (« ma variante »).
- Import par URL : file d'attente et import en lot.

### 5.3 V2
- **Suggestions par envies (IA)** : « propose-moi quelque chose de léger et asiatique »
  → filtrage/scoring de la collection par le fournisseur IA configuré.
- **Foyers partagés** : groupe d'utilisateurs avec recettes et planification communes
  (le modèle de données V1 le prévoit, voir §6).
- **Import en masse** : migration de recettes existantes (fichiers, listes d'URLs, autre app).
- Garde-manger : inventaire de base (« j'ai déjà du riz ») déduit de la liste d'épicerie.
- Contraintes pratiques dans la rotation : temps total de cuisine du week-end, saisonnalité.
- Couverture des déjeuners/collations (V1 : dîners + soupers seulement, décision D7).

---

## 6. Modèle de données (entités principales)

```
User          id, email, passwordHash, name, emailVerifiedAt, createdAt,
              locale(fr|en), unitSystem(metric|imperial),
              defaultDishCount(4), defaultPeopleCount(1), defaultDaysCount(7),
              rotationWeeks(3)
              (V2 : householdId nullable → table Household)

Recipe        id, userId (FK), title, description, imageUrl, baseServings,
              prepMinutes, cookMinutes, sourceUrl?, freezable(bool),
              notes, createdAt, updatedAt, deletedAt (soft delete)

RecipeStep    id, recipeId (FK), position, text

RecipeIngredient  id, recipeId (FK), position, quantity(decimal), unit(enum|null),
                  name, aisle(enum, déduit)

Tag           id, userId (FK), name          — n-n avec Recipe via RecipeTag

MealPlan      id, userId (FK), weekStartDate, peopleCount, dishCount, daysCount,
              totalServings, status(draft|validated|cooked), createdAt

MealPlanRecipe    id, mealPlanId (FK), recipeId (FK), targetServings,
                  status(suggested|accepted|swapped|cooked), cookedAt?

GroceryList   id, mealPlanId (FK 1-1), createdAt
GroceryItem   id, groceryListId (FK), label, quantity?, unit?, aisle,
              checked(bool), isManual(bool), sourceIngredientIds(json)

CookingHistory    — vue dérivée de MealPlanRecipe (status=cooked) ;
                  alimente la rotation intelligente
```

Principes :
- **Toutes les tables métier portent `userId`** (ou y remontent) — l'isolation
  multi-utilisateur est vérifiée dans une couche d'accès unique, jamais dans chaque handler.
- Quantités en `decimal`, jamais en `float`.
- Unités : enum normalisé couvrant métrique et impérial (`g, kg, ml, l, oz, lb, cup,
  tbsp, tsp, unit, pinch…`) + libellé libre en secours ; table de conversion par
  dimension (masse, volume) pour l'agrégation et l'affichage selon `unitSystem`.

---

## 7. Architecture technique

### 7.1 Stack

| Couche | Choix | Justification |
|---|---|---|
| Framework | **Nuxt (dernière version stable)** — front + back (routes serveur Nitro) | Exigence du projet ; full-stack TypeScript unifié |
| Langage | TypeScript strict partout | Sécurité de types front↔back (schémas partagés) |
| Base de données | **PostgreSQL 16+** | Robuste, transactions, enums, decimal |
| ORM | **Drizzle ORM** (+ drizzle-kit pour les migrations) | Léger, SQL-proche, excellent en TypeScript ; alternative : Prisma |
| Validation | **Zod** — chaque route serveur valide son entrée | Schémas partagés client/serveur |
| Auth | **nuxt-auth-utils** (sessions scellées) + Argon2id | Simple, sans dépendance SaaS |
| UI | Nuxt UI ou Tailwind CSS | Rapidité, responsive mobile-first |
| i18n | **@nuxtjs/i18n** — FR/EN dès la V1 (D5) | Toutes les chaînes en fichiers de traduction dès le premier écran ; le français est la locale de référence |
| IA / extraction | **API Claude par défaut** (photos + repli URL), interface de fournisseur interchangeable (voir §7.4) | Coût uniquement à la création de recettes (D9) ; le matériel du home lab exclut l'auto-hébergement (D1b) |
| Courriels | Nodemailer via le **relais SMTP partagé du home lab** (Brevo, 300 mails/j gratuits) | Variables `SMTP_*` déjà en place (Vaultwarden, Netdata…) ; `SMTP_FROM` dédié `hebdomiam@${DOMAIN}` |
| Upload images | Stockage volume Docker en V1 (S3-compatible en option) | Simplicité |

### 7.2 Docker (home lab, Traefik existant)

- **Dockerfile multi-stage** : build Node → image finale minimale (`node:22-slim` ou
  distroless), exécution en **utilisateur non-root**, `HEALTHCHECK` inclus.
- **docker-compose.yml** (pas de reverse proxy dans ce compose — on s'appuie sur le
  **Traefik existant** du home lab et ses conventions) :
  - `app` — Nuxt (Nitro, port interne 3000), rattaché au réseau externe **`proxy`**
    (celui de Traefik) + au réseau interne app↔db ; aucun port publié sur l'hôte :

    ```yaml
    networks:
      - proxy
      - hebdomiam-internal
    labels:
      - "traefik.enable=true"
      - "traefik.http.routers.hebdomiam.rule=Host(`hebdomiam.${DOMAIN}`)"
      - "traefik.http.routers.hebdomiam.entrypoints=websecure,websecure-alt"
      - "traefik.http.routers.hebdomiam.tls=true"
      - "traefik.http.routers.hebdomiam.middlewares=geoblock-eu-na@file,crowdsec-bouncer@file,secure-headers@file,rate-limit@file"
      - "traefik.http.services.hebdomiam.loadbalancer.server.port=3000"
    ```
  - **Routeur prioritaire supplémentaire** scopé sur les routes d'authentification
    (`/api/auth`) avec `rate-limit-auth@file` (pattern `immich-auth` du home lab) —
    protection brute-force au périmètre, en plus du rate limiting applicatif.
  - `db` — PostgreSQL avec volume persistant, sur `hebdomiam-internal` déclaré
    `internal: true` (pattern `immich-internal`) : ni réseau `proxy`, ni port hôte,
    ni accès Internet.
  - Le DNS et le certificat sont déjà couverts : le DDNS proxifie `*.${DOMAIN}` et le
    wildcard TLS s'applique — aucun enregistrement à créer pour un nouveau sous-domaine.
- `docker-compose.dev.yml` séparé pour le développement (hot reload, port DB local,
  pas de dépendance à Traefik).
- Secrets uniquement via variables d'environnement / fichier `.env` non versionné
  (`.env.example` versionné, `DOMAIN=example.com` en exemple) ; jamais de secret dans l'image.
- **Courriels** : réutilisation du relais SMTP partagé du home lab
  (`SMTP_HOST/PORT/SECURITY/USERNAME/PASSWORD`, Brevo) avec `SMTP_FROM=hebdomiam@${DOMAIN}`.

### 7.3 Environnements
- **dev** : compose de dev, seed de données de démonstration.
- **CI** : base PostgreSQL éphémère (service GitHub Actions).
- **prod (home lab)** : compose de prod ; migrations exécutées au déploiement (job dédié, pas au boot).

### 7.4 Fournisseurs d'extraction (URL et photo)

L'extraction est isolée derrière une **interface unique** (`ExtractionProvider`) avec un
contrat commun : entrée (HTML nettoyé ou image) → sortie (recette au schéma Zod de
l'app). Le fournisseur actif est choisi par configuration (`.env`), et un ordre de
repli peut être défini.

| Fournisseur | Usage | Coût | Remarques |
|---|---|---|---|
| `schema-org` | URLs — parsing JSON-LD/microdata `schema.org/Recipe` | Gratuit | Toujours essayé en premier pour les URLs ; couvre la majorité des sites de recettes |
| `claude` | **Défaut pour les photos** + repli URL — API Claude (vision) | Quelques cents **par extraction seulement** (D9 : acceptable) | Qualité maximale ; clé API côté serveur uniquement, quota par utilisateur |
| `ollama` | Prévu par l'interface, **non retenu pour l'instant** | Gratuit (matériel) | Le mini-PC N100 du home lab n'a pas de GPU exploitable pour l'inférence (iGPU Quick Sync = transcodage vidéo seulement) : CPU-only avec petit modèle quantifié (1–4B GGUF Q4), en concurrence RAM avec Immich/Jellyfin — trop lent et limité pour la vision. À réévaluer si le matériel évolue. |

**Au sujet d'`immich-ml` (déjà présent sur le home lab)** : non réutilisable.
Son API (port 3003) est interne à Immich et ne fait que des embeddings CLIP et de la
reconnaissance faciale — pas d'extraction structurée. Les versions récentes d'Immich
offrent bien un OCR (`GET /assets/{id}/ocr` sur l'API du serveur Immich), mais il
faudrait téléverser chaque photo de recette dans la photothèque Immich (pollution de la
bibliothèque, traitement asynchrone) et l'OCR ne retourne que du **texte brut** — il
faudrait de toute façon un LLM derrière pour le structurer en recette. L'architecture
en fournisseurs garde la porte ouverte à un moteur auto-hébergé le jour où le matériel
le permettra, sans toucher au reste de l'app.

---

## 8. Sécurité

Objectif : niveau « exposé publiquement sur Internet » (décision D4). Le Traefik du
home lab fournit déjà TLS et des protections périmétriques ; l'application applique
néanmoins **ses propres défenses** (défense en profondeur — elle doit rester sûre même
si un middleware Traefik saute ou si elle est déployée ailleurs un jour).

### Authentification et sessions
- Argon2id pour les mots de passe ; politique de longueur (≥ 12 caractères) sans règles arbitraires.
- Cookies de session : `httpOnly`, `secure`, `sameSite=lax`, expiration + rotation à la connexion.
- **Rate limiting** sur login/inscription/reset (par IP et par compte) + délai progressif.
- Verrouillage temporaire après échecs répétés ; messages d'erreur non énumérants
  (« courriel ou mot de passe invalide »).
- 2FA TOTP en option (V1.5).

### Application (OWASP Top 10)
- Validation Zod systématique de **toute** entrée (body, query, params) côté serveur.
- Contrôle d'accès centralisé : chaque ressource vérifiée contre `session.userId`
  (protection IDOR) — testé automatiquement.
- Module **nuxt-security** : CSP stricte, HSTS, X-Content-Type-Options, frame-ancestors,
  limites de taille de requête. ⚠️ Le middleware Traefik `secure-headers@file` du home
  lab applique déjà sa propre CSP stricte : **une seule couche doit être propriétaire
  de chaque en-tête** (à arbitrer dès M0, valider en M5) pour éviter des en-têtes
  contradictoires qui casseraient le front.
- Protection CSRF sur les mutations (token ou vérification d'origine).
- ORM paramétré (pas de SQL concaténé) ; échappement automatique côté rendu (XSS).

### Spécifique aux fonctionnalités
- **Import URL = risque SSRF** : résolution DNS contrôlée, blocage des IP privées/loopback/
  link-local et des redirections vers celles-ci, allowlist de schémas (https), timeout
  et taille de réponse max.
- **Upload photo** : vérification du type réel (magic bytes), taille max, ré-encodage
  de l'image (supprime les métadonnées/EXIF et neutralise les payloads), stockage hors racine web.
- **Appels IA** : la clé API (fournisseur `claude`) ne transite jamais côté client ;
  quotas par utilisateur (protection contre l'abus de coût) ; le contenu extrait —
  qu'il vienne d'Ollama ou de Claude — est traité comme non fiable (validation Zod).

### Infrastructure et chaîne d'approvisionnement
- Conteneurs non-root, système de fichiers en lecture seule quand possible, réseau
  interne pour la DB, utilisateur PostgreSQL aux privilèges minimaux.
- **Dependabot** (mises à jour de dépendances) + `npm audit` en CI.
- **Scan des images Docker (Trivy)** et **analyse statique (CodeQL)** en CI.
- Sauvegardes automatiques de la DB (pg_dump quotidien, rétention 30 jours) et procédure de restauration testée.
- Journalisation des événements de sécurité (connexions, échecs, resets) sans données sensibles.

---

## 9. Tests et qualité

| Niveau | Outil | Portée |
|---|---|---|
| Unitaires | **Vitest** | Logique métier : mise à l'échelle des portions, agrégation/normalisation d'unités, rotation intelligente, parsing schema.org |
| Intégration | Vitest + base éphémère | Routes API Nitro : auth, CRUD, isolation entre utilisateurs (tests IDOR), génération de liste |
| E2E | **Playwright** | Parcours critiques : inscription → créer recette → planifier → liste d'épicerie → cocher en mode magasin |
| Statique | ESLint + Prettier + `tsc --noEmit` (strict) | Tout le code |

Cibles : logique métier critique (portions, agrégation, rotation) ≈ 100 % ; global ≥ 80 %.
L'extraction IA est testée avec des réponses enregistrées (fixtures), pas d'appel API réel en CI.

## 10. Git et CI/CD (GitHub)

- **GitHub Flow** : branches courtes → PR → revue → merge sur `main` ; `main` protégée
  (CI verte obligatoire, pas de push direct).
- **Conventional Commits** (`feat:`, `fix:`…) — changelog généré.
- **GitHub Actions** :
  1. `ci.yml` — lint, typecheck, tests unitaires/intégration, E2E, build.
  2. `security.yml` — CodeQL, npm audit, scan Trivy de l'image.
  3. `release.yml` — build et push de l'image Docker (GHCR) taguée sur release.
- Dependabot activé (npm + actions + docker).

---

## 11. Jalons

| Jalon | Contenu | Critère de sortie |
|---|---|---|
| **M0 — Socle** | Repo, Nuxt + Docker + Postgres + Drizzle, CI complète, auth | Un utilisateur s'inscrit et se connecte via l'app dockerisée ; CI verte |
| **M1 — Recettes** | CRUD recettes + ingrédients structurés + tags + recherche | Saisie manuelle complète utilisable |
| **M2 — Planification** | Rotation intelligente, validation/échange, historique | Le rituel du vendredi fonctionne de bout en bout |
| **M3 — Épicerie** | Agrégation, rayons, mode magasin mobile | Liste utilisable en magasin sur téléphone |
| **M4 — Import** | URL (schema.org + repli) puis photo via l'API Claude (D1b), validées sur un banc d'essai de photos réelles (livre, manuscrit, capture d'écran) | 8 sites de recettes populaires sur 10 importés sans retouche majeure ; extraction photo fiable sur le banc d'essai |
| **M5 — Durcissement** | 2FA, PWA, sauvegardes, revue de sécurité complète | Prêt pour exposition publique via le Traefik du home lab |

---

## 12. Journal des décisions et questions restantes

**Résolu le 2026-07-02** (voir tableau complet en §4) : nom **Hebdomiam** ; extraction
schema.org pour les URLs, API Claude pour les photos (interface interchangeable) ;
comptes privés V1 ; rotation intelligente V1 ; hébergement home lab derrière Traefik
existant ; pas d'OAuth pour l'instant ; bilingue FR/EN dès la V1 ; unités métriques +
impériales ; dîners + soupers seulement ; import en masse en V2 ; coûts IA limités à la
création de recettes.

**Complété le 2026-07-02 — infrastructure home lab :**
- Matériel : mini-PC classe N100, iGPU Quick Sync réservé au transcodage (pas
  d'inférence LLM) → extraction photo via API Claude, Ollama écarté (D1b).
- Traefik : réseau externe `proxy`, entrypoints `websecure` (443) + `websecure-alt`
  (8443), chaîne standard `geoblock-eu-na` → `crowdsec-bouncer` → `secure-headers` →
  `rate-limit` (tous `@file`), + routeur `rate-limit-auth@file` sur les routes d'auth ;
  DB isolée sur un réseau `internal: true`.
- SMTP : relais partagé Brevo réutilisé (`SMTP_*` du `.env`), `SMTP_FROM` dédié.
- Domaine : `hebdomiam.${DOMAIN}` — DNS wildcard (DDNS) et certificat wildcard couvrent
  automatiquement le nouveau sous-domaine, rien à créer.

**Questions restantes (non bloquantes) :**
1. Sous-domaine final : `hebdomiam.${DOMAIN}` (retenu par défaut) ou plus court `miam.${DOMAIN}` ?
2. Propriété de la CSP : Traefik `secure-headers@file` ou l'app (nuxt-security) — à
   arbitrer quand le premier écran existera (voir §8).
3. Geoblock EU/NA : en voyage hors EU/NA, l'app ne sera accessible que via
   Tailscale/LAN (exemptés). Accepté tel quel, ou prévoir une exemption comme Vaultwarden ?
