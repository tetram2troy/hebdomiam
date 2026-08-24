# Rapport final — mission hébergement (docs/HOSTING.md §9)

À transmettre au repo d'infra de serveur-perso avec le reste de ce dossier.

```
Service / conteneur   : hebdomiam (+ hebdomiam-migrate one-shot, hebdomiam-db)
Sous-domaine          : recettes.${DOMAIN}
Image                 : ghcr.io/tetram2troy/hebdomiam:0.1.2 (GHCR, workflow release.yml
                        sur tag v* ; « latest » suit les versions stables seulement)
Port interne          : 3000 (PORT, surchargeable ; écoute 0.0.0.0)
Volumes requis        : /app-tetram/hebdomiam/db -> /var/lib/postgresql/data (uid/gid 70:70)
                        App : aucun volume — stateless, read_only + tmpfs /tmp
Variables .env        : HEBDOMIAM_POSTGRES_PASSWORD (secret), HEBDOMIAM_SESSION_PASSWORD
                        (secret, ≥ 32 car.), HEBDOMIAM_ANTHROPIC_API_KEY (secret, optionnelle).
                        Réutilisées (existantes) : DOMAIN, TZ, SMTP_HOST/PORT/SECURITY/
                        USERNAME/PASSWORD. Requises au boot par l'app (NODE_ENV=production) :
                        NUXT_DATABASE_URL, NUXT_SESSION_PASSWORD, NUXT_PUBLIC_BASE_URL,
                        SMTP_HOST — toutes fournies par service.yml.
Middlewares Traefik   : geoblock-eu-na@file,crowdsec-bouncer@file,secure-headers@file,
                        rate-limit@file (routeur principal) ; second routeur priority=100
                        sur PathPrefix(/api/auth) avec rate-limit-auth@file (brute-force).
                        Écart : secure-headers exige l'ajustement CSP ci-dessous.
Origines externes     : img-src UNIQUEMENT — photos de recettes = URL https externes
                        fournies par l'utilisateur/l'import. Il faut img-src 'self' https: data:
                        (essayer secure-headers-media@file, sinon middleware dédié — README §7).
                        Tout le reste auto-hébergé : fontes locales, aucun CDN, aucun fetch
                        cross-origin navigateur, pas d'iframe ni service worker.
Requêtes au cold load : ~25 immédiates (+ ~10 prefetch) ≈ 35 -> rate-limit@file (100/min,
                        burst 50) convient. PRÉREQUIS : sourceCriterion sur l'IP client
                        réelle (voir Prérequis infra), sinon seuil partagé par tous.
Geo-blocking          : geoblock-eu-na@file conservé (usage QC/EU ; aucun webhook entrant ;
                        en voyage hors EU/NA : Tailscale/LAN).
Services annexes      : hebdomiam-db (postgres:16-alpine, réseau hebdomiam-internal
                        internal: true, jamais le Postgres d'Immich) ; hebdomiam-migrate
                        (même image que l'app, job dédié avant chaque démarrage).
                        L'app reste sur proxy AUSSI : sortie Internet requise (API Claude,
                        SMTP, téléchargement des pages importées).
Limites ressources    : app 384M / 0.75 cpu (mesuré 185 Mo au repos sur 0.1.2, marge SSR) ;
                        db 256M / 0.5 cpu (mesuré 38 Mo) — service.security.yml.
Sauvegarde            : pg_dump -Fc via docker exec hebdomiam-db (README §5, bloc pour
                        backup.sh) ; restauration pg_restore --clean --if-exists.
Moniteur Uptime Kuma  : https://recettes.${DOMAIN}/healthz — mot-clé « ok » — 60 s
                        (tombe aussi si la DB est en panne : 503 voulu).
Prérequis infra       : 1) CSP img-src (ci-dessus). 2) forwardedHeaders.trustedIPs =
                        plages Cloudflare + sourceCriterion des rate-limit* sur l'IP
                        client réelle — sinon rate limit et CrowdSec comptent/bannissent
                        l'IP edge Cloudflare partagée (auto-DoS). 3) GHCR : docker login
                        avec PAT read:packages si le paquet est privé. 4) mkdir/chown
                        70:70 de /app-tetram/hebdomiam/db. 5) Publier le tag git v0.1.2
                        (les images ≤ 0.1.1 n'ont ni /healthz ni les gardes de config).
Risques / à surveiller: extractions IA proches du readTimeout 60 s de Traefik (README §9) ;
                        fenêtre « première inscription = admin » à fermer immédiatement
                        après le up ; migrations non réversibles (dump avant mise à jour) ;
                        Watchtower désactivé sur les trois services (tag immuable + schéma) —
                        les mises à jour sont manuelles et le restent ;
                        NUXT_IMPORT_ALLOW_PRIVATE et NUXT_TRUST_XFF : drapeaux de test,
                        jamais en production.
```
