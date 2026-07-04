<script setup lang="ts">
const { loggedIn, user, clear } = useUserSession()
const localePath = useLocalePath()

// Applique le thème choisi avant la peinture (évite l'éclair clair→sombre).
useHead({
  script: [{
    innerHTML: '(function(){try{var t=localStorage.getItem("hb-theme");if(t==="light"||t==="dark")document.documentElement.dataset.theme=t}catch(e){}})()',
    tagPosition: 'head',
  }],
})

async function logout() {
  await $fetch('/api/auth/logout', { method: 'POST' })
  await clear()
  await navigateTo(localePath('/login'))
}
</script>

<template>
  <div class="layout">
    <header class="topnav">
      <NuxtLink :to="localePath('/')" class="brand">
        <svg class="brand-mark" width="26" height="26" viewBox="0 0 34 34" fill="none" aria-hidden="true">
          <rect x="2" y="2" width="30" height="30" rx="15" fill="var(--md-sys-color-primary)" />
          <path d="M10.5 17.5 l5 5 L24.5 12.5" stroke="var(--md-sys-color-on-primary)" stroke-width="3.4" stroke-linecap="round" stroke-linejoin="round" fill="none" />
        </svg>
        {{ $t('app.title') }}
      </NuxtLink>
      <nav v-if="loggedIn" class="nav" :aria-label="$t('nav.main')">
        <NuxtLink :to="localePath('/')" class="nav-link">{{ $t('nav.home') }}</NuxtLink>
        <NuxtLink :to="localePath('/recettes')" class="nav-link">{{ $t('nav.recipes') }}</NuxtLink>
        <NuxtLink :to="localePath('/epicerie')" class="nav-link">{{ $t('nav.grocery') }}</NuxtLink>
      </nav>
      <div class="topnav-end">
        <template v-if="loggedIn">
          <NuxtLink :to="localePath('/planifier')" class="btn topnav-btn" data-testid="nav-plan">{{ $t('nav.plan') }}</NuxtLink>
          <NuxtLink :to="localePath('/reglages')" class="nav-link" data-testid="nav-settings">{{ user?.name }}</NuxtLink>
          <button type="button" class="btn-outline topnav-btn" @click="logout">{{ $t('auth.logout') }}</button>
        </template>
        <template v-else>
          <NuxtLink :to="localePath('/login')" class="nav-link">{{ $t('auth.login') }}</NuxtLink>
          <NuxtLink :to="localePath('/register')" class="nav-link">{{ $t('auth.register') }}</NuxtLink>
        </template>
      </div>
    </header>
    <main class="main">
      <NuxtPage />
    </main>
  </div>
</template>

<style>
.topnav {
  display: flex;
  align-items: center;
  gap: var(--hb-space-5);
  padding: var(--hb-space-3) var(--hb-space-4);
  background: var(--md-sys-color-surface-container-lowest);
  border-bottom: 1px solid var(--md-sys-color-outline-variant);
  flex-wrap: wrap;
}

.brand {
  display: inline-flex;
  align-items: center;
  gap: var(--hb-space-2);
  font: var(--md-sys-typescale-title-large);
  font-weight: 700;
  letter-spacing: -0.015em;
  color: var(--md-sys-color-primary);
  text-decoration: none;
}

.brand-mark {
  flex: none;
}

.nav, .topnav-end {
  display: flex;
  align-items: center;
  gap: var(--hb-space-4);
}

.topnav-end {
  margin-left: auto;
}

.nav-link {
  color: var(--md-sys-color-on-surface-variant);
  font: var(--md-sys-typescale-label-large);
  text-decoration: none;
  padding: var(--hb-space-2) 0;
}

.nav-link:hover {
  color: var(--md-sys-color-on-surface);
}

.nav-link.router-link-active {
  color: var(--md-sys-color-on-surface);
  box-shadow: inset 0 -3px 0 var(--md-sys-color-primary);
}

.topnav-btn {
  min-height: 40px;
  padding: 6px 16px;
}
</style>
