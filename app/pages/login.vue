<script setup lang="ts">
const { fetch: refreshSession } = useUserSession()
const localePath = useLocalePath()
const { t } = useI18n()

const email = ref('')
const password = ref('')
const error = ref('')
const pending = ref(false)

async function submit() {
  error.value = ''
  pending.value = true
  try {
    await $fetch('/api/auth/login', {
      method: 'POST',
      body: { email: email.value, password: password.value },
    })
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
    </form>
  </section>
</template>
