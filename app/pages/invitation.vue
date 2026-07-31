<script setup lang="ts">
// Acceptation d'une invitation : l'invité choisit son mot de passe via le lien
// reçu (même flux que la réinitialisation — pas de session posée, connexion ensuite).
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
    await $fetch('/api/auth/invitation', {
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
    <h1>{{ $t('auth.inviteTitle') }}</h1>
    <p class="meta">{{ $t('auth.inviteIntro') }}</p>

    <template v-if="done">
      <p class="ok" role="status" data-testid="invite-done">{{ $t('auth.inviteDone') }}</p>
      <NuxtLink :to="localePath('/login')" class="btn">{{ $t('auth.login') }}</NuxtLink>
    </template>
    <form v-else @submit.prevent="submit">
      <label>
        {{ $t('auth.password') }}
        <input v-model="password" type="password" autocomplete="new-password" required minlength="12" data-testid="invite-password">
      </label>
      <p class="hint">{{ $t('auth.passwordHint') }}</p>
      <p v-if="error" class="error" role="alert">{{ error }}</p>
      <button type="submit" :disabled="pending" data-testid="invite-submit">{{ $t('auth.inviteSubmit') }}</button>
    </form>
  </section>
</template>

<style scoped>
.ok {
  color: var(--md-sys-color-primary);
  font-weight: 600;
}
</style>
