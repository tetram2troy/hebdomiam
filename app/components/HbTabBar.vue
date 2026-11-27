<script setup lang="ts">
// Navigation mobile et tablette (< 1024 px) : barre d'onglets basse à quatre
// destinations (docs/design-system.md §8.9). Le CTA « Planifier » vit dans
// l'accueil ; langue, déconnexion et administration vivent dans Réglages.
const localePath = useLocalePath()
const route = useRoute()
const routeBaseName = useRouteBaseName()

type Tab = 'home' | 'recipes' | 'grocery' | 'settings'

// L'onglet actif suit la section, pas seulement la page exacte : planifier et
// cuisiner partent de l'accueil, l'administration s'ouvre depuis Réglages.
const active = computed<Tab | null>(() => {
  // Les noms de route de Nuxt sont des chaînes ; le type admet aussi `symbol`.
  const name = String(routeBaseName(route) ?? '')
  if (name === 'index' || name === 'planifier' || name === 'cuisine') return 'home'
  if (name.startsWith('recettes')) return 'recipes'
  if (name === 'epicerie') return 'grocery'
  if (name === 'reglages' || name === 'admin') return 'settings'
  return null
})

function current(tab: Tab) {
  return active.value === tab ? 'page' : undefined
}
</script>

<template>
  <nav class="tabbar" :aria-label="$t('nav.main')" data-testid="tabbar">
    <NuxtLink :to="localePath('/')" class="tab" :class="{ on: active === 'home' }" :aria-current="current('home')" data-testid="tab-home">
      <span class="pill">
        <svg viewBox="0 0 24 24" aria-hidden="true">
          <path d="M3.5 10.5 12 3.5l8.5 7" />
          <path d="M5.5 9.5V20h13V9.5" />
          <path d="M10 20v-5.5h4V20" />
        </svg>
      </span>
      {{ $t('nav.home') }}
    </NuxtLink>
    <NuxtLink :to="localePath('/recettes')" class="tab" :class="{ on: active === 'recipes' }" :aria-current="current('recipes')" data-testid="tab-recipes">
      <span class="pill">
        <svg viewBox="0 0 24 24" aria-hidden="true">
          <path d="M5 4.5h12.5a1.5 1.5 0 0 1 1.5 1.5v14H7a2 2 0 0 1-2-2z" />
          <path d="M5 18a2 2 0 0 1 2-2h12" />
          <path d="M9 8.5h6M9 12h4" />
        </svg>
      </span>
      {{ $t('nav.recipes') }}
    </NuxtLink>
    <NuxtLink :to="localePath('/epicerie')" class="tab" :class="{ on: active === 'grocery' }" :aria-current="current('grocery')" data-testid="tab-grocery">
      <span class="pill">
        <svg viewBox="0 0 24 24" aria-hidden="true">
          <path d="M3 4.5h2.5L8 15.5h9.5L20 8H6.5" />
          <circle cx="9" cy="19.5" r="1.3" />
          <circle cx="16.5" cy="19.5" r="1.3" />
        </svg>
      </span>
      {{ $t('nav.grocery') }}
    </NuxtLink>
    <NuxtLink :to="localePath('/reglages')" class="tab" :class="{ on: active === 'settings' }" :aria-current="current('settings')" data-testid="tab-settings">
      <span class="pill">
        <svg viewBox="0 0 24 24" aria-hidden="true">
          <path d="M4 7.5h8M18 7.5h2" />
          <circle cx="15" cy="7.5" r="2.5" />
          <path d="M4 16.5h2M12 16.5h8" />
          <circle cx="9" cy="16.5" r="2.5" />
        </svg>
      </span>
      {{ $t('settings.title') }}
    </NuxtLink>
  </nav>
</template>

<style scoped>
/* Collée au bas de l'écran sans sortir du flux (`sticky` dans la colonne
   `.layout`) : le contenu n'est jamais masqué dessous, aucune marge de
   compensation à maintenir. */
.tabbar {
  position: sticky;
  bottom: 0;
  z-index: 10;
  display: flex;
  background: var(--md-sys-color-surface-container-lowest);
  border-top: 1px solid var(--md-sys-color-outline-variant);
  padding: var(--hb-space-2) var(--hb-space-1) calc(var(--hb-space-2) + env(safe-area-inset-bottom, 0px));
}

/* ≥ 1024 px : navigation haute (app.vue). */
@media (min-width: 1024px) {
  .tabbar {
    display: none;
  }
}

.tab {
  flex: 1 1 0;
  min-width: 0;
  min-height: var(--hb-touch-target);
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: var(--hb-space-1);
  color: var(--md-sys-color-on-surface-variant);
  font: var(--md-sys-typescale-label-medium);
  text-decoration: none;
}

.pill {
  display: grid;
  place-items: center;
  width: 56px;
  height: 28px;
  border-radius: var(--md-sys-shape-corner-full);
}

.pill svg {
  width: 22px;
  height: 22px;
  fill: none;
  stroke: currentColor;
  stroke-width: 2;
  stroke-linecap: round;
  stroke-linejoin: round;
}

.tab.on {
  color: var(--md-sys-color-on-surface);
}

.tab.on .pill {
  background: var(--md-sys-color-primary-container);
  color: var(--md-sys-color-on-primary-container);
}

@media (prefers-reduced-motion: no-preference) {
  .pill {
    transition: background-color var(--md-sys-motion-duration-short) var(--md-sys-motion-easing-standard);
  }
}
</style>
