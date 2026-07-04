<script setup lang="ts">
const localePath = useLocalePath()
const route = useRoute()

const state = ref<'pending' | 'done' | 'failed'>('pending')

onMounted(async () => {
  try {
    await $fetch('/api/auth/verify/confirm', {
      method: 'POST',
      body: { token: String(route.query.token ?? '') },
    })
    state.value = 'done'
  }
  catch {
    state.value = 'failed'
  }
})
</script>

<template>
  <section>
    <h1>{{ $t('auth.verifyTitle') }}</h1>
    <p v-if="state === 'pending'" class="meta">{{ $t('common.loading') }}</p>
    <p v-else-if="state === 'done'" class="ok" role="status" data-testid="verify-done">{{ $t('auth.verifyDone') }}</p>
    <p v-else class="error" role="alert" data-testid="verify-failed">{{ $t('auth.verifyFailed') }}</p>
    <NuxtLink :to="localePath('/')" class="btn btn-text">{{ $t('nav.home') }}</NuxtLink>
  </section>
</template>

<style scoped>
.ok {
  color: var(--md-sys-color-primary);
  font-weight: 600;
}
</style>
