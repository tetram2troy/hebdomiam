// Front Hebdomiam : SPA générée (`nuxt generate` → .output/public), servie
// en production par le binaire Rust de l'API (api/). Plus de serveur Node :
// l'authentification, les en-têtes de sécurité et les limites de requête
// vivent dans l'API (api/src/app.rs).
const apiOrigin = process.env.API_ORIGIN ?? 'http://localhost:8080'

export default defineNuxtConfig({
  modules: ['@nuxt/eslint', '@nuxtjs/i18n'],

  ssr: false,

  devtools: { enabled: true },

  app: {
    head: {
      // Sans favicon déclaré, chaque navigateur demande /favicon.ico.
      link: [{ rel: 'icon', type: 'image/svg+xml', href: '/favicon.svg' }],
      // Thème appliqué avant la peinture — fichier externe (bloquant, minuscule)
      // et non script inline : la CSP posée par l'API n'en autorise aucun.
      script: [{ src: '/theme-init.js' }],
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

  // Développement : `npm run dev` sert le front et relaie /api vers l'API Rust
  // (service `api` de docker-compose.dev.yml). L'en-tête Host d'origine est
  // conservé : les liens de courriels pointent vers le front (:3000).
  $development: {
    nitro: {
      devProxy: {
        '/api': { target: `${apiOrigin}/api`, changeOrigin: false },
        '/healthz': { target: `${apiOrigin}/healthz`, changeOrigin: false },
      },
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
})
