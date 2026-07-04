<script setup lang="ts">
const localePath = useLocalePath()
const route = useRoute()
const { t } = useI18n()

const password = ref('')
const done = ref(false)
const error = ref('')
const pending = ref(false)

async function submit() {
  error.value = ''
  pending.value = true
  try {
    await $fetch('/api/auth/password/reset', {
      method: 'POST',
      body: { token: String(route.query.token ?? ''), password: password.value },
    })
    done.value = true
  }
  catch (err) {
    error.value = errorMessage(err, t)
  }
  finally {
    pending.value = false
  }
}
</script>

<template>
  <section>
    <h1>{{ $t('auth.resetTitle') }}</h1>

    <template v-if="done">
      <p class="ok" role="status" data-testid="reset-done">{{ $t('auth.resetDone') }}</p>
      <NuxtLink :to="localePath('/login')" class="btn">{{ $t('auth.login') }}</NuxtLink>
    </template>
    <form v-else @submit.prevent="submit">
      <label>
        {{ $t('auth.password') }}
        <input v-model="password" type="password" autocomplete="new-password" required minlength="12">
      </label>
      <p class="hint">{{ $t('auth.passwordHint') }}</p>
      <p v-if="error" class="error" role="alert">{{ error }}</p>
      <button type="submit" :disabled="pending" data-testid="reset-submit">{{ $t('auth.resetSubmit') }}</button>
    </form>
  </section>
</template>

<style scoped>
.ok {
  color: var(--md-sys-color-primary);
  font-weight: 600;
}
</style>
