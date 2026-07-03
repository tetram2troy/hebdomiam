<script setup lang="ts">
import type { MealPlanDto, PlanDefaults, PlanRecipeDto } from '#shared/types/plan'
import type { RecipeDetail } from '#shared/types/recipe'

definePageMeta({ middleware: 'auth' })

const localePath = useLocalePath()
const { t, locale } = useI18n()

const { data: bundle } = await useFetch<{ plan: MealPlanDto | null, defaults: PlanDefaults }>('/api/plans/current')

const plan = ref(bundle.value?.plan ?? null)
const cookablePlan = computed(() =>
  plan.value && plan.value.status !== 'draft' ? plan.value : null)

// ── Onglet courant + détail de la recette sélectionnée ─────────────────────
const selectedId = ref('')

function selectDefaultTab() {
  const recipes = cookablePlan.value?.recipes ?? []
  const current = recipes.find(r => r.id === selectedId.value)
  if (current && current.status !== 'cooked') return
  selectedId.value = (recipes.find(r => r.status === 'accepted') ?? recipes[0])?.id ?? ''
}
selectDefaultTab()

const selected = computed(() =>
  cookablePlan.value?.recipes.find(r => r.id === selectedId.value) ?? null)

const { data: detail } = await useFetch<RecipeDetail>(
  () => `/api/recipes/${selected.value?.recipe.id}`,
  { immediate: Boolean(selected.value), watch: [selectedId] },
)

const scaleFactor = computed(() => {
  if (!selected.value || !detail.value) return 1
  return selected.value.targetServings / detail.value.baseServings
})

const numberFormat = computed(() =>
  new Intl.NumberFormat(locale.value === 'fr' ? 'fr-CA' : 'en-CA', { maximumFractionDigits: 2 }))

function scaledQuantity(ingredient: RecipeDetail['ingredients'][number]): string {
  const scaled = Math.round(ingredient.quantity * scaleFactor.value * 100) / 100
  const formatted = numberFormat.value.format(scaled)
  return ingredient.unit && ingredient.unit !== 'unit'
    ? `${formatted} ${t(`units.${ingredient.unit}`)}`
    : formatted
}

// ── Cases d'étapes persistées (interruptions fréquentes en cuisine) ────────
const doneSteps = ref<Set<string>>(new Set())

function storageKey() {
  return `hb-cuisine-${selectedId.value}`
}

watch(selectedId, () => {
  if (!import.meta.client || !selectedId.value) return
  try {
    doneSteps.value = new Set(JSON.parse(localStorage.getItem(storageKey()) ?? '[]'))
  }
  catch {
    doneSteps.value = new Set()
  }
}, { immediate: true })

function toggleStep(stepId: string) {
  const next = new Set(doneSteps.value)
  if (next.has(stepId)) next.delete(stepId)
  else next.add(stepId)
  doneSteps.value = next
  localStorage.setItem(storageKey(), JSON.stringify([...next]))
}

// ── Wake Lock : écran allumé, mains dans la farine ─────────────────────────
const wakeActive = ref(false)
let wakeLock: { release: () => Promise<void> } | null = null

async function acquireWakeLock() {
  try {
    const navigatorWithLock = navigator as Navigator & { wakeLock?: { request: (type: 'screen') => Promise<{ release: () => Promise<void>, addEventListener: (t: string, f: () => void) => void }> } }
    if (!navigatorWithLock.wakeLock) return
    const lock = await navigatorWithLock.wakeLock.request('screen')
    lock.addEventListener('release', () => { wakeActive.value = false })
    wakeLock = lock
    wakeActive.value = true
  }
  catch {
    wakeActive.value = false
  }
}

onMounted(() => {
  acquireWakeLock()
  document.addEventListener('visibilitychange', onVisibility)
})

function onVisibility() {
  if (document.visibilityState === 'visible' && !wakeActive.value) acquireWakeLock()
}

onBeforeUnmount(() => {
  document.removeEventListener('visibilitychange', onVisibility)
  wakeLock?.release().catch(() => {})
})

// ── Cuisinée ✓ — alimente l'historique de rotation ─────────────────────────
const error = ref('')
const marking = ref(false)

async function markCooked() {
  if (!cookablePlan.value || !selected.value) return
  error.value = ''
  marking.value = true
  try {
    plan.value = await $fetch<MealPlanDto>(
      `/api/plans/${cookablePlan.value.id}/recipes/${selected.value.id}`,
      { method: 'PATCH', body: { action: 'cook' } },
    )
    localStorage.removeItem(storageKey())
    selectDefaultTab()
  }
  catch (err) {
    error.value = errorMessage(err, t)
  }
  finally {
    marking.value = false
  }
}

const allCooked = computed(() =>
  cookablePlan.value !== null && cookablePlan.value.recipes.every(r => r.status === 'cooked'))

function tabLabel(item: PlanRecipeDto): string {
  const title = item.recipe.title
  return title.length > 22 ? `${title.slice(0, 21)}…` : title
}
</script>

<template>
  <div class="cook">
    <!-- Pas de plan validé -->
    <div v-if="!cookablePlan" class="empty" data-testid="no-cooking">
      <h1>{{ $t('cooking.emptyTitle') }}</h1>
      <p class="meta">{{ $t('cooking.emptyBody') }}</p>
      <NuxtLink :to="localePath('/planifier')" class="btn">{{ $t('cooking.goPlan') }}</NuxtLink>
    </div>

    <template v-else>
      <header class="khead">
        <h1>{{ $t('cooking.title') }}</h1>
        <span class="chip wake" :class="{ off: !wakeActive }" data-testid="wake-chip">
          {{ wakeActive ? $t('cooking.wakeOn') : $t('cooking.wakeOff') }}
        </span>
      </header>

      <p v-if="error" class="error" role="alert">{{ error }}</p>
      <p v-if="allCooked" class="all-done" data-testid="all-cooked">{{ $t('cooking.allDone') }}</p>

      <div class="rtabs" role="tablist">
        <button
          v-for="item in cookablePlan.recipes"
          :key="item.id"
          type="button"
          class="rtab"
          :class="{ on: item.id === selectedId, done: item.status === 'cooked' }"
          role="tab"
          :aria-selected="item.id === selectedId"
          data-testid="cook-tab"
          @click="selectedId = item.id"
        >
          <template v-if="item.status === 'cooked'">✓ </template>{{ tabLabel(item) }}
        </button>
      </div>

      <template v-if="selected && detail">
        <div class="scale">
          <span class="big" data-testid="scaled-servings">{{ $t('cooking.servingsBig', { n: selected.targetServings }, selected.targetServings) }}</span>
          <span class="meta">{{ $t('cooking.scale', { base: detail.baseServings, factor: numberFormat.format(scaleFactor) }) }}</span>
        </div>

        <details class="ingfold">
          <summary>{{ $t('cooking.ingredientsFold', { n: detail.ingredients.length }) }}</summary>
          <ul class="ing-list">
            <li v-for="ingredient in detail.ingredients" :key="ingredient.id">
              <b class="qty">{{ scaledQuantity(ingredient) }}</b> {{ ingredient.name }}
            </li>
          </ul>
        </details>

        <ol class="ksteps">
          <li v-for="(step, index) in detail.steps" :key="step.id">
            <button
              type="button"
              class="kstep"
              :class="{ done: doneSteps.has(step.id) }"
              :aria-pressed="doneSteps.has(step.id)"
              :aria-label="$t('cooking.stepLabel', { n: index + 1 })"
              data-testid="cook-step"
              @click="toggleStep(step.id)"
            >
              <span class="box" aria-hidden="true">{{ doneSteps.has(step.id) ? '✓' : '' }}</span>
              <span class="text">{{ step.text }}</span>
            </button>
          </li>
        </ol>

        <div class="kfoot">
          <button
            v-if="selected.status === 'accepted'"
            type="button"
            class="big-cta"
            :disabled="marking"
            data-testid="mark-cooked"
            @click="markCooked"
          >{{ $t('cooking.markCooked') }}</button>
          <p v-else class="meta">✓ {{ $t('cooking.cookedAt') }}</p>
          <p class="meta progress">{{ $t('cooking.progress', { done: [...doneSteps].filter(id => detail!.steps.some(s => s.id === id)).length, total: detail.steps.length }) }}</p>
        </div>
      </template>
    </template>
  </div>
</template>

<style scoped>
/* Mode cuisine : écran posé sur le comptoir, lisible à un mètre. */
.cook {
  max-width: 640px;
  margin: 0 auto;
}

.empty {
  text-align: center;
  margin: var(--hb-space-12) auto;
  display: grid;
  gap: var(--hb-space-3);
  justify-items: center;
}

.khead {
  display: flex;
  align-items: center;
  gap: var(--hb-space-3);
  flex-wrap: wrap;
  margin-bottom: var(--hb-space-3);
}

.khead h1 {
  margin: 0;
  flex: 1;
}

.wake.off {
  background: var(--md-sys-color-surface-container-high);
  color: var(--md-sys-color-on-surface-variant);
}

.all-done {
  color: var(--md-sys-color-primary);
  font-weight: 700;
}

.rtabs {
  display: flex;
  gap: var(--hb-space-2);
  overflow-x: auto;
  padding-bottom: var(--hb-space-2);
  margin-bottom: var(--hb-space-3);
}

.rtab {
  flex: none;
  background: var(--md-sys-color-surface-container-lowest);
  border: 2px solid var(--md-sys-color-outline);
  color: var(--md-sys-color-on-surface-variant);
  font: var(--md-sys-typescale-label-large);
  min-height: 44px;
}

.rtab.done {
  border-color: var(--md-sys-color-primary);
  background: var(--md-sys-color-secondary-container);
  color: var(--md-sys-color-on-secondary-container);
}

.rtab.on {
  border-color: var(--md-sys-color-primary);
  background: var(--md-sys-color-primary);
  color: var(--md-sys-color-on-primary);
}

.scale {
  display: flex;
  align-items: baseline;
  gap: var(--hb-space-3);
  flex-wrap: wrap;
  background: var(--md-sys-color-surface-container-lowest);
  border: 1px solid var(--md-sys-color-outline-variant);
  border-radius: var(--md-sys-shape-corner-medium);
  padding: var(--hb-space-3) var(--hb-space-4);
  margin-bottom: var(--hb-space-3);
}

.scale .big {
  font-family: var(--md-sys-typescale-font-brand);
  font-size: 30px;
  font-weight: 800;
  color: var(--md-sys-color-primary);
  font-variant-numeric: tabular-nums;
}

.ingfold {
  background: var(--md-sys-color-surface-container-low);
  border-radius: var(--md-sys-shape-corner-medium);
  padding: var(--hb-space-3) var(--hb-space-4);
  margin-bottom: var(--hb-space-3);
}

.ingfold summary {
  font: var(--md-sys-typescale-title-medium);
  cursor: pointer;
  min-height: 32px;
}

.ing-list {
  margin: var(--hb-space-2) 0 0;
  padding: 0;
  list-style: none;
  display: grid;
  gap: var(--hb-space-1);
  font: var(--md-sys-typescale-body-large);
}

.qty {
  font-variant-numeric: tabular-nums;
}

.ksteps {
  list-style: none;
  margin: 0;
  padding: 0;
  display: grid;
  gap: var(--hb-space-2);
}

.kstep {
  width: 100%;
  display: flex;
  align-items: flex-start;
  gap: var(--hb-space-4);
  background: var(--md-sys-color-surface-container-lowest);
  border: 1px solid var(--md-sys-color-outline-variant);
  border-radius: var(--md-sys-shape-corner-medium);
  color: var(--md-sys-color-on-surface);
  padding: var(--hb-space-4);
  text-align: left;
  min-height: var(--hb-touch-target-store);
}

.kstep .box {
  flex: none;
  width: 32px;
  height: 32px;
  border-radius: var(--md-sys-shape-corner-medium);
  border: 2.5px solid var(--md-sys-color-outline);
  display: grid;
  place-items: center;
  font-weight: 800;
  color: var(--md-sys-color-on-primary);
  margin-top: 2px;
}

.kstep.done .box {
  background: var(--md-sys-color-primary);
  border-color: var(--md-sys-color-primary);
}

/* Lisible à distance de bras (décision : hb-kitchen-step) */
.kstep .text {
  font: var(--hb-sys-typescale-kitchen-step, 400 1.25rem/1.5 var(--md-sys-typescale-font-plain));
}

.kstep.done .text {
  opacity: 0.45;
  text-decoration: line-through;
}

.kfoot {
  margin-top: var(--hb-space-4);
  display: grid;
  gap: var(--hb-space-2);
  justify-items: center;
}

.big-cta {
  width: 100%;
  min-height: var(--hb-touch-target-store);
  font-size: 16px;
}

.progress {
  font-variant-numeric: tabular-nums;
}
</style>
