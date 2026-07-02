<script setup lang="ts">
const { loggedIn, user } = useUserSession()
</script>

<template>
  <section>
    <h1>{{ $t('app.title') }}</h1>
    <p v-if="loggedIn" data-testid="welcome">
      {{ $t('home.welcome', { name: user?.name }) }}
    </p>
    <p v-else>{{ $t('home.pitch') }}</p>
  </section>
</template>
