# Mission : rendre cette app Node.js hébergeable sur le serveur perso "serveur-perso"

> **Mode d'emploi (pour l'humain)**
> 1. Copier ce fichier dans le repo de l'app Node, par ex. `docs/HOSTING.md`.
> 2. Lancer Claude Code dans ce repo et dire :
>    *« Lis `docs/HOSTING.md` et exécute la mission de bout en bout. Pose-moi les questions
>    de la section 6 si tu ne peux pas y répondre en lisant le code. »*
> 3. Récupérer le rapport final (section 9) et le bundle `deploy/serveur-perso/`, puis les
>    donner au Claude Code du repo `serveur-perso` pour l'intégration côté infra.

---

## 0. Contexte : l'infrastructure cible

Tu prépares cette application pour un **serveur perso auto-hébergé** (mini PC Debian/Ubuntu à
domicile). **Tu n'as pas accès à ce serveur ni à son repo de configuration.** Ton travail se limite
à ce repo-ci : rendre l'app déployable, et produire les fichiers d'intégration que l'admin
copiera dans son repo d'infra.

Faits sur l'infra cible — ce sont des **contraintes**, pas des suggestions :

| Élément | Réalité sur le serveur |
|---|---|
| Orchestration | Un seul `docker-compose.yml` regroupant tous les services |
| Reverse proxy | **Traefik v3**, provider Docker, `exposedByDefault: false`, réseau `proxy` |
| Entrypoints | `websecure` (`:443`) et `websecure-alt` (`:8443`) — `web` (`:80`) redirige en 301 vers HTTPS |
| TLS | Certificat **wildcard** `*.<domaine>` déjà émis (DNS-01 Cloudflare). Rien à faire pour le certificat |
| DNS | Enregistrement **wildcard `*.<domaine>` déjà proxifié via Cloudflare** + DDNS. Un nouveau sous-domaine fonctionne **sans aucune action DNS** |
| Réseaux Docker | `proxy` (bridge, partagé, **fournit aussi la sortie Internet**) ; réseaux `*-internal` en `internal: true` (**aucun accès Internet**) |
| Sécurité HTTP | Middlewares fichier : geo-blocking (EU+NA), bouncer CrowdSec + WAF AppSec (Coraza/OWASP CRS), en-têtes de sécurité, rate limiting |
| IPS | CrowdSec analyse les access logs Traefik et bannit les IP au niveau du proxy |
| Données persistantes | **Tout** sous `/app-tetram/<service>/` (bind mounts, pas de volumes nommés) |
| Mises à jour | **Watchtower** met à jour les images automatiquement à 4h du matin |
| Monitoring | Netdata (métriques host + Docker) et Uptime Kuma (disponibilité + moniteurs Push) |
| Sauvegarde | `backup.sh` en cron à 3h : dumps DB + rsync vers disque externe, heartbeat Uptime Kuma |
| Ressources | Mini PC (Intel N100/N150). RAM et CPU **limités** : viser une empreinte modeste |
| Fuseau | `TZ` fourni par l'environnement (ex. `Europe/Paris`) |

---

## 1. Ce que tu dois produire

Dans **ce** repo :

- **A.** `Dockerfile` de production (multi-stage, non-root, sans dépendances de dev).
- **B.** `.dockerignore` complet.
- **C.** Un endpoint de **healthcheck** applicatif (`GET /healthz`) + une directive `HEALTHCHECK`.
- **D.** Le **contrat de configuration** : `.env.example` documenté + validation au démarrage.
- **E.** Les adaptations de code nécessaires (écoute, trust proxy, arrêt propre, logs) — voir §2.
- **F.** La **publication de l'image** sur GHCR via GitHub Actions (recommandé — voir §5).
- **G.** Un bundle d'intégration `deploy/serveur-perso/` :
  - `service.yml` — le bloc de service à coller dans le `docker-compose.yml` du serveur ;
  - `service.security.yml` — le bloc de limites de ressources (fichier d'override séparé côté infra) ;
  - `env.example` — **uniquement** les lignes à ajouter au `.env` du serveur ;
  - `README.md` — procédure de déploiement, de mise à jour, de sauvegarde/restauration et de rollback.
- **H.** Le **rapport final** (§9) dans ta réponse.

---

## 2. Règles dures (non négociables)

1. **Aucun `ports:` publié.** L'app n'est jamais exposée sur l'hôte ; Traefik l'atteint par le
   réseau Docker `proxy`. Publier un port contournerait le WAF, le geo-blocking et CrowdSec.
2. **Écouter sur `0.0.0.0`**, port lu depuis `process.env.PORT` (défaut documenté). Écouter sur
   `127.0.0.1` rend le conteneur injoignable depuis Traefik.
3. **Faire confiance au proxy.** L'app est derrière Cloudflare **puis** Traefik : l'IP client réelle
   est dans `X-Forwarded-For`, le schéma dans `X-Forwarded-Proto`. Sur Express :
   `app.set('trust proxy', true)` (sinon : rate limiting sur la mauvaise IP, cookies `secure`
   refusés, redirections en `http://`). Adapter selon le framework (Fastify : `trustProxy: true`).
4. **Non-root** dans l'image (`USER node` ou un uid dédié). Compatible avec
   `no-new-privileges:true` et, si possible, `read_only: true` + `tmpfs`.
5. **Conteneur jetable.** Aucun état dans le conteneur : tout ce qui doit survivre va dans un
   volume monté (côté serveur : `/app-tetram/<app>/...`). Uploads, SQLite, caches, sessions :
   soit dans un volume, soit externalisés (Redis/Postgres), soit assumés comme perdables.
6. **Logs sur `stdout`/`stderr`** uniquement, une ligne par événement (JSON de préférence).
   Pas de fichier de log dans le conteneur, pas de rotation applicative — c'est le job de Docker.
7. **Arrêt propre sur `SIGTERM`** en moins de 10 s (fermer le serveur HTTP, drainer les requêtes en
   cours, fermer les connexions DB). Ne pas lancer Node via un script shell qui avale les signaux ;
   utiliser la forme exec (`CMD ["node", "..."]`) ou un init (`tini`/`dumb-init`) si l'app fork.
8. **Zéro secret dans l'image et zéro secret committé.** Toute valeur sensible vient d'une variable
   d'environnement injectée par Compose depuis le `.env` du serveur.
9. **Configuration 12-factor.** Aucun chemin, domaine ou identifiant codé en dur : tout par variable
   d'environnement, avec **validation au démarrage** (échec immédiat et message explicite si une
   variable requise manque — pas de démarrage en mode dégradé silencieux).
10. **Image versionnée par tag immuable** (`vX.Y.Z` ou digest), pas seulement `latest`.
11. **Empreinte maîtrisée** : mesurer la RAM au repos et sous charge légère, et proposer des limites
    cohérentes dans `service.security.yml`.

---

## 3. Dockerfile — attentes

Gabarit de référence à adapter (framework, étape de build, chemins) :

```dockerfile
# ---------- build ----------
FROM node:22-alpine AS build
WORKDIR /app
COPY package*.json ./
RUN npm ci                                  # dev deps incluses : nécessaires au build
COPY . .
RUN npm run build                           # supprimer si pas d'étape de build

# ---------- deps de production ----------
FROM node:22-alpine AS deps
WORKDIR /app
COPY package*.json ./
RUN npm ci --omit=dev && npm cache clean --force

# ---------- runtime ----------
FROM node:22-alpine
ENV NODE_ENV=production
WORKDIR /app
COPY --from=deps  /app/node_modules ./node_modules
COPY --from=build /app/dist ./dist          # adapter aux artefacts réels
COPY package.json ./
USER node
ENV PORT=3000
EXPOSE 3000
# Healthcheck sans dépendance externe (fetch global disponible depuis Node 18)
HEALTHCHECK --interval=30s --timeout=5s --start-period=20s --retries=3 \
  CMD node -e "fetch('http://127.0.0.1:'+(process.env.PORT||3000)+'/healthz').then(r=>process.exit(r.ok?0:1)).catch(()=>process.exit(1))"
CMD ["node", "dist/server.js"]
```

Critères :

- Ordre des `COPY` optimisé pour le cache (manifestes avant le code source).
- `npm ci` (jamais `npm install`) ; `package-lock.json` présent et committé.
- Pas de compilateur ni de source dans l'image finale. Si une dépendance native l'exige
  (`sharp`, `bcrypt`, `better-sqlite3`…), compiler en étape `build` et ne copier que le résultat —
  et le documenter dans le rapport.
- Si l'app a besoin de `git`, `ffmpeg`, `openssl`… : les installer explicitement et le signaler.
- `/healthz` doit vérifier ce qui compte (DB joignable ?) tout en restant **rapide** et **sans
  authentification**, et ne doit pas être journalisé bruyamment à chaque appel.

## 4. Bundle d'intégration `deploy/serveur-perso/`

### `service.yml` — bloc à coller dans le `docker-compose.yml` du serveur

Gabarit ; remplacer `<app>`, le port et les middlewares selon les décisions de §6.

```yaml
  <app>:
    image: ghcr.io/<user>/<app>:<version>     # tag immuable
    container_name: <app>
    restart: unless-stopped
    security_opt:
      - no-new-privileges:true
    networks:
      - proxy
    environment:
      - NODE_ENV=production
      - TZ=${TZ:-Europe/Paris}
      - PORT=3000
      - APP_URL=https://<sous-domaine>.${DOMAIN}
      - <APP>_SECRET=${<APP>_SECRET}          # défini dans le .env du serveur
    volumes:
      - /app-tetram/<app>/data:/data          # uniquement si état à persister
    healthcheck:
      test: ["CMD", "node", "-e", "fetch('http://127.0.0.1:3000/healthz').then(r=>process.exit(r.ok?0:1)).catch(()=>process.exit(1))"]
      interval: 30s
      timeout: 5s
      retries: 3
      start_period: 20s
    labels:
      - "traefik.enable=true"                 # obligatoire : exposedByDefault vaut false
      - "traefik.http.routers.<app>.rule=Host(`<sous-domaine>.${DOMAIN}`)"
      - "traefik.http.routers.<app>.entrypoints=websecure,websecure-alt"
      - "traefik.http.routers.<app>.tls=true"
      - "traefik.http.routers.<app>.middlewares=geoblock-eu-na@file,crowdsec-bouncer@file,secure-headers@file,rate-limit@file"
      - "traefik.http.services.<app>.loadbalancer.server.port=3000"
      - "traefik.docker.network=proxy"
```

Notes importantes :

- **Ordre de la chaîne de middlewares** : geo-blocking d'abord (filtre le moins coûteux), puis
  CrowdSec/WAF, puis en-têtes, puis rate limit. Les noms se terminent par `@file` (définis dans
  `traefik/dynamic/middlewares.yml`). **Ne pas inventer de nouveaux noms de middleware** : soit tu
  réutilises ceux listés ici, soit tu documentes précisément dans le README ce qu'il faut ajouter.
- Middlewares réutilisables existants : `geoblock-eu-na`, `crowdsec-bouncer`, `secure-headers`,
  `secure-headers-media` (CSP plus permissive), `rate-limit` (100/min), `rate-limit-strict` (20/min),
  `rate-limit-auth` (5/min), `compress`.
- `traefik.docker.network=proxy` évite toute ambiguïté si le conteneur rejoint plusieurs réseaux.
- Si l'app a besoin d'un **endpoint d'auth durci**, prévoir un second routeur avec
  `priority=100` et `rate-limit-auth@file` sur le préfixe de chemin concerné.
- Si l'image est **construite sur le serveur** (au lieu d'être tirée de GHCR), ajouter
  `- "com.centurylinklabs.watchtower.enable=false"` : Watchtower ne sait pas mettre à jour une
  image locale.

### `service.security.yml` — limites de ressources

```yaml
services:
  <app>:
    deploy:
      resources:
        limits:
          memory: 256M          # mesurer et ajuster
          cpus: '0.5'
        reservations:
          memory: 64M
```

### `env.example`

Uniquement les lignes nouvelles, avec un commentaire par variable et la commande de génération
des secrets (ex. `openssl rand -base64 32`). Ne jamais dupliquer les variables existantes
(`DOMAIN`, `TZ`, `PUID`, `PGID`…).

### `README.md`

- Prérequis côté serveur : `sudo mkdir -p /app-tetram/<app>/data && sudo chown -R <uid>:<gid> …`
  (préciser l'uid/gid attendu par l'image).
- Déploiement : ajouter le bloc, compléter le `.env`, `docker compose up -d <app>`.
- Vérification : logs, `docker inspect --format '{{.State.Health.Status}}' <app>`, appel HTTPS.
- Mise à jour / rollback (changement de tag).
- Sauvegarde : **quoi** sauvegarder exactement et **comment** obtenir un dump cohérent
  (pour SQLite : `sqlite3 <fichier> ".backup <dest>"` ; pour Postgres : `pg_dump`), avec la
  commande de restauration. C'est ce bloc qui sera ajouté à `backup.sh`.
- Moniteur Uptime Kuma suggéré (URL HTTPS, mot-clé attendu, intervalle).

---

## 5. Livraison de l'image : GHCR (recommandé)

Publier l'image sur `ghcr.io` via GitHub Actions plutôt que de builder sur le serveur :

- le mini PC n'a pas à héberger les sources ni une toolchain de build ;
- `docker compose pull && up -d` suffit à déployer ;
- Watchtower peut assurer les mises à jour automatiques si tu suis un tag mouvant.

Le workflow doit : builder à chaque tag `v*`, pousser `ghcr.io/<user>/<app>:<version>` **et**
`:latest`, et utiliser le cache GitHub Actions. Si le repo est privé, indiquer dans le README la
procédure `docker login ghcr.io` avec un PAT `read:packages` côté serveur.

Alternative acceptable : build sur le serveur via `build: ./<app>` — dans ce cas, désactiver
Watchtower sur le service et documenter la procédure `git pull && docker compose build`.

---

## 6. Décisions à prendre (lis le code, puis demande ce qui reste)

Réponds à chacune dans le rapport final. Si le code ne permet pas de trancher, **pose la question**
à l'humain avant de figer le bundle.

1. **Sous-domaine** souhaité (`<quoi>.<domaine>`) et nom du service/conteneur.
2. **Port** d'écoute interne.
3. **En-têtes de sécurité** : `secure-headers` applique une **CSP restrictive**
   (`default-src 'self'`, `script-src 'self' 'unsafe-inline' 'unsafe-eval'`, `frame-ancestors 'self'`).
   Vérifie dans le code si l'app charge des ressources **externes** (CDN, Google Fonts, images
   tierces, appels `fetch` cross-origin, iframe embarquée, service worker). Si oui : soit auto-héberger
   les assets (préférable), soit spécifier la CSP exacte nécessaire pour qu'un middleware dédié soit
   créé côté infra. **N'omets jamais l'inventaire des origines externes.**
4. **Rate limit** : `rate-limit` (100 req/min, burst 50) convient à une app classique. Une SPA qui
   télécharge des dizaines de chunks au chargement à froid le dépasse → 429 → JS tronqué et app
   cassée. Estime le nombre de requêtes du chargement initial et recommande une valeur.
5. **Geo-blocking** : `geoblock-eu-na` n'autorise que l'Europe et l'Amérique du Nord (403 ailleurs).
   Convient sauf si l'app doit être joignable depuis ailleurs (recevoir des **webhooks** de
   prestataires hors EU/NA, usage en voyage). Le dire explicitement.
6. **Authentification** : l'app gère-t-elle sa propre auth ? Sinon, faut-il un basic auth Traefik ?
   Y a-t-il des routes publiques (webhooks, callbacks OAuth) à ne pas protéger ?
7. **Uploads / gros corps de requête** : le proxy Cloudflare **plafonne les requêtes à 100 Mo**.
   Si l'app dépasse, il faut un enregistrement DNS *non proxifié* — le signaler comme prérequis
   infra. Indiquer aussi si des timeouts longs (> 60 s par requête) sont nécessaires.
8. **WebSocket / SSE** : Traefik les gère nativement, mais le middleware `compress` **casse le SSE**
   s'il n'exclut pas `text/event-stream` (déjà exclu côté serveur — juste à savoir). Préciser les
   endpoints temps réel et leurs besoins de timeout d'inactivité.
9. **Base de données / cache** : si l'app en a besoin, suivre le motif du serveur — un service dédié
   (`<app>-postgres`, `<app>-redis`) sur un réseau `<app>-internal` en `internal: true`, avec
   `depends_on: condition: service_healthy`. **Piège** : un conteneur sur un réseau `internal: true`
   n'a **aucune sortie Internet** ; s'il doit télécharger quoi que ce soit, l'attacher aussi à `proxy`.
   Ne réutilise **jamais** le Postgres d'Immich. Et `watchtower.enable=false` sur les DB.
10. **Tâches planifiées / workers** : processus séparé, ou cron côté hôte ? Éviter les cron
    in-process qui se dupliquent si le service redémarre.
11. **Sortie mail** : si l'app envoie des emails, réutiliser les variables SMTP existantes du serveur
    (`SMTP_HOST`, `SMTP_PORT`, `SMTP_SECURITY`, `SMTP_USERNAME`, `SMTP_PASSWORD`, `SMTP_FROM`)
    plutôt que d'en introduire de nouvelles.
12. **Sauvegarde** : quels chemins/DB, et quelle commande donne un dump cohérent à chaud ?

---

## 7. Pièges déjà rencontrés sur ce serveur

À garder en tête, ils ont tous coûté un incident :

- **`traefik.enable=true` est obligatoire** (`exposedByDefault: false`). Sans ce label, aucun
  routage — et le symptôme est un 404 de Traefik, pas une erreur explicite.
- **Un conteneur invisible pour Traefik** s'il n'est pas sur le réseau `proxy`.
- **Rate limit trop bas sur une SPA** → 429 sur des chunks JS → `NS_ERROR_CORRUPTED_CONTENT`,
  page blanche difficile à diagnostiquer.
- **CSP restrictive** → assets tiers silencieusement bloqués (visible uniquement en console).
- **Réseau `internal: true` = pas de DNS/Internet** → `Temporary failure in name resolution`.
- **Watchtower à 4h** met à jour les images suivant `latest` : une app avec migrations de schéma
  doit être épinglée (`watchtower.enable=false`) pour éviter un bump majeur non supervisé.
- **`readTimeout` Traefik à 60 s par défaut** : un upload long est coupé en plein transfert.
- **Cloudflare proxifié plafonne à 100 Mo par requête.**
- **Aucun état hors volume** : un `docker compose down` détruit tout ce qui n'est pas monté.

---

## 8. Validation avant livraison

Fais tourner et rapporte les résultats :

```bash
docker build -t <app>:test .
docker image inspect <app>:test --format '{{.Config.User}} {{.Size}}'   # non-root + taille

# Démarrage avec les seules variables du contrat, en simulant l'absence de ports publiés
docker run --rm --name <app>-test -e PORT=3000 -e <...> <app>:test &
docker exec <app>-test node -e "fetch('http://127.0.0.1:3000/healthz').then(r=>console.log(r.status))"

# Arrêt propre : doit sortir bien avant les 10 s
time docker stop <app>-test

# Le conteneur démarre-t-il en lecture seule ?
docker run --rm --read-only --tmpfs /tmp -e PORT=3000 <app>:test

# Variable requise manquante -> doit échouer immédiatement avec un message clair
docker run --rm <app>:test
```

Vérifie aussi : `docker compose -f deploy/serveur-perso/service.security.yml config` est valide,
et le YAML de `service.yml` parse (indentation à 2 espaces, service au niveau d'indentation d'un
service dans `services:`).

---

## 9. Rapport final attendu

Termine par ce tableau rempli — c'est ce que l'admin transmettra au repo d'infra :

```
Service / conteneur   :
Sous-domaine          :
Image                 :            (GHCR ou build local)
Port interne          :
Volumes requis        :            (chemin hôte -> chemin conteneur, uid/gid)
Variables .env        :            (nom + rôle + secret oui/non)
Middlewares Traefik   :            (chaîne exacte, avec justification des écarts)
Origines externes     :            (liste, ou "aucune" -> CSP par défaut OK)
Requêtes au cold load :            (estimation -> rate limit recommandé)
Geo-blocking          :            (OK / à retirer, avec raison)
Services annexes      :            (DB, Redis, worker...)
Limites ressources    :            (RAM/CPU mesurés puis proposés)
Sauvegarde            :            (quoi + commande de dump + commande de restauration)
Moniteur Uptime Kuma  :            (URL, mot-clé, intervalle)
Prérequis infra       :            (DNS non proxifié, timeouts, paquets hôte, login GHCR...)
Risques / à surveiller:
```

---

## 10. Hors périmètre

- Ne modifie **pas** la configuration Traefik, CrowdSec, ni le `docker-compose.yml` du serveur :
  tu ne les as pas, tu produis des fichiers à copier.
- N'ajoute **aucun** `ports:`, ni `network_mode: host`, ni Nginx/Caddy/PM2 dans le conteneur :
  Traefik est le seul point d'entrée et Docker le seul superviseur.
- N'invente pas le nom de domaine : utilise `${DOMAIN}` dans les fichiers Compose.
- Ne committe aucun secret, ni `.env` réel.
- Ne refactore pas la logique métier de l'app : limite-toi à ce que l'hébergement exige,
  et signale séparément ce que tu aurais envie de changer.
