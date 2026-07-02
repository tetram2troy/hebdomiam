<script setup lang="ts">
import type { RecipeSummary, TagSummary } from '#shared/types/recipe'

definePageMeta({ middleware: 'auth' })

const localePath = useLocalePath()

const search = ref('')
const debouncedSearch = ref('')
const activeTag = ref<string>()

let searchTimer: ReturnType<typeof setTimeout> | undefined
watch(search, (value) => {
  clearTimeout(searchTimer)
  searchTimer = setTimeout(() => {
    debouncedSearch.value = value.trim()
  }, 250)
})

const query = computed(() => ({
  q: debouncedSearch.value || undefined,
  tag: activeTag.value,
}))

const { data: recipes, pending } = useFetch<RecipeSummary[]>('/api/recipes', { query })
const { data: tags } = useFetch<TagSummary[]>('/api/tags')

const hasFilters = computed(() => Boolean(debouncedSearch.value || activeTag.value))

function toggleTag(name: string) {
  activeTag.value = activeTag.value === name ? undefined : name
}

function clearFilters() {
  search.value = ''
  debouncedSearch.value = ''
  activeTag.value = undefined
}
</script>

<template>
  <div>
    <div class="page-head">
      <h1>{{ $t('recipes.listTitle') }}</h1>
      <NuxtLink :to="localePath('/recettes/nouvelle')" class="btn" data-testid="new-recipe">
        + {{ $t('recipes.new') }}
      </NuxtLink>
    </div>

    <div class="toolbar">
      <input
        v-model="search"
        type="search"
        class="search"
        :aria-label="$t('recipes.searchLabel')"
        :placeholder="$t('recipes.searchPlaceholder')"
      >
    </div>

    <div v-if="tags?.length" class="filters" role="group" :aria-label="$t('recipes.filters')">
      <button
        v-for="tag in tags"
        :key="tag.name"
        type="button"
        class="chip-filter chip"
        :aria-pressed="activeTag === tag.name"
        @click="toggleTag(tag.name)"
      >
        {{ tag.name }}<span v-if="activeTag === tag.name"> ✓</span>
      </button>
      <button v-if="hasFilters" type="button" class="btn-text clear-btn" @click="clearFilters">
        {{ $t('recipes.clearFilters') }}
      </button>
    </div>

    <p v-if="recipes && hasFilters" class="meta result-count" aria-live="polite">
      {{ $t('recipes.resultCount', { n: recipes.length }, recipes.length) }}
    </p>

    <!-- Squelettes pendant le chargement -->
    <div v-if="pending && !recipes" class="recipe-grid" :aria-label="$t('common.loading')">
      <div v-for="n in 6" :key="n" class="card">
        <div class="skel thumb-skel" />
        <div class="body-skel">
          <div class="skel line-70" />
          <div class="skel line-45" />
        </div>
      </div>
    </div>

    <div v-else-if="recipes?.length" class="recipe-grid">
      <RecipeCard v-for="recipe in recipes" :key="recipe.id" :recipe="recipe" />
    </div>

    <!-- Recherche sans résultat -->
    <div v-else-if="hasFilters" class="empty" data-testid="no-results">
      <h2>{{ $t('recipes.noResultsTitle') }}</h2>
      <p class="meta">{{ $t('recipes.noResultsBody') }}</p>
      <button type="button" class="btn-outline" @click="clearFilters">{{ $t('recipes.clearFilters') }}</button>
    </div>

    <!-- Collection vide -->
    <div v-else class="empty" data-testid="empty-collection">
      <RecipePlaceholder class="empty-illus" />
      <h2>{{ $t('recipes.emptyTitle') }}</h2>
      <p class="meta">{{ $t('recipes.emptyBody') }}</p>
      <NuxtLink :to="localePath('/recettes/nouvelle')" class="btn">
        + {{ $t('recipes.new') }}
      </NuxtLink>
    </div>
  </div>
</template>

<style scoped>
.page-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--hb-space-4);
  flex-wrap: wrap;
  margin-bottom: var(--hb-space-4);
}

.page-head h1 {
  margin: 0;
}

.toolbar {
  margin-bottom: var(--hb-space-3);
}

.search {
  max-width: 480px;
}

.filters {
  display: flex;
  flex-wrap: wrap;
  gap: var(--hb-space-2);
  align-items: center;
  margin-bottom: var(--hb-space-3);
}

.clear-btn {
  min-height: 36px;
  padding: 4px 12px;
}

.result-count {
  margin-bottom: var(--hb-space-3);
}

.recipe-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(240px, 1fr));
  gap: var(--hb-space-4);
}

/* Décision design §10.5 : 3 colonnes max */
@media (min-width: 1280px) {
  .recipe-grid {
    grid-template-columns: repeat(3, 1fr);
  }
}

.thumb-skel {
  height: 140px;
  border-radius: 0;
}

.body-skel {
  padding: var(--hb-space-4);
  display: grid;
  gap: var(--hb-space-2);
}

.line-70 { height: 16px; width: 70%; }
.line-45 { height: 12px; width: 45%; }

.empty {
  text-align: center;
  max-width: 420px;
  margin: var(--hb-space-12) auto;
  display: grid;
  gap: var(--hb-space-3);
  justify-items: center;
}

.empty-illus {
  width: 96px;
  height: 96px;
  border-radius: 50%;
  overflow: hidden;
}
</style>
