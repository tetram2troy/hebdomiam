<script setup lang="ts">
const { loggedIn, user, clear } = useUserSession()
const localePath = useLocalePath()

async function logout() {
  await $fetch('/api/auth/logout', { method: 'POST' })
  await clear()
  await navigateTo(localePath('/login'))
}
</script>

<template>
  <div class="layout">
    <header class="header">
      <NuxtLink :to="localePath('/')" class="brand">{{ $t('app.title') }}</NuxtLink>
      <nav class="nav">
        <template v-if="loggedIn">
          <span>{{ user?.name }}</span>
          <button type="button" @click="logout">{{ $t('auth.logout') }}</button>
        </template>
        <template v-else>
          <NuxtLink :to="localePath('/login')">{{ $t('auth.login') }}</NuxtLink>
          <NuxtLink :to="localePath('/register')">{{ $t('auth.register') }}</NuxtLink>
        </template>
      </nav>
    </header>
    <main class="main">
      <NuxtPage />
    </main>
  </div>
</template>

<style>
/* Style volontairement minimal : le design system arrive après M0. */
* {
  box-sizing: border-box;
}

body {
  margin: 0;
  font-family: system-ui, sans-serif;
  color: #1a1a1a;
}

.layout {
  max-width: 40rem;
  margin: 0 auto;
  padding: 1rem;
}

.header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 1rem;
  padding-block: 0.5rem;
  border-bottom: 1px solid #ddd;
}

.brand {
  font-weight: 700;
  text-decoration: none;
  color: inherit;
}

.nav {
  display: flex;
  align-items: center;
  gap: 0.75rem;
}

.main {
  padding-block: 1.5rem;
}

form {
  display: grid;
  gap: 0.75rem;
  max-width: 22rem;
}

label {
  display: grid;
  gap: 0.25rem;
  font-size: 0.9rem;
}

input {
  padding: 0.5rem;
  font: inherit;
}

button {
  padding: 0.5rem 1rem;
  font: inherit;
  cursor: pointer;
}

.error {
  color: #b00020;
}

.hint {
  font-size: 0.8rem;
  color: #555;
}
</style>
