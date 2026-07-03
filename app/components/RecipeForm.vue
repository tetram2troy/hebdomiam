<script setup lang="ts">
import { AISLES, UNITS, type Aisle, type Unit } from '#shared/schemas/recipe'
import type { ImportDraft } from '#shared/types/import'
import type { RecipeDetail } from '#shared/types/recipe'

const props = defineProps<{
  mode: 'create' | 'edit'
  recipe?: RecipeDetail
  /** Brouillon d'import (URL/photo) : pré-remplit sans rien sauvegarder. */
  draft?: ImportDraft | null
}>()

const { t } = useI18n()
const localePath = useLocalePath()

interface IngredientDraft {
  quantity: string
  unit: Unit | ''
  name: string
  aisle: Aisle
}

function emptyIngredient(): IngredientDraft {
  return { quantity: '', unit: '', name: '', aisle: 'other' }
}

const seed = props.draft
  ? {
      ...props.draft,
      notes: null,
      steps: props.draft.steps,
      ingredients: props.draft.ingredients,
    }
  : props.recipe
    ? {
        ...props.recipe,
        steps: props.recipe.steps.map(s => s.text),
      }
    : null

const form = reactive({
  title: seed?.title ?? '',
  description: seed?.description ?? '',
  baseServings: seed?.baseServings ?? 4,
  prepMinutes: seed?.prepMinutes?.toString() ?? '',
  cookMinutes: seed?.cookMinutes?.toString() ?? '',
  freezable: seed?.freezable ?? false,
  notes: seed?.notes ?? '',
  ingredients: seed?.ingredients.map(i => ({
    quantity: String(i.quantity),
    unit: i.unit ?? '' as const,
    name: i.name,
    aisle: i.aisle,
  })) ?? [emptyIngredient()],
  steps: seed?.steps ?? [''],
  tags: [...(seed?.tags ?? [])],
})

const tagInput = ref('')
const error = ref('')
const pending = ref(false)

function addIngredient() {
  form.ingredients.push(emptyIngredient())
}

function removeIngredient(index: number) {
  form.ingredients.splice(index, 1)
}

function addStep() {
  form.steps.push('')
}

function removeStep(index: number) {
  form.steps.splice(index, 1)
}

function moveStep(index: number, delta: number) {
  const target = index + delta
  if (target < 0 || target >= form.steps.length) return
  const [step] = form.steps.splice(index, 1)
  form.steps.splice(target, 0, step!)
}

function addTag() {
  const name = tagInput.value.trim().replace(/,+$/, '')
  if (name && !form.tags.includes(name) && form.tags.length < 20) {
    form.tags.push(name)
  }
  tagInput.value = ''
}

function removeTag(tag: string) {
  form.tags = form.tags.filter(name => name !== tag)
}

function toPayload() {
  return {
    title: form.title,
    description: form.description.trim() || null,
    // Conservés à travers l'édition ; renseignés par l'import.
    sourceUrl: props.draft?.sourceUrl ?? props.recipe?.sourceUrl ?? null,
    imageUrl: props.draft?.imageUrl ?? props.recipe?.imageUrl ?? null,
    baseServings: form.baseServings,
    prepMinutes: form.prepMinutes === '' ? null : Number(form.prepMinutes),
    cookMinutes: form.cookMinutes === '' ? null : Number(form.cookMinutes),
    freezable: form.freezable,
    notes: form.notes.trim() || null,
    ingredients: form.ingredients.map(ing => ({
      quantity: Number(ing.quantity),
      unit: ing.unit === '' ? null : ing.unit,
      name: ing.name,
      aisle: ing.aisle,
    })),
    steps: form.steps.filter(text => text.trim() !== ''),
    tags: form.tags,
  }
}

async function submit() {
  // Un tag encore dans le champ de saisie fait partie de l'intention.
  addTag()
  error.value = ''
  pending.value = true
  try {
    const recipe = props.mode === 'create'
      ? await $fetch('/api/recipes', { method: 'POST', body: toPayload() })
      : await $fetch(`/api/recipes/${props.recipe!.id}`, { method: 'PUT', body: toPayload() })
    await navigateTo(localePath(`/recettes/${recipe.id}`))
  }
  catch (err) {
    error.value = errorMessage(err, t)
  }
  finally {
    pending.value = false
  }
}
</script>

<template>
  <form class="recipe-form" @submit.prevent="submit">
    <div class="grid">
      <section class="col">
        <label>
          {{ $t('recipes.form.title') }} *
          <input v-model="form.title" type="text" name="title" required maxlength="200">
        </label>

        <label>
          {{ $t('recipes.form.description') }}
          <textarea v-model="form.description" name="description" maxlength="2000" rows="3" />
        </label>

        <fieldset class="panel">
          <legend>{{ $t('recipes.form.ingredients') }} *</legend>
          <p class="hint">{{ $t('recipes.form.ingredientsHint') }}</p>
          <div
            v-for="(ingredient, index) in form.ingredients"
            :key="index"
            class="ingredient-row"
          >
            <label class="sr-only-label">
              <span class="sr-only">{{ $t('recipes.form.quantity') }}</span>
              <input
                v-model="ingredient.quantity"
                type="number"
                min="0.01"
                step="0.01"
                max="99999"
                required
                :placeholder="$t('recipes.form.quantity')"
              >
            </label>
            <label class="sr-only-label">
              <span class="sr-only">{{ $t('recipes.form.unit') }}</span>
              <select v-model="ingredient.unit">
                <option value="">{{ $t('units.none') }}</option>
                <option v-for="unit in UNITS" :key="unit" :value="unit">{{ $t(`units.${unit}`) }}</option>
              </select>
            </label>
            <label class="sr-only-label ingredient-name">
              <span class="sr-only">{{ $t('recipes.form.name') }}</span>
              <input
                v-model="ingredient.name"
                type="text"
                required
                maxlength="200"
                :placeholder="$t('recipes.form.name')"
              >
            </label>
            <label class="sr-only-label">
              <span class="sr-only">{{ $t('recipes.form.aisle') }}</span>
              <select v-model="ingredient.aisle">
                <option v-for="aisle in AISLES" :key="aisle" :value="aisle">{{ $t(`aisles.${aisle}`) }}</option>
              </select>
            </label>
            <button
              type="button"
              class="btn-text row-btn"
              :disabled="form.ingredients.length === 1"
              :aria-label="$t('recipes.form.removeIngredient', { n: index + 1 })"
              @click="removeIngredient(index)"
            >✕</button>
          </div>
          <button type="button" class="btn-text" @click="addIngredient">
            + {{ $t('recipes.form.addIngredient') }}
          </button>
        </fieldset>

        <fieldset class="panel">
          <legend>{{ $t('recipes.form.steps') }} *</legend>
          <div v-for="(step, index) in form.steps" :key="index" class="step-row">
            <label class="sr-only-label step-text">
              <span class="sr-only">{{ $t('recipes.form.stepLabel', { n: index + 1 }) }}</span>
              <textarea
                v-model="form.steps[index]"
                :required="index === 0"
                maxlength="2000"
                rows="2"
                :placeholder="$t('recipes.form.stepLabel', { n: index + 1 })"
              />
            </label>
            <div class="step-actions">
              <button type="button" class="btn-text row-btn" :disabled="index === 0" :aria-label="$t('recipes.form.moveStepUp', { n: index + 1 })" @click="moveStep(index, -1)">↑</button>
              <button type="button" class="btn-text row-btn" :disabled="index === form.steps.length - 1" :aria-label="$t('recipes.form.moveStepDown', { n: index + 1 })" @click="moveStep(index, 1)">↓</button>
              <button type="button" class="btn-text row-btn" :disabled="form.steps.length === 1" :aria-label="$t('recipes.form.removeStep', { n: index + 1 })" @click="removeStep(index)">✕</button>
            </div>
          </div>
          <button type="button" class="btn-text" @click="addStep">
            + {{ $t('recipes.form.addStep') }}
          </button>
        </fieldset>
      </section>

      <section class="col">
        <label>
          {{ $t('recipes.form.baseServings') }} *
          <input v-model.number="form.baseServings" type="number" min="1" max="50" required class="narrow">
        </label>

        <div class="two-cols">
          <label>
            {{ $t('recipes.form.prepMinutes') }}
            <input v-model="form.prepMinutes" type="number" min="0" max="1440">
          </label>
          <label>
            {{ $t('recipes.form.cookMinutes') }}
            <input v-model="form.cookMinutes" type="number" min="0" max="1440">
          </label>
        </div>

        <label>
          {{ $t('recipes.form.tags') }}
          <input
            v-model="tagInput"
            type="text"
            maxlength="30"
            @keydown.enter.prevent="addTag"
            @keydown.,.prevent="addTag"
          >
        </label>
        <p class="hint">{{ $t('recipes.form.tagsHint') }}</p>
        <p v-if="form.tags.length" class="tag-list">
          <button
            v-for="tag in form.tags"
            :key="tag"
            type="button"
            class="chip"
            :aria-label="$t('recipes.form.removeTag', { tag })"
            @click="removeTag(tag)"
          >{{ tag }} ✕</button>
        </p>

        <label class="check-row">
          <input v-model="form.freezable" type="checkbox" name="freezable">
          {{ $t('recipes.form.freezable') }}
        </label>

        <label>
          {{ $t('recipes.form.notes') }}
          <textarea v-model="form.notes" name="notes" maxlength="5000" rows="3" />
        </label>
      </section>
    </div>

    <p v-if="error" class="error" role="alert">{{ error }}</p>

    <div class="actions">
      <NuxtLink
        :to="mode === 'edit' && recipe ? localePath(`/recettes/${recipe.id}`) : localePath('/recettes')"
        class="btn btn-text"
      >{{ $t('common.cancel') }}</NuxtLink>
      <button type="submit" :disabled="pending" data-testid="recipe-submit">
        {{ mode === 'create' ? $t('recipes.form.submitCreate') : $t('recipes.form.submitEdit') }}
      </button>
    </div>
  </form>
</template>

<style scoped>
.grid {
  display: grid;
  grid-template-columns: minmax(0, 7fr) minmax(0, 5fr);
  gap: var(--hb-space-6);
}

@media (max-width: 860px) {
  .grid {
    grid-template-columns: 1fr;
  }
}

.col {
  display: grid;
  gap: var(--hb-space-4);
  align-content: start;
}

.panel {
  border: 1px solid var(--md-sys-color-outline-variant);
  border-radius: var(--md-sys-shape-corner-large);
  background: var(--md-sys-color-surface-container-lowest);
  padding: var(--hb-space-4);
  display: grid;
  gap: var(--hb-space-3);
  margin: 0;
}

.panel legend {
  font: var(--md-sys-typescale-title-medium);
  padding-inline: var(--hb-space-1);
}

.ingredient-row {
  display: grid;
  grid-template-columns: 90px 130px minmax(0, 1fr) 150px 44px;
  gap: var(--hb-space-2);
  align-items: start;
}

@media (max-width: 860px) {
  .ingredient-row {
    grid-template-columns: 90px 1fr 44px;
  }

  .ingredient-row .ingredient-name {
    grid-column: 1 / -2;
  }
}

.sr-only-label {
  display: block;
}

.sr-only {
  position: absolute;
  width: 1px;
  height: 1px;
  overflow: hidden;
  clip-path: inset(50%);
  white-space: nowrap;
}

.step-row {
  display: flex;
  gap: var(--hb-space-2);
  align-items: flex-start;
}

.step-text {
  flex: 1;
}

.step-actions {
  display: flex;
  gap: var(--hb-space-1);
}

.row-btn {
  min-height: 44px;
  min-width: 44px;
  padding: 0;
}

.two-cols {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: var(--hb-space-3);
}

.narrow {
  max-width: 120px;
}

.tag-list {
  display: flex;
  flex-wrap: wrap;
  gap: var(--hb-space-2);
  margin: 0;
}

.tag-list .chip {
  cursor: pointer;
  min-height: 36px;
}

.check-row {
  display: flex;
  align-items: center;
  gap: var(--hb-space-2);
  font: var(--md-sys-typescale-body-large);
}

.actions {
  display: flex;
  justify-content: flex-end;
  gap: var(--hb-space-3);
  border-top: 1px solid var(--md-sys-color-outline-variant);
  padding-top: var(--hb-space-4);
  margin-top: var(--hb-space-2);
}
</style>
