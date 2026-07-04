<script setup lang="ts">
const { fetch: refreshSession } = useUserSession()
const localePath = useLocalePath()
const { t } = useI18n()

const email = ref('')
const password = ref('')
const code = ref('')
const requires2fa = ref(false)
const error = ref('')
const pending = ref(false)

async function submit() {
  error.value = ''
  pending.value = true
  try {
    const result = await $fetch<{ requires2fa?: boolean }>('/api/auth/login', {
      method: 'POST',
      body: { email: email.value, password: password.value },
    })
    if (result.requires2fa) {
      requires2fa.value = true
      return
    }
    await refreshSession()
    await navigateTo(localePath('/'))
  }
  catch (err) {
    error.value = errorMessage(err, t)
  }
  finally {
    pending.value = false
  }
}

async function submitCode() {
  error.value = ''
  pending.value = true
  try {
    await $fetch('/api/auth/2fa', { method: 'POST', body: { code: code.value } })
    await refreshSession()
    await navigateTo(localePath('/'))
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
    <template v-if="!requires2fa">
      <h1>{{ $t('auth.loginTitle') }}</h1>
      <form @submit.prevent="submit">
        <label>
          {{ $t('auth.email') }}
          <input v-model="email" type="email" name="email" autocomplete="email" required>
        </label>
        <label>
          {{ $t('auth.password') }}
          <input v-model="password" type="password" name="password" autocomplete="current-password" required>
        </label>
        <p v-if="error" class="error" role="alert">{{ error }}</p>
        <button type="submit" :disabled="pending">{{ $t('auth.submitLogin') }}</button>
        <NuxtLink :to="localePath('/mot-de-passe-oublie')" class="btn btn-text">{{ $t('auth.forgot') }}</NuxtLink>
      </form>
    </template>

    <template v-else>
      <h1>{{ $t('auth.totpTitle') }}</h1>
      <p class="hint">{{ $t('auth.totpBody') }}</p>
      <form @submit.prevent="submitCode">
        <label>
          {{ $t('auth.totpCode') }}
          <input
            v-model="code"
            type="text"
            inputmode="numeric"
            autocomplete="one-time-code"
            required
            minlength="6"
            maxlength="16"
            data-testid="totp-login-code"
          >
        </label>
        <p v-if="error" class="error" role="alert">{{ error }}</p>
        <button type="submit" :disabled="pending" data-testid="totp-login-submit">{{ $t('auth.totpSubmit') }}</button>
      </form>
    </template>
  </section>
</template>
