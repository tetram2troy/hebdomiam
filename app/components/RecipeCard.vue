<script setup lang="ts">
const props = defineProps<{
  recipe: {
    id: string
    title: string
    imageUrl: string | null
    prepMinutes: number | null
    cookMinutes: number | null
    baseServings: number
    freezable: boolean
    tags: string[]
  }
}>()

const localePath = useLocalePath()

// Photo distante bloquée (hotlink) ou morte → placeholder illustré.
const imageFailed = ref(false)

const totalMinutes = computed(() => {
  const total = (props.recipe.prepMinutes ?? 0) + (props.recipe.cookMinutes ?? 0)
  return total > 0 ? total : null
})

const visibleTags = computed(() => props.recipe.tags.slice(0, 2))
const extraTags = computed(() => props.recipe.tags.length - 2)
</script>

<template>
  <NuxtLink :to="localePath(`/recettes/${recipe.id}`)" class="card recipe-card" data-testid="recipe-card">
    <div class="thumb">
      <img v-if="recipe.imageUrl && !imageFailed" :src="recipe.imageUrl" alt="" loading="lazy" @error="imageFailed = true">
      <RecipePlaceholder v-else :tags="recipe.tags" />
    </div>
    <div class="body">
      <h3 class="title">{{ recipe.title }}</h3>
      <p class="meta">
        <span v-if="totalMinutes">{{ $t('recipes.minutes', { n: totalMinutes }) }} · </span>
        <span>{{ $t('recipes.servings', recipe.baseServings) }}</span>
        <span v-if="recipe.freezable" class="frz"> · ❄ {{ $t('recipes.freezable') }}</span>
      </p>
      <p v-if="recipe.tags.length" class="tags">
        <span v-for="tag in visibleTags" :key="tag" class="chip chip-sm">{{ tag }}</span>
        <span v-if="extraTags > 0" class="chip chip-sm">+{{ extraTags }}</span>
      </p>
    </div>
  </NuxtLink>
</template>

<style scoped>
.recipe-card {
  display: block;
  color: inherit;
  text-decoration: none;
}

@media (prefers-reduced-motion: no-preference) {
  .recipe-card {
    transition: box-shadow var(--md-sys-motion-duration-short) var(--md-sys-motion-easing-standard),
      translate var(--md-sys-motion-duration-short) var(--md-sys-motion-easing-standard);
  }
}

.recipe-card:hover {
  box-shadow: var(--md-sys-elevation-level2);
  translate: 0 -2px;
}

.thumb {
  height: 150px;
  width: 100%;
  overflow: hidden;
}

.thumb img {
  width: 100%;
  height: 100%;
  object-fit: cover;
  display: block;
}

.body {
  padding: var(--hb-space-3) var(--hb-space-4) var(--hb-space-4);
}

.title {
  font: var(--md-sys-typescale-title-large);
  margin: 0 0 var(--hb-space-1);
  display: -webkit-box;
  -webkit-box-orient: vertical;
  -webkit-line-clamp: 2;
  line-clamp: 2;
  overflow: hidden;
}

.frz {
  color: var(--md-sys-color-primary);
  font-weight: 600;
}

.tags {
  display: flex;
  gap: var(--hb-space-2);
  margin: var(--hb-space-2) 0 0;
  flex-wrap: wrap;
}

.chip-sm {
  padding: 3px 10px;
  font: var(--md-sys-typescale-label-medium);
}
</style>
