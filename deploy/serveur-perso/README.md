# Hebdomiam sur serveur-perso — procédure d'intégration

Application de planification de repas (Nuxt/Nitro + PostgreSQL 16).
URL cible : **https://recettes.${DOMAIN}** — couverte par le DNS wildcard et le
certificat wildcard existants, aucune action DNS.

L'app est **stateless** (aucun fichier persistant : pas d'uploads stockés, pas
de cache disque) — seule la base PostgreSQL a un volume.

## 1. Prérequis

- **Image GHCR** : le dépôt doit être poussé sur GitHub ; le workflow
  `release.yml` publie `ghcr.io/<compte>/<repo>:X.Y.Z` (+ `latest`) à chaque
  tag `vX.Y.Z` — le « v » du tag git n'apparaît pas dans le tag d'image.
  Adapter `image:` dans `service.yml` au compte réel.
  Si le paquet GHCR est privé : `docker login ghcr.io` sur le serveur avec un
  PAT `read:packages`.
  *Alternative sans GitHub* : cloner le repo sur le serveur et remplacer
  `image:` par `build: ./hebdomiam` + cible `runtime` (le label Watchtower est
  déjà à `false`, rien d'autre à changer).
- **Répertoire de données** (postgres:16-alpine tourne en uid/gid **70:70**) :
  ```bash
  sudo mkdir -p /app-tetram/hebdomiam/db
  sudo chown -R 70:70 /app-tetram/hebdomiam/db
  ```
- Conteneur app : uid `node` (1000), aucun volume ; `read_only: true` +
  `tmpfs: /tmp` sont appliqués dans `service.yml` (validés au build).
- **Résolution de l'IP client au périmètre** : l'app lit `cf-connecting-ip`
  (posé par Cloudflare) pour son rate limiting d'auth et ses logs de sécurité.
  Côté Traefik, vérifier que `forwardedHeaders.trustedIPs` couvre les plages
  Cloudflare et que le `sourceCriterion` des middlewares `rate-limit*` repose
  sur l'IP client réelle — sinon rate limit et CrowdSec comptent/bannissent
  l'**IP edge Cloudflare**, partagée par tous les visiteurs (auto-DoS : trois
  chargements à froid simultanés suffiraient à déclencher des 429).

## 2. Déploiement initial

1. Coller les trois services de `service.yml` sous `services:` du compose du
   serveur, et le réseau sous `networks:` :
   ```yaml
     hebdomiam-internal:
       internal: true
   ```
2. Reporter le bloc de `service.security.yml` dans l'override de ressources.
3. Ajouter les lignes de `env.example` au `.env` du serveur (secrets générés).
4. Démarrer l'app — le job `hebdomiam-migrate` s'exécute automatiquement entre
   la base et l'app (`depends_on`), qui ne démarre jamais sur un schéma non migré :
   ```bash
   docker compose up -d hebdomiam
   ```

Le **premier compte inscrit devient administrateur** (l'inscription publique se
ferme ensuite : les comptes suivants se créent par invitation depuis /admin).
S'inscrire immédiatement après la mise en ligne.

## 3. Vérification

```bash
docker inspect --format '{{.State.Health.Status}}' hebdomiam    # healthy
docker logs hebdomiam --tail 20                                  # JSON une ligne/événement
curl -s https://recettes.${DOMAIN}/healthz                       # {"status":"ok"}
```

Le healthcheck (`/healthz`, alias `/api/health`) vérifie que la base répond
(`SELECT 1`) : `503 db-unreachable` si la DB est en panne. Une variable requise
manquante (`NUXT_DATABASE_URL`, `NUXT_SESSION_PASSWORD`, `NUXT_PUBLIC_BASE_URL`,
`SMTP_HOST`) fait échouer le démarrage immédiatement avec un message
`[config] …` explicite — `service.yml` les fournit toutes.

## 4. Mise à jour / rollback

Tag immuable + migrations de schéma ⇒ **jamais de Watchtower** (label déjà posé).

```bash
# Mise à jour : changer le tag dans le compose (ex. 0.2.0) — sur les DEUX
# services hebdomiam et hebdomiam-migrate — puis
docker compose pull hebdomiam hebdomiam-migrate
docker compose up -d hebdomiam    # migre puis redémarre l'app

# Rollback applicatif : remettre le tag précédent (les deux services) puis up -d.
# ⚠ Les migrations ne sont pas réversibles automatiquement : si la version
# fautive a migré le schéma, restaurer d'abord le dump (voir §5).
```

## 5. Sauvegarde / restauration (bloc pour backup.sh)

Tout l'état vit dans PostgreSQL. Dump cohérent à chaud :

```bash
docker exec hebdomiam-db pg_dump -U hebdomiam -Fc hebdomiam \
  > /sauvegardes/hebdomiam-$(date +%F).dump
```

Restauration (base arrêtée côté app pour éviter les écritures) :

```bash
docker compose stop hebdomiam
docker exec -i hebdomiam-db pg_restore -U hebdomiam -d hebdomiam --clean --if-exists \
  < /sauvegardes/hebdomiam-YYYY-MM-DD.dump
docker compose start hebdomiam
```

Le répertoire `/app-tetram/hebdomiam/db` est aussi couvert par le rsync
global, mais seul le dump garantit une restauration cohérente.

## 6. Moniteur Uptime Kuma

- URL : `https://recettes.${DOMAIN}/healthz`
- Mot-clé attendu : `ok`
- Intervalle : 60 s
- Le moniteur tombe aussi si la base est en panne (503) — c'est voulu.

## 7. En-têtes de sécurité — un écart CSP à traiter

`secure-headers@file` applique `default-src 'self'` : or les **photos de
recettes sont des URL externes** fournies par l'utilisateur (`<img src>` vers
n'importe quel site de recettes). Avec la CSP par défaut, ces images seront
bloquées silencieusement (visible seulement en console navigateur).

Deux options côté infra, par ordre de préférence :

1. Essayer `secure-headers-media@file` (CSP plus permissive existante) sur le
   routeur principal — vérifier qu'elle autorise bien `img-src https:`.
2. Sinon, créer un middleware dédié `hebdomiam-headers` identique à
   `secure-headers` avec pour seule différence :
   `img-src 'self' https: data:`.

Tout le reste est auto-hébergé (fontes en `/fonts/*.woff2`, aucun CDN, aucun
`fetch` cross-origin côté navigateur, pas d'iframe, pas de service worker) —
la CSP restrictive convient pour `script-src`, `style-src`, `connect-src`.

## 8. Choix validés (référence)

| Sujet | Décision |
|---|---|
| Rate limit | `rate-limit@file` (100/min) suffit : ~35 requêtes au chargement à froid (53 chunks au total, jamais tous chargés d'un coup), puis cache navigateur. |
| Geo-blocking | `geoblock-eu-na@file` conservé — décision PRD : en voyage hors EU/NA, accès via Tailscale/LAN. Aucun webhook entrant. |
| Auth | Gérée par l'app (sessions scellées, Argon2id, 2FA TOTP) + routeur `rate-limit-auth@file` sur `/api/auth`. Pas de basic auth Traefik. Routes publiques : `/healthz` (alias `/api/health`), `/api/auth/*` (invitation, reset — déjà rate-limitées). |
| Corps de requête | Import photo ≤ 10 Mo par image (max 3) ; plafond applicatif 35 Mo par requête multipart, 2 Mo en JSON — loin du plafond Cloudflare (100 Mo). Pas de DNS non proxifié nécessaire. |
| Timeouts | Extractions IA : réponse en 15-45 s typiquement, sous le readTimeout. À surveiller (voir §9). |
| WebSocket / SSE | Aucun. |
| Tâches planifiées | Aucune in-process. Sauvegarde = cron hôte existant (§5). |
| SMTP | Relais partagé réutilisé, `SMTP_FROM=hebdomiam@${DOMAIN}`. |

## 9. Risques / à surveiller

- **Import par URL/photo avec repli IA** : si une extraction dépasse ~60 s, le
  proxy peut couper la réponse — l'utilisateur verra une erreur d'import,
  jamais de corruption. Si ça arrive souvent : allonger le timeout du routeur
  ou passer l'extraction en asynchrone (V2).
- **Première inscription** : la fenêtre « instance vierge » est ouverte entre
  `up -d` et ta première inscription — la faire immédiatement (le rate limit
  auth 5/min et le geoblock réduisent déjà l'exposition).
- **CSP** : tant que l'écart §7 n'est pas traité, les photos de recettes
  externes ne s'afficheront pas (placeholder illustré à la place — pas de
  casse fonctionnelle).
- **Drapeaux de test — jamais en production** : `NUXT_IMPORT_ALLOW_PRIVATE`
  (débranche la garde SSRF de l'import URL) et `NUXT_TRUST_XFF` (fait confiance
  à `X-Forwarded-For`, usurpable). Aucun des deux ne figure dans `service.yml` ;
  ne jamais les y ajouter.
