<script setup lang="ts">
// Placeholder illustré déterministe : picto au trait dérivé du premier tag reconnu
// (décision design §10.2). Couleurs par tokens, jamais aléatoire au rendu.
const props = defineProps<{ tags?: string[] }>()

type Picto = 'pot' | 'soup' | 'leaf' | 'fish' | 'flame' | 'default'

const RULES: Array<[RegExp, Picto]> = [
  [/mijoteuse|slow/i, 'pot'],
  [/soupe|soup|potage/i, 'soup'],
  [/v[ée]g[ée]|vegan/i, 'leaf'],
  [/poisson|fish|saumon|crevette/i, 'fish'],
  [/grill|bbq|barbecue/i, 'flame'],
]

const picto = computed<Picto>(() => {
  for (const tag of props.tags ?? []) {
    for (const [re, p] of RULES) {
      if (re.test(tag)) return p
    }
  }
  return 'default'
})
</script>

<template>
  <div class="ph" aria-hidden="true">
    <svg width="44" height="44" viewBox="0 0 44 44" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round">
      <template v-if="picto === 'pot'">
        <path d="M17 8 c0 4, 4 4, 4 8 M25 6 c0 4, 4 4, 4 8" />
        <path d="M8 22 h28 v4 a11 11 0 0 1 -11 11 h-6 a11 11 0 0 1 -11 -11 z" />
        <path d="M5 22 h34" />
      </template>
      <template v-else-if="picto === 'soup'">
        <path d="M18 6 c0 3, 3 3, 3 6 M26 6 c0 3, 3 3, 3 6" />
        <path d="M8 20 h28 a14 14 0 0 1 -28 0 z" />
      </template>
      <template v-else-if="picto === 'leaf'">
        <path d="M34 8 C18 8, 10 18, 10 30 c12 0, 22 -8, 24 -22 z" />
        <path d="M10 36 C16 28, 24 20, 32 12" />
      </template>
      <template v-else-if="picto === 'fish'">
        <path d="M6 22 c6 -8, 14 -10, 22 -6 l8 -6 v14 l-8 -6 c-8 4, -16 2, -22 -6 z" transform="translate(0 6)" />
        <circle cx="13" cy="26" r="1.4" fill="currentColor" stroke="none" />
      </template>
      <template v-else-if="picto === 'flame'">
        <path d="M22 6 c2 6, 10 9, 10 18 a10 10 0 0 1 -20 0 c0 -6, 4 -8, 6 -12 c1 2, 2 3, 4 4 c0 -4, -1 -7, 0 -10 z" />
      </template>
      <template v-else>
        <path d="M14 10 v-4 M22 10 v-4 M30 10 v-4" />
        <path d="M10 16 h24 v8 a10 10 0 0 1 -10 10 h-4 a10 10 0 0 1 -10 -10 z" />
        <path d="M34 18 h4 v4 h-4" />
      </template>
    </svg>
  </div>
</template>

<style scoped>
.ph {
  display: grid;
  place-items: center;
  width: 100%;
  height: 100%;
  background: var(--md-sys-color-secondary-container);
  color: var(--md-sys-color-on-secondary-container);
}
</style>
