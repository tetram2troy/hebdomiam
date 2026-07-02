<script setup lang="ts">
import type { RecipeDetail } from '#shared/types/recipe'

definePageMeta({ middleware: 'auth' })

const route = useRoute()
const localePath = useLocalePath()
const { t, locale } = useI18n()

const { data: recipe, error: fetchError } = await useFetch<RecipeDetail>(`/api/recipes/${route.params.id as string}`)

const servings = ref(recipe.value?.baseServings ?? 4)
const deleteError = ref('')
const deleting = ref(false)

const totalMinutes = computed(() => {
  if (!recipe.value) return 0
  return (recipe.value.prepMinutes ?? 0) + (recipe.value.cookMinutes ?? 0)
})

const quantityFormat = computed(() => new Intl.NumberFormat(locale.value === 'fr' ? 'fr-CA' : 'en-CA', {
  maximumFractionDigits: 2,
}))

function scaledQuantity(ingredient: { quantity: number, unit: string | null }): string {
  if (!recipe.value) return ''
  const scaled = ingredient.quantity * servings.value / recipe.value.baseServings
  const formatted = quantityFormat.value.format(Math.round(scaled * 100) / 100)
  return ingredient.unit ? `${formatted} ${t(`units.${ingredient.unit}`)}` : formatted
}

async function removeRecipe() {
  if (!recipe.value) return
  if (!window.confirm(t('recipes.deleteConfirm', { title: recipe.value.title }))) return
  deleteError.value = ''
  deleting.value = true
  try {
    await $fetch(`/api/recipes/${recipe.value.id}`, { method: 'DELETE' })
    await navigateTo(localePath('/recettes'))
  }
  catch (err) {
    deleteError.value = errorMessage(err, t)
  }
  finally {
    deleting.value = false
  }
}
</script>

<template>
  <div v-if="fetchError" class="empty">
    <h1>{{ $t('errors.notFound') }}</h1>
    <NuxtLink :to="localePath('/recettes')" class="btn-outline btn">{{ $t('recipes.backToList') }}</NuxtLink>
  </div>

  <article v-else-if="recipe">
    <p class="meta back">
      <NuxtLink :to="localePath('/recettes')" class="back-link">← {{ $t('recipes.backToList') }}</NuxtLink>
    </p>

    <header class="head">
      <div>
        <p v-if="recipe.tags.length" class="tags">
          <span v-for="tag in recipe.tags" :key="tag" class="chip">{{ tag }}</span>
        </p>
        <h1 data-testid="recipe-title">{{ recipe.title }}</h1>
        <p v-if="recipe.description" class="description">{{ recipe.description }}</p>
        <p class="metaline">
          <span v-if="recipe.prepMinutes">{{ $t('recipes.prep') }} <b>{{ $t('recipes.minutes', { n: recipe.prepMinutes }) }}</b></span>
          <span v-if="recipe.cookMinutes">{{ $t('recipes.cook') }} <b>{{ $t('recipes.minutes', { n: recipe.cookMinutes }) }}</b></span>
          <span v-if="totalMinutes">{{ $t('recipes.total') }} <b>{{ $t('recipes.minutes', { n: totalMinutes }) }}</b></span>
          <span v-if="recipe.freezable" class="frz">❄ {{ $t('recipes.freezable') }}</span>
        </p>
        <p v-if="recipe.sourceUrl" class="meta">
          {{ $t('recipes.source') }} : <a :href="recipe.sourceUrl" rel="noopener nofollow" target="_blank">{{ recipe.sourceUrl }}</a>
        </p>
        <div class="actions">
          <NuxtLink :to="localePath(`/recettes/${recipe.id}/modifier`)" class="btn btn-outline" data-testid="edit-recipe">
            ✎ {{ $t('common.edit') }}
          </NuxtLink>
          <button type="button" class="btn-danger" :disabled="deleting" data-testid="delete-recipe" @click="removeRecipe">
            {{ $t('common.delete') }}
          </button>
        </div>
        <p v-if="deleteError" class="error" role="alert">{{ deleteError }}</p>
      </div>
      <div class="photo">
        <img v-if="recipe.imageUrl" :src="recipe.imageUrl" alt="">
        <RecipePlaceholder v-else :tags="recipe.tags" />
      </div>
    </header>

    <div class="cols">
      <section class="panel">
        <div class="panel-head">
          <h2>{{ $t('recipes.ingredients') }}</h2>
          <HbStepper v-model="servings" :label="$t('recipes.portionsLabel')" :min="1" :max="50" />
        </div>
        <p class="hint">{{ $t('recipes.portionsLabel') }} — {{ $t('recipes.portionsHint') }}</p>
        <ul class="ingredients">
          <li v-for="ingredient in recipe.ingredients" :key="ingredient.id">
            <span class="qty">{{ scaledQuantity(ingredient) }}</span>
            <span class="name">{{ ingredient.name }}</span>
            <span class="meta aisle">{{ $t(`aisles.${ingredient.aisle}`) }}</span>
          </li>
        </ul>
      </section>

      <section class="panel">
        <h2>{{ $t('recipes.steps') }}</h2>
        <ol class="steps">
          <li v-for="step in recipe.steps" :key="step.id">{{ step.text }}</li>
        </ol>
        <div v-if="recipe.notes" class="notes">
          <b>{{ $t('recipes.notes') }} :</b> {{ recipe.notes }}
        </div>
      </section>
    </div>
  </article>
</template>

<style scoped>
.back {
  margin-bottom: var(--hb-space-4);
}

.back-link {
  color: var(--md-sys-color-on-surface-variant);
  text-decoration: none;
}

.head {
  display: grid;
  grid-template-columns: minmax(0, 7fr) minmax(0, 5fr);
  gap: var(--hb-space-6);
  margin-bottom: var(--hb-space-6);
}

@media (max-width: 860px) {
  .head {
    grid-template-columns: 1fr;
  }

  .photo {
    order: -1;
  }
}

.tags {
  display: flex;
  gap: var(--hb-space-2);
  flex-wrap: wrap;
  margin: 0 0 var(--hb-space-2);
}

.description {
  color: var(--md-sys-color-on-surface-variant);
  max-width: 60ch;
}

.metaline {
  display: flex;
  flex-wrap: wrap;
  gap: var(--hb-space-2) var(--hb-space-5);
  font: var(--md-sys-typescale-body-medium);
  color: var(--md-sys-color-on-surface-variant);
}

.metaline b {
  color: var(--md-sys-color-on-surface);
}

.frz {
  color: var(--md-sys-color-primary);
  font-weight: 600;
}

.actions {
  display: flex;
  gap: var(--hb-space-3);
  margin-top: var(--hb-space-4);
}

.photo {
  aspect-ratio: 3 / 2;
  border-radius: var(--md-sys-shape-corner-large);
  overflow: hidden;
  max-height: 260px;
}

.photo img {
  width: 100%;
  height: 100%;
  object-fit: cover;
  display: block;
}

.cols {
  display: grid;
  grid-template-columns: minmax(0, 5fr) minmax(0, 7fr);
  gap: var(--hb-space-6);
  align-items: start;
}

@media (max-width: 860px) {
  .cols {
    grid-template-columns: 1fr;
  }
}

.panel {
  background: var(--md-sys-color-surface-container-lowest);
  border: 1px solid var(--md-sys-color-outline-variant);
  border-radius: var(--md-sys-shape-corner-large);
  padding: var(--hb-space-5);
}

.panel-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--hb-space-3);
  flex-wrap: wrap;
}

.panel-head h2, .panel h2 {
  margin: 0 0 var(--hb-space-2);
}

.ingredients {
  list-style: none;
  margin: var(--hb-space-3) 0 0;
  padding: 0;
}

.ingredients li {
  display: flex;
  gap: var(--hb-space-3);
  padding: var(--hb-space-2) 0;
  border-bottom: 1px solid var(--md-sys-color-outline-variant);
  align-items: baseline;
}

.ingredients li:last-child {
  border-bottom: none;
}

.qty {
  min-width: 96px;
  text-align: right;
  font-weight: 700;
  font-variant-numeric: tabular-nums;
}

.name {
  flex: 1;
}

.steps {
  margin: var(--hb-space-2) 0 0;
  padding-left: var(--hb-space-6);
  display: grid;
  gap: var(--hb-space-3);
}

.steps li::marker {
  color: var(--md-sys-color-primary);
  font-weight: 700;
}

.notes {
  margin-top: var(--hb-space-4);
  background: var(--md-sys-color-surface-container-low);
  border: 1px solid var(--md-sys-color-outline-variant);
  border-radius: var(--md-sys-shape-corner-medium);
  padding: var(--hb-space-3) var(--hb-space-4);
  font: var(--md-sys-typescale-body-medium);
}

.empty {
  text-align: center;
  margin: var(--hb-space-12) auto;
  display: grid;
  gap: var(--hb-space-4);
  justify-items: center;
}
</style>
