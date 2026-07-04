<script setup lang="ts">
const localePath = useLocalePath()
const { t } = useI18n()

const email = ref('')
const sent = ref(false)
const error = ref('')
const pending = ref(false)

async function submit() {
  error.value = ''
  pending.value = true
  try {
    await $fetch('/api/auth/password/forgot', { method: 'POST', body: { email: email.value } })
    sent.value = true
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
    <h1>{{ $t('auth.forgotTitle') }}</h1>
    <p class="hint">{{ $t('auth.forgotBody') }}</p>

    <p v-if="sent" class="sent" role="status" data-testid="forgot-sent">{{ $t('auth.forgotSent') }}</p>
    <form v-else @submit.prevent="submit">
      <label>
        {{ $t('auth.email') }}
        <input v-model="email" type="email" autocomplete="email" required>
      </label>
      <p v-if="error" class="error" role="alert">{{ error }}</p>
      <button type="submit" :disabled="pending">{{ $t('auth.forgotSubmit') }}</button>
    </form>
    <NuxtLink :to="localePath('/login')" class="btn btn-text">{{ $t('auth.backToLogin') }}</NuxtLink>
  </section>
</template>

<style scoped>
.sent {
  color: var(--md-sys-color-primary);
  font-weight: 600;
}
</style>
