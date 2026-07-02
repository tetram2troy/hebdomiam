<script setup lang="ts">
const { fetch: refreshSession } = useUserSession()
const localePath = useLocalePath()
const { t } = useI18n()

const name = ref('')
const email = ref('')
const password = ref('')
const error = ref('')
const pending = ref(false)

async function submit() {
  error.value = ''
  pending.value = true
  try {
    await $fetch('/api/auth/register', {
      method: 'POST',
      body: { name: name.value, email: email.value, password: password.value },
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
    <h1>{{ $t('auth.registerTitle') }}</h1>
    <form @submit.prevent="submit">
      <label>
        {{ $t('auth.name') }}
        <input v-model="name" type="text" name="name" autocomplete="name" required>
      </label>
      <label>
        {{ $t('auth.email') }}
        <input v-model="email" type="email" name="email" autocomplete="email" required>
      </label>
      <label>
        {{ $t('auth.password') }}
        <input
          v-model="password"
          type="password"
          name="password"
          autocomplete="new-password"
          minlength="12"
          required
        >
        <span class="hint">{{ $t('auth.passwordHint') }}</span>
      </label>
      <p v-if="error" class="error" role="alert">{{ error }}</p>
      <button type="submit" :disabled="pending">{{ $t('auth.submitRegister') }}</button>
    </form>
  </section>
</template>
