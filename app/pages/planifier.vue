<script setup lang="ts">
import type { MealPlanDto, PlanDefaults, PlanRecipeDto, ReplacementOption } from '#shared/types/plan'

definePageMeta({ middleware: 'auth' })

const localePath = useLocalePath()
const route = useRoute()
const { t } = useI18n()

const { data: bundle } = await useFetch<{ plan: MealPlanDto | null, defaults: PlanDefaults }>('/api/plans/current')

const plan = ref<MealPlanDto | null>(bundle.value?.plan ?? null)
const defaults = bundle.value?.defaults

const params = reactive({
  dishCount: defaults?.dishCount ?? 4,
  peopleCount: defaults?.peopleCount ?? 1,
  daysCount: defaults?.daysCount ?? 7,
})

// « Replanifier » depuis l'accueil : repartir des paramètres même si un plan existe.
const forceParams = ref(route.query.replace === '1')

const phase = computed(() => {
  if (forceParams.value || !plan.value) return 'params'
  return plan.value.status === 'draft' ? 'suggestions' : 'done'
})

const totalServings = computed(() => params.daysCount * 2 * params.peopleCount)
const notEnough = computed(() => (defaults?.recipeCount ?? 0) < params.dishCount)
const acceptedCount = computed(() => plan.value?.recipes.filter(r => r.status === 'accepted').length ?? 0)

const error = ref('')
const creating = ref(false)
const validating = ref(false)
const busyItemId = ref('')

async function propose() {
  error.value = ''
  creating.value = true
  try {
    plan.value = await $fetch<MealPlanDto>('/api/plans', {
      method: 'POST',
      body: { ...params, replace: forceParams.value },
    })
    forceParams.value = false
  }
  catch (err) {
    error.value = errorMessage(err, t)
  }
  finally {
    creating.value = false
  }
}

async function itemAction(item: PlanRecipeDto, action: 'accept' | 'unaccept') {
  if (!plan.value) return
  error.value = ''
  busyItemId.value = item.id
  try {
    plan.value = await $fetch<MealPlanDto>(`/api/plans/${plan.value.id}/recipes/${item.id}`, {
      method: 'PATCH',
      body: { action },
    })
  }
  catch (err) {
    error.value = errorMessage(err, t)
  }
  finally {
    busyItemId.value = ''
  }
}

async function swapItem(item: PlanRecipeDto) {
  if (!plan.value) return
  error.value = ''
  busyItemId.value = item.id
  try {
    plan.value = await $fetch<MealPlanDto>(`/api/plans/${plan.value.id}/recipes/${item.id}/swap`, { method: 'POST' })
  }
  catch (err) {
    error.value = errorMessage(err, t)
  }
  finally {
    busyItemId.value = ''
  }
}

async function validateWeek() {
  if (!plan.value) return
  error.value = ''
  validating.value = true
  try {
    plan.value = await $fetch<MealPlanDto>(`/api/plans/${plan.value.id}/validate`, { method: 'POST' })
  }
  catch (err) {
    error.value = errorMessage(err, t)
  }
  finally {
    validating.value = false
  }
}

// ── Panneau « Remplacer » ──────────────────────────────────────────────────
const replacing = ref<PlanRecipeDto | null>(null)
const replaceMode = ref<'search' | 'suggest'>('search')
const replaceQuery = ref('')
const options = ref<ReplacementOption[]>([])
const optionsPending = ref(false)

let optionsTimer: ReturnType<typeof setTimeout> | undefined
watch([replacing, replaceMode, replaceQuery], () => {
  if (!replacing.value) return
  clearTimeout(optionsTimer)
  optionsTimer = setTimeout(loadOptions, replaceQuery.value ? 250 : 0)
})

async function loadOptions() {
  if (!plan.value || !replacing.value) return
  optionsPending.value = true
  try {
    options.value = await $fetch<ReplacementOption[]>(`/api/plans/${plan.value.id}/replacements`, {
      query: {
        mode: replaceMode.value,
        q: replaceMode.value === 'search' && replaceQuery.value.trim() ? replaceQuery.value.trim() : undefined,
      },
    })
  }
  catch (err) {
    error.value = errorMessage(err, t)
  }
  finally {
    optionsPending.value = false
  }
}

function openReplace(item: PlanRecipeDto) {
  replacing.value = item
  replaceMode.value = 'search'
  replaceQuery.value = ''
  options.value = []
}

async function chooseReplacement(option: ReplacementOption) {
  if (!plan.value || !replacing.value) return
  error.value = ''
  try {
    plan.value = await $fetch<MealPlanDto>(`/api/plans/${plan.value.id}/recipes/${replacing.value.id}/replace`, {
      method: 'POST',
      body: { recipeId: option.id },
    })
    replacing.value = null
  }
  catch (err) {
    error.value = errorMessage(err, t)
  }
}

function rotationLabel(option: ReplacementOption): string {
  if (!option.lastCookedAt) return t('plan.neverCooked')
  const weeks = Math.max(1, Math.floor((Date.now() - new Date(option.lastCookedAt).getTime()) / (7 * 24 * 60 * 60 * 1000)))
  return t('plan.cookedAgo', { n: weeks })
}

function totalMinutes(item: PlanRecipeDto): number {
  return (item.recipe.prepMinutes ?? 0) + (item.recipe.cookMinutes ?? 0)
}
</script>

<template>
  <div>
    <nav class="steps" :aria-label="$t('plan.title')">
      <span class="step" :class="{ on: phase === 'params' }"><i>1</i> {{ $t('plan.stepParams') }}</span>
      <span class="sep" aria-hidden="true" />
      <span class="step" :class="{ on: phase === 'suggestions' }"><i>2</i> {{ $t('plan.stepSuggestions') }}</span>
      <span class="sep" aria-hidden="true" />
      <span class="step" :class="{ on: phase === 'done' }"><i>3</i> {{ $t('plan.stepValidation') }}</span>
    </nav>

    <p v-if="error" class="error" role="alert">{{ error }}</p>

    <!-- ÉTAPE 1 — PARAMÈTRES -->
    <section v-if="phase === 'params'" class="params" data-testid="plan-params">
      <h1>{{ $t('plan.paramsTitle') }}</h1>

      <div class="prow">
        <span class="lab"><b>{{ $t('plan.dishes') }}</b><span class="hint">{{ $t('plan.dishesHint') }}</span></span>
        <HbStepper v-model="params.dishCount" :label="$t('plan.dishes')" :min="1" :max="10" />
      </div>
      <div class="prow">
        <span class="lab"><b>{{ $t('plan.people') }}</b><span class="hint">{{ $t('plan.peopleHint') }}</span></span>
        <HbStepper v-model="params.peopleCount" :label="$t('plan.people')" :min="1" :max="12" />
      </div>
      <div class="prow">
        <span class="lab"><b>{{ $t('plan.days') }}</b><span class="hint">{{ $t('plan.daysHint') }}</span></span>
        <HbStepper v-model="params.daysCount" :label="$t('plan.days')" :min="1" :max="7" />
      </div>
      <div class="prow">
        <span class="lab"><b>{{ $t('plan.meals') }}</b></span>
        <span class="chip">{{ $t('plan.mealsValue') }}</span>
      </div>

      <p class="calc">{{ $t('plan.calc', { total: totalServings, days: params.daysCount, people: params.peopleCount }) }}</p>

      <div v-if="notEnough" class="warn" data-testid="not-enough">
        <p>{{ $t('plan.notEnough', { needed: params.dishCount, count: defaults?.recipeCount ?? 0 }) }}</p>
        <NuxtLink :to="localePath('/recettes/nouvelle')" class="btn btn-tonal">{{ $t('plan.addRecipes') }}</NuxtLink>
      </div>

      <button
        type="button"
        class="big-cta"
        :disabled="creating || notEnough"
        data-testid="propose"
        @click="propose"
      >{{ $t('plan.propose', { n: params.dishCount }) }}</button>
    </section>

    <!-- ÉTAPE 2 — SUGGESTIONS -->
    <section v-else-if="phase === 'suggestions' && plan" data-testid="plan-suggestions">
      <div class="sugg-head">
        <h1>{{ $t('plan.suggestionsTitle', { n: plan.recipes.length }) }}</h1>
        <span class="chip" data-testid="accepted-count">{{ $t('plan.acceptedCount', { accepted: acceptedCount, total: plan.recipes.length }) }}</span>
        <span class="meta">{{ $t('plan.portionsEach', { n: plan.recipes[0]?.targetServings ?? 0 }) }}</span>
      </div>

      <div class="sugg-grid">
        <article
          v-for="item in plan.recipes"
          :key="item.id"
          class="card sugg"
          :class="{ ok: item.status === 'accepted', busy: busyItemId === item.id }"
          data-testid="suggestion-card"
        >
          <span v-if="item.status === 'accepted'" class="okbadge">{{ $t('plan.acceptedBadge') }}</span>
          <div class="thumb">
            <img v-if="item.recipe.imageUrl" :src="item.recipe.imageUrl" alt="">
            <RecipePlaceholder v-else :tags="item.recipe.tags" />
          </div>
          <div class="body">
            <p v-if="item.recipe.tags.length" class="tags">
              <span v-for="tag in item.recipe.tags.slice(0, 2)" :key="tag" class="chip chip-sm">{{ tag }}</span>
            </p>
            <h2 class="title">
              <NuxtLink :to="localePath(`/recettes/${item.recipe.id}`)" class="title-link">{{ item.recipe.title }}</NuxtLink>
            </h2>
            <p class="meta">
              <span v-if="totalMinutes(item)">{{ $t('recipes.minutes', { n: totalMinutes(item) }) }} · </span>
              <span>{{ $t('recipes.servings', { n: item.targetServings }, item.targetServings) }}</span>
              <span v-if="item.recipe.freezable"> · ❄</span>
            </p>

            <div v-if="item.status === 'suggested'" class="acts">
              <div class="row">
                <button type="button" :disabled="busyItemId === item.id" data-testid="accept" @click="itemAction(item, 'accept')">{{ $t('plan.accept') }}</button>
                <button type="button" class="btn-outline" :disabled="busyItemId === item.id" :aria-label="$t('plan.swapLabel', { title: item.recipe.title })" data-testid="swap" @click="swapItem(item)">{{ $t('plan.swap') }}</button>
              </div>
              <button type="button" class="btn-text" data-testid="open-replace" @click="openReplace(item)">{{ $t('plan.openReplace') }}</button>
            </div>
            <div v-else class="acts">
              <button type="button" class="btn-text" :disabled="busyItemId === item.id" @click="itemAction(item, 'unaccept')">{{ $t('plan.unaccept') }}</button>
            </div>
          </div>
        </article>
      </div>

      <!-- Panneau remplacer -->
      <div v-if="replacing" class="replace" role="dialog" :aria-label="$t('plan.replaceTitle', { title: replacing.recipe.title })" data-testid="replace-panel">
        <div class="replace-head">
          <h2>{{ $t('plan.replaceTitle', { title: replacing.recipe.title }) }}</h2>
          <button type="button" class="btn-text" @click="replacing = null">✕ {{ $t('plan.close') }}</button>
        </div>
        <div class="modes">
          <button type="button" class="chip chip-filter" :aria-pressed="replaceMode === 'search'" @click="replaceMode = 'search'">{{ $t('plan.modeSearch') }}</button>
          <button type="button" class="chip chip-filter" :aria-pressed="replaceMode === 'suggest'" data-testid="mode-suggest" @click="replaceMode = 'suggest'">{{ $t('plan.modeSuggest') }}</button>
        </div>
        <input
          v-if="replaceMode === 'search'"
          v-model="replaceQuery"
          type="search"
          :aria-label="$t('plan.searchMine')"
          :placeholder="$t('plan.searchMine')"
        >
        <p v-else class="meta">{{ $t('plan.suggestHint') }}</p>

        <div v-if="optionsPending" class="skel" style="height: 56px" />
        <p v-else-if="options.length === 0" class="meta">{{ $t('plan.noOptions') }}</p>
        <ul v-else class="options">
          <li v-for="option in options" :key="option.id">
            <div class="opt-thumb">
              <img v-if="option.imageUrl" :src="option.imageUrl" alt="">
              <RecipePlaceholder v-else :tags="option.tags" />
            </div>
            <div class="grow">
              <b>{{ option.title }}</b>
              <p class="meta">{{ rotationLabel(option) }}</p>
            </div>
            <button type="button" class="btn-outline" data-testid="choose-option" @click="chooseReplacement(option)">{{ $t('plan.choose') }}</button>
          </li>
        </ul>
      </div>

      <div class="footer-bar">
        <span class="meta grow">{{ $t('plan.acceptAllHint') }}</span>
        <button type="button" class="big-cta" :disabled="validating" data-testid="validate-week" @click="validateWeek">
          {{ $t('plan.validateWeek') }}
        </button>
      </div>
    </section>

    <!-- ÉTAPE 3 — VALIDÉ -->
    <section v-else-if="plan" class="done" data-testid="plan-done">
      <h1>{{ $t('plan.validatedTitle') }}</h1>
      <p class="meta">{{ $t('plan.validatedBody') }}</p>
      <div class="mini">
        <div v-for="item in plan.recipes" :key="item.id" class="card mini-card">
          <div class="opt-thumb tall">
            <img v-if="item.recipe.imageUrl" :src="item.recipe.imageUrl" alt="">
            <RecipePlaceholder v-else :tags="item.recipe.tags" />
          </div>
          <p class="mini-title">{{ item.recipe.title }}</p>
        </div>
      </div>
      <NuxtLink :to="localePath('/epicerie')" class="btn big-cta" data-testid="see-grocery">{{ $t('plan.seeGrocery') }}</NuxtLink>
      <NuxtLink :to="localePath('/')" class="btn btn-text" data-testid="back-home">{{ $t('plan.backHome') }}</NuxtLink>
    </section>
  </div>
</template>

<style scoped>
.steps {
  display: flex;
  align-items: center;
  gap: var(--hb-space-2);
  margin-bottom: var(--hb-space-6);
  font: var(--md-sys-typescale-label-medium);
  color: var(--md-sys-color-on-surface-variant);
  flex-wrap: wrap;
}

.step {
  display: inline-flex;
  align-items: center;
  gap: var(--hb-space-2);
  font-weight: 700;
}

.step i {
  font-style: normal;
  width: 24px;
  height: 24px;
  border-radius: 50%;
  border: 2px solid var(--md-sys-color-outline);
  display: grid;
  place-items: center;
  font-size: 11px;
}

.step.on {
  color: var(--md-sys-color-on-surface);
}

.step.on i {
  background: var(--md-sys-color-primary);
  border-color: var(--md-sys-color-primary);
  color: var(--md-sys-color-on-primary);
}

.sep {
  flex: 0 0 24px;
  border-top: 2px solid var(--md-sys-color-outline-variant);
}

.params {
  max-width: 560px;
  margin: 0 auto;
}

.params h1 {
  text-align: center;
}

.prow {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--hb-space-4);
  padding: var(--hb-space-3) 0;
  border-bottom: 1px solid var(--md-sys-color-outline-variant);
}

.prow .lab b {
  display: block;
  font: var(--md-sys-typescale-title-medium);
}

.calc {
  text-align: center;
  color: var(--md-sys-color-on-surface-variant);
  margin: var(--hb-space-4) 0;
  font-variant-numeric: tabular-nums;
}

.warn {
  background: var(--md-sys-color-tertiary-container);
  color: var(--md-sys-color-on-tertiary-container);
  border-radius: var(--md-sys-shape-corner-medium);
  padding: var(--hb-space-4);
  display: grid;
  gap: var(--hb-space-2);
  justify-items: start;
  margin-bottom: var(--hb-space-4);
}

.big-cta {
  width: 100%;
  min-height: var(--hb-touch-target-store);
  font-size: 16px;
}

.sugg-head {
  display: flex;
  align-items: center;
  gap: var(--hb-space-3);
  flex-wrap: wrap;
  margin-bottom: var(--hb-space-4);
}

.sugg-head h1 {
  margin: 0;
  flex: 1 1 auto;
}

.sugg-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(240px, 1fr));
  gap: var(--hb-space-4);
  margin-bottom: var(--hb-space-4);
}

@media (min-width: 1280px) {
  .sugg-grid {
    grid-template-columns: repeat(4, 1fr);
  }
}

.sugg {
  position: relative;
  display: flex;
  flex-direction: column;
}

.sugg.ok {
  border: 2px solid var(--md-sys-color-primary);
}

.sugg.busy {
  opacity: 0.6;
}

.okbadge {
  position: absolute;
  top: var(--hb-space-2);
  right: var(--hb-space-2);
  background: var(--md-sys-color-primary);
  color: var(--md-sys-color-on-primary);
  border-radius: var(--md-sys-shape-corner-full);
  padding: 4px 12px;
  font: var(--md-sys-typescale-label-medium);
  z-index: 1;
}

.sugg .thumb {
  height: 120px;
  overflow: hidden;
}

.sugg .thumb img {
  width: 100%;
  height: 100%;
  object-fit: cover;
  display: block;
}

.sugg .body {
  padding: var(--hb-space-3) var(--hb-space-4) var(--hb-space-4);
  display: flex;
  flex-direction: column;
  flex: 1;
}

.tags {
  display: flex;
  gap: var(--hb-space-2);
  margin: 0 0 var(--hb-space-1);
}

.chip-sm {
  padding: 3px 10px;
  font: var(--md-sys-typescale-label-medium);
}

.sugg .title {
  font: var(--md-sys-typescale-title-large);
  margin: 0 0 var(--hb-space-1);
}

.title-link {
  color: inherit;
  text-decoration: none;
}

.acts {
  display: grid;
  gap: var(--hb-space-2);
  margin-top: auto;
  padding-top: var(--hb-space-3);
}

.acts .row {
  display: flex;
  gap: var(--hb-space-2);
}

.acts .row button {
  flex: 1;
  padding: 8px 6px;
  font-size: 13.5px;
}

.replace {
  border: 2px solid var(--md-sys-color-primary);
  border-radius: var(--md-sys-shape-corner-large);
  background: var(--md-sys-color-surface-container-lowest);
  padding: var(--hb-space-4);
  max-width: 620px;
  margin-bottom: var(--hb-space-4);
  display: grid;
  gap: var(--hb-space-3);
}

.replace-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--hb-space-3);
}

.replace-head h2 {
  margin: 0;
  font: var(--md-sys-typescale-title-large);
}

.modes {
  display: flex;
  gap: var(--hb-space-2);
  flex-wrap: wrap;
}

.options {
  list-style: none;
  margin: 0;
  padding: 0;
  display: grid;
  gap: var(--hb-space-2);
}

.options li {
  display: flex;
  align-items: center;
  gap: var(--hb-space-3);
  padding: var(--hb-space-2) 0;
  border-bottom: 1px solid var(--md-sys-color-outline-variant);
}

.options li:last-child {
  border-bottom: none;
}

.opt-thumb {
  width: 48px;
  height: 48px;
  border-radius: var(--md-sys-shape-corner-medium);
  overflow: hidden;
  flex: none;
}

.opt-thumb img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.grow {
  flex: 1;
  min-width: 0;
}

.footer-bar {
  display: flex;
  align-items: center;
  gap: var(--hb-space-4);
  background: var(--md-sys-color-surface-container-lowest);
  border: 1px solid var(--md-sys-color-outline-variant);
  border-radius: var(--md-sys-shape-corner-large);
  padding: var(--hb-space-4);
  flex-wrap: wrap;
}

.footer-bar .big-cta {
  width: auto;
  flex: 1 1 280px;
}

.done {
  max-width: 720px;
  margin: 0 auto;
  text-align: center;
  display: grid;
  gap: var(--hb-space-4);
  justify-items: center;
}

.mini {
  display: flex;
  gap: var(--hb-space-3);
  flex-wrap: wrap;
  justify-content: center;
}

.mini-card {
  width: 140px;
}

.opt-thumb.tall {
  width: 100%;
  height: 64px;
  border-radius: 0;
}

.mini-title {
  font: var(--md-sys-typescale-label-large);
  padding: var(--hb-space-2) var(--hb-space-3) var(--hb-space-3);
  margin: 0;
}
</style>
