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

// Les filtres tiennent sur une rangée défilante, à toutes les tailles d'écran.
// Au-delà de quelques tags, un bouton « Tous » ouvre la liste complète
// (RecipeTagSheet) — en dessous du seuil, la rangée suffit et le bouton ne
// serait que du bruit.
const ALL_TAGS_THRESHOLD = 6
const tagSheet = ref<{ open: () => void } | null>(null)
const filtersRow = ref<HTMLElement | null>(null)

// Ordre de la rangée : le tag actif d'abord, puis « Tout effacer », puis les
// autres tags. Il est porté par le DOM et non par un `order` CSS : tabulation
// et lecteurs d'écran doivent suivre l'ordre visuel (design-system §9).
type RowItem = { kind: 'tag', tag: TagSummary } | { kind: 'clear' }

const filterRow = computed<RowItem[]>(() => {
  const all = tags.value ?? []
  const active = all.find(tag => tag.name === activeTag.value)
  return [
    ...(active ? [{ kind: 'tag' as const, tag: active }] : []),
    ...(hasFilters.value ? [{ kind: 'clear' as const }] : []),
    ...all.filter(tag => tag !== active).map(tag => ({ kind: 'tag' as const, tag })),
  ]
})

// Le tag actif arrive en tête : on y ramène aussi le défilement, sinon un tag
// choisi dans la liste complète resterait hors écran.
watch(activeTag, () => {
  filtersRow.value?.scrollTo({ left: 0 })
})

function clearFilters() {
  search.value = ''
  debouncedSearch.value = ''
  activeTag.value = undefined
}

// Depuis la rangée, le bouton activé change de place dans le DOM (ou
// disparaît, pour « Tout effacer ») et perd le focus : on le rend au tag
// devenu actif, sinon au premier bouton de la rangée — toujours à l'écran.
async function refocusRow(selector: string) {
  await nextTick()
  filtersRow.value?.querySelector<HTMLElement>(selector)?.focus({ preventScroll: true })
}

async function toggleTagInRow(name: string) {
  toggleTag(name)
  await refocusRow(activeTag.value === name ? '[aria-pressed="true"]' : 'button')
}

async function clearFiltersInRow() {
  clearFilters()
  await refocusRow('button')
}
</script>

<template>
  <div>
    <div class="page-head">
      <h1>{{ $t('recipes.listTitle') }}</h1>
      <div class="head-actions">
        <NuxtLink :to="localePath('/recettes/importer')" class="btn btn-outline" data-testid="import-recipe">
          {{ $t('recipes.importCta') }}
        </NuxtLink>
        <NuxtLink :to="localePath('/recettes/nouvelle')" class="btn" data-testid="new-recipe">
          + {{ $t('recipes.new') }}
        </NuxtLink>
      </div>
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

    <div v-if="tags?.length" ref="filtersRow" class="filters" role="group" :aria-label="$t('recipes.filters')">
      <button
        v-if="tags.length > ALL_TAGS_THRESHOLD"
        type="button"
        class="chip all-tags"
        aria-haspopup="dialog"
        data-testid="all-tags"
        @click="tagSheet?.open()"
      >
        {{ $t('recipes.allTags', { n: tags.length }) }}
      </button>
      <template v-for="item in filterRow" :key="item.kind === 'tag' ? `tag:${item.tag.name}` : 'clear'">
        <button
          v-if="item.kind === 'tag'"
          type="button"
          class="chip-filter chip"
          :aria-pressed="activeTag === item.tag.name"
          @click="toggleTagInRow(item.tag.name)"
        >
          {{ item.tag.name }}<span v-if="activeTag === item.tag.name"> ✓</span>
        </button>
        <button v-else type="button" class="btn-text clear-btn" @click="clearFiltersInRow">
          {{ $t('recipes.clearFilters') }}
        </button>
      </template>
    </div>
    <RecipeTagSheet v-if="tags?.length" ref="tagSheet" :tags="tags" :active="activeTag" @select="toggleTag" />

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

.head-actions {
  display: flex;
  gap: var(--hb-space-3);
}

.toolbar {
  margin-bottom: var(--hb-space-3);
}

.search {
  max-width: 480px;
}

/* Une seule rangée défilante, à toutes les tailles d'écran, au lieu d'un mur
   de tags avant la première recette (design-system §8.7). La liste complète
   s'ouvre par le bouton « Tous · N ». */
.filters {
  display: flex;
  gap: var(--hb-space-2);
  align-items: center;
  overflow-x: auto;
  scrollbar-width: none;
  /* Le padding (compensé par une marge négative) laisse la place à l'anneau
     de focus, que `overflow` rognerait ; à droite, celle du dégradé. */
  margin: calc(-1 * var(--hb-space-1)) calc(-1 * var(--hb-space-1)) var(--hb-space-2);
  padding: var(--hb-space-1) var(--hb-space-10) var(--hb-space-1) var(--hb-space-1);
  /* Dégradé d'indication : le dernier tag visible s'estompe à droite. */
  mask-image: linear-gradient(to right, #000 calc(100% - var(--hb-space-10)), transparent);
}

.filters::-webkit-scrollbar {
  display: none;
}

.filters > * {
  flex: none;
  white-space: nowrap;
}

.all-tags {
  cursor: pointer;
}

.clear-btn {
  min-height: 36px;
  padding: 4px 12px;
}

/* Mobile : bord à bord — la rangée déborde des marges de page pour défiler
   jusqu'aux bords de l'écran. */
@media (max-width: 767px) {
  .filters {
    margin-inline: calc(-1 * var(--hb-space-4));
    padding-left: var(--hb-space-4);
  }
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
