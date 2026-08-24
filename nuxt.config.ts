export default defineNuxtConfig({
  modules: ['@nuxt/eslint', '@nuxtjs/i18n', 'nuxt-auth-utils', 'nuxt-security'],

  devtools: { enabled: true },

  app: {
    head: {
      // Sans favicon déclaré, chaque navigateur demande /favicon.ico et Nitro
      // rend un 404 SSR complet à chaque chargement à froid.
      link: [{ rel: 'icon', type: 'image/svg+xml', href: '/favicon.svg' }],
    },
  },

  css: [
    '~/assets/css/tokens.css',
    '~/assets/css/fonts.css',
    '~/assets/css/base.css',
  ],

  compatibilityDate: '2026-07-02',

  typescript: {
    strict: true,
  },

  runtimeConfig: {
    // Surchargé par NUXT_DATABASE_URL
    databaseUrl: '',
    session: {
      maxAge: 60 * 60 * 24 * 7, // 7 jours
      password: '', // surchargé par NUXT_SESSION_PASSWORD (min 32 caractères)
    },
  },

  i18n: {
    defaultLocale: 'fr',
    strategy: 'prefix_except_default',
    locales: [
      { code: 'fr', language: 'fr-CA', name: 'Français', file: 'fr.json' },
      { code: 'en', language: 'en-CA', name: 'English', file: 'en.json' },
    ],
  },

  security: {
    headers: {
      // La CSP est la propriété du middleware Traefik secure-headers@file du home lab
      // (PRD §8) — arbitrage à réévaluer au jalon M5.
      contentSecurityPolicy: false,
    },
    // Pas de CORS : l'API n'est consommée que par le front même origine.
    // Le défaut du module émettait Access-Control-Allow-Origin: *.
    corsHandler: false,
    // Le défaut (8 Mo par requête multipart) coupait l'import photo avant le
    // handler, qui accepte 3 images × 10 Mo (server/api/import/photo.post.ts).
    // 35 Mo = 3×10 Mo + enrobage multipart — loin du plafond Cloudflare (100 Mo).
    requestSizeLimiter: {
      maxRequestSizeInBytes: 2_000_000,
      maxUploadFileRequestInBytes: 35_000_000,
      throwError: true,
    },
    // Rate limiting global désactivé : limiteur applicatif ciblé sur les routes d'auth
    // (server/utils/rate-limit.ts) + rate-limit-auth@file côté Traefik.
    rateLimiter: false,
  },
})
