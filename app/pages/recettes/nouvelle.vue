<script setup lang="ts">
definePageMeta({ middleware: 'auth' })
</script>

<template>
  <div class="form-page">
    <h1>{{ $t('recipes.form.createTitle') }}</h1>
    <RecipeForm mode="create" />
  </div>
</template>

<style scoped>
.form-page {
  max-width: 960px;
}
</style>
