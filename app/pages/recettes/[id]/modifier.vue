<script setup lang="ts">
import type { RecipeDetail } from '#shared/types/recipe'

definePageMeta({ middleware: 'auth' })

const route = useRoute()
const localePath = useLocalePath()

const { data: recipe, error: fetchError } = await useFetch<RecipeDetail>(`/api/recipes/${route.params.id as string}`)
</script>

<template>
  <div v-if="fetchError" class="empty">
    <h1>{{ $t('errors.notFound') }}</h1>
    <NuxtLink :to="localePath('/recettes')" class="btn-outline btn">{{ $t('recipes.backToList') }}</NuxtLink>
  </div>

  <div v-else-if="recipe" class="form-page">
    <h1>{{ $t('recipes.form.editTitle') }}</h1>
    <RecipeForm mode="edit" :recipe="recipe" />
  </div>
</template>

<style scoped>
.form-page {
  max-width: 960px;
}

.empty {
  text-align: center;
  margin: var(--hb-space-12) auto;
  display: grid;
  gap: var(--hb-space-4);
  justify-items: center;
}
</style>
