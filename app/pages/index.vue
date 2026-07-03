<script setup lang="ts">
import type { MealPlanDto, PlanDefaults } from '#shared/types/plan'

const { loggedIn, user } = useUserSession()
const localePath = useLocalePath()
const { t, locale } = useI18n()

const { data: bundle } = await useFetch<{ plan: MealPlanDto | null, defaults: PlanDefaults }>(
  '/api/plans/current',
  { immediate: loggedIn.value, watch: false },
)

const plan = computed(() => bundle.value?.plan ?? null)
const error = ref('')
const busyItemId = ref('')

const weekLabel = computed(() => {
  if (!plan.value) return ''
  const [year, month, day] = plan.value.weekStartDate.split('-').map(Number)
  const date = new Date(year!, month! - 1, day!)
  return new Intl.DateTimeFormat(locale.value === 'fr' ? 'fr-CA' : 'en-CA', {
    day: 'numeric',
    month: 'long',
  }).format(date)
})

const cookedCount = computed(() => plan.value?.recipes.filter(r => r.status === 'cooked').length ?? 0)
const allCooked = computed(() => plan.value !== null && cookedCount.value === plan.value.recipes.length)

async function markCooked(itemId: string) {
  if (!plan.value || !bundle.value) return
  error.value = ''
  busyItemId.value = itemId
  try {
    bundle.value = {
      ...bundle.value,
      plan: await $fetch<MealPlanDto>(`/api/plans/${plan.value.id}/recipes/${itemId}`, {
        method: 'PATCH',
        body: { action: 'cook' },
      }),
    }
  }
  catch (err) {
    error.value = errorMessage(err, t)
  }
  finally {
    busyItemId.value = ''
  }
}

async function replan() {
  if (!window.confirm(t('home.replanConfirm'))) return
  await navigateTo({ path: localePath('/planifier'), query: { replace: '1' } })
}
</script>

<template>
  <section>
    <template v-if="loggedIn">
      <p class="welcome meta" data-testid="welcome">{{ $t('home.welcome', { name: user?.name }) }}</p>

      <!-- État A — rien de prévu -->
      <div v-if="!plan" class="empty" data-testid="no-plan">
        <RecipePlaceholder class="illus" />
        <h1>{{ $t('home.nothingTitle') }}</h1>
        <p class="meta">{{ $t('home.nothingBody') }}</p>
        <NuxtLink :to="localePath('/planifier')" class="btn big" data-testid="start-planning">
          {{ $t('nav.plan') }}
        </NuxtLink>
        <NuxtLink :to="localePath('/recettes')" class="btn btn-text">
          {{ $t('home.browseRecipes', { n: bundle?.defaults.recipeCount ?? 0 }) }}
        </NuxtLink>
      </div>

      <!-- État B — semaine planifiée -->
      <div v-else data-testid="week-plan">
        <div class="week-head">
          <h1>{{ $t('home.weekOf', { date: weekLabel }) }}</h1>
          <span v-if="plan.status === 'draft'" class="chip draft-chip">{{ $t('home.draftChip') }}</span>
          <span v-else class="chip" data-testid="cooked-chip">{{ $t('home.cookedChip', { cooked: cookedCount, total: plan.recipes.length }) }}</span>
          <span class="meta">{{ $t('recipes.servings', { n: plan.totalServings }, plan.totalServings) }} · {{ plan.peopleCount }} 👤</span>
        </div>

        <p v-if="error" class="error" role="alert">{{ error }}</p>
        <p v-if="allCooked" class="all-cooked" data-testid="all-cooked">{{ $t('home.allCooked') }}</p>

        <div class="plan-grid">
          <article v-for="item in plan.recipes" :key="item.id" class="card plan-card" data-testid="plan-card">
            <NuxtLink :to="localePath(`/recettes/${item.recipe.id}`)" class="thumb-link">
              <div class="thumb">
                <img v-if="item.recipe.imageUrl" :src="item.recipe.imageUrl" alt="">
                <RecipePlaceholder v-else :tags="item.recipe.tags" />
              </div>
            </NuxtLink>
            <div class="body">
              <span
                v-if="plan.status !== 'draft'"
                class="badge"
                :class="item.status === 'cooked' ? 'badge-done' : 'badge-todo'"
              >{{ item.status === 'cooked' ? $t('home.statusCooked') : $t('home.statusTodo') }}</span>
              <h2 class="title">
                <NuxtLink :to="localePath(`/recettes/${item.recipe.id}`)" class="title-link">{{ item.recipe.title }}</NuxtLink>
              </h2>
              <p class="meta">{{ $t('recipes.servings', { n: item.targetServings }, item.targetServings) }}</p>
              <button
                v-if="plan.status === 'validated' && item.status === 'accepted'"
                type="button"
                class="btn-tonal cook-btn"
                :disabled="busyItemId === item.id"
                data-testid="mark-cooked"
                @click="markCooked(item.id)"
              >{{ $t('home.markCooked') }}</button>
            </div>
          </article>
        </div>

        <div class="week-actions">
          <NuxtLink v-if="plan.status === 'draft'" :to="localePath('/planifier')" class="btn" data-testid="resume-plan">
            {{ $t('home.resume') }}
          </NuxtLink>
          <button v-else type="button" class="btn-outline" data-testid="replan" @click="replan">
            {{ $t('home.replan') }}
          </button>
        </div>
      </div>
    </template>

    <template v-else>
      <h1>{{ $t('app.title') }}</h1>
      <p>{{ $t('home.pitch') }}</p>
      <NuxtLink :to="localePath('/register')" class="btn">{{ $t('auth.register') }}</NuxtLink>
    </template>
  </section>
</template>

<style scoped>
.welcome {
  margin-bottom: var(--hb-space-4);
}

.empty {
  text-align: center;
  max-width: 460px;
  margin: var(--hb-space-10) auto;
  display: grid;
  gap: var(--hb-space-3);
  justify-items: center;
}

.illus {
  width: 110px;
  height: 110px;
  border-radius: 50%;
  overflow: hidden;
}

.big {
  min-height: var(--hb-touch-target-store);
  font-size: 16px;
  padding-inline: var(--hb-space-8);
}

.week-head {
  display: flex;
  align-items: center;
  gap: var(--hb-space-3);
  flex-wrap: wrap;
  margin-bottom: var(--hb-space-4);
}

.week-head h1 {
  margin: 0;
}

.draft-chip {
  background: var(--md-sys-color-tertiary-container);
  color: var(--md-sys-color-on-tertiary-container);
}

.all-cooked {
  color: var(--md-sys-color-primary);
  font-weight: 700;
}

.plan-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
  gap: var(--hb-space-4);
  margin-bottom: var(--hb-space-5);
}

@media (min-width: 1280px) {
  .plan-grid {
    grid-template-columns: repeat(4, 1fr);
  }
}

.plan-card .thumb {
  height: 110px;
  overflow: hidden;
}

.plan-card .thumb img {
  width: 100%;
  height: 100%;
  object-fit: cover;
  display: block;
}

.thumb-link {
  display: block;
}

.plan-card .body {
  padding: var(--hb-space-3) var(--hb-space-4) var(--hb-space-4);
  display: grid;
  gap: var(--hb-space-1);
  justify-items: start;
}

.badge {
  padding: 2px 10px;
  border-radius: var(--md-sys-shape-corner-full);
  font: var(--md-sys-typescale-label-medium);
}

.badge-done {
  background: var(--md-sys-color-secondary-container);
  color: var(--md-sys-color-on-secondary-container);
}

.badge-todo {
  background: var(--md-sys-color-tertiary-container);
  color: var(--md-sys-color-on-tertiary-container);
}

.plan-card .title {
  font: var(--md-sys-typescale-title-medium);
  margin: 0;
}

.title-link {
  color: inherit;
  text-decoration: none;
}

.cook-btn {
  margin-top: var(--hb-space-2);
}

.week-actions {
  display: flex;
  gap: var(--hb-space-3);
}
</style>
