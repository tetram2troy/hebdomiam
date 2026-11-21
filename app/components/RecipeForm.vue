<script setup lang="ts">
import { AISLES, UNITS, type Aisle, type Unit } from '#shared/catalog'
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
  imageUrl: seed?.imageUrl ?? '',
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
const imageFailed = ref(false)

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
    // Conservé à travers l'édition ; renseigné par l'import.
    sourceUrl: props.draft?.sourceUrl ?? props.recipe?.sourceUrl ?? null,
    imageUrl: form.imageUrl.trim() || null,
    baseServings: form.baseServings,
    prepMinutes: form.prepMinutes === '' ? null : Number(form.prepMinutes),
    cookMinutes: form.cookMinutes === '' ? null : Number(form.cookMinutes),
    freezable: form.freezable,
    notes: form.notes.trim() || null,
    ingredients: form.ingredients.map(ing => ({
      quantity: Number(ing.quantity),
      unit: ing.unit === '' ? null : ing.unit,
      // Le nom est une zone de texte (pour passer à la ligne) : un collage peut
      // y glisser des sauts de ligne, qui n'ont pas de sens dans un nom.
      name: ing.name.replace(/\s*[\r\n]+\s*/g, ' '),
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
      ? await $fetch<RecipeDetail>('/api/recipes', { method: 'POST', body: toPayload() })
      : await $fetch<RecipeDetail>(`/api/recipes/${props.recipe!.id}`, { method: 'PUT', body: toPayload() })
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
  <!-- Desktop : le texte à relire (titre, description, ingrédients, étapes)
       occupe la colonne large ; les champs courts et l'enregistrement vivent
       dans un panneau latéral collant, toujours à portée après un import. -->
  <form class="recipe-form" @submit.prevent="submit">
    <section class="content" data-testid="form-content">
      <label>
        {{ $t('recipes.form.title') }} *
        <input v-model="form.title" type="text" name="title" required maxlength="200" class="title-input">
      </label>

      <label>
        {{ $t('recipes.form.description') }}
        <textarea v-model="form.description" v-autogrow name="description" maxlength="2000" rows="3" class="grow" data-testid="description" />
      </label>

      <fieldset class="panel">
        <legend>{{ $t('recipes.form.ingredients') }} *</legend>
        <p class="hint">{{ $t('recipes.form.ingredientsHint') }}</p>
        <!-- En-têtes visuels seulement : chaque champ garde son libellé
             accessible (sr-only) ligne par ligne. -->
        <div class="ingredient-head" aria-hidden="true">
          <span>{{ $t('recipes.form.quantity') }}</span>
          <span>{{ $t('recipes.form.unit') }}</span>
          <span>{{ $t('recipes.form.name') }}</span>
          <span>{{ $t('recipes.form.aisle') }}</span>
        </div>
        <div
          v-for="(ingredient, index) in form.ingredients"
          :key="index"
          class="ingredient-row"
        >
          <label class="sr-only-label field-qty">
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
          <label class="sr-only-label field-unit">
            <span class="sr-only">{{ $t('recipes.form.unit') }}</span>
            <select v-model="ingredient.unit">
              <option value="">{{ $t('units.none') }}</option>
              <option v-for="unit in UNITS" :key="unit" :value="unit">{{ $t(`units.${unit}`) }}</option>
            </select>
          </label>
          <label class="sr-only-label ingredient-name">
            <span class="sr-only">{{ $t('recipes.form.name') }}</span>
            <!-- Zone de texte d'une ligne qui passe à la ligne : un nom importé
                 long reste lisible en entier au lieu d'être coupé. Entrée ne
                 crée pas de saut de ligne (sans objet dans un nom). -->
            <textarea
              v-model="ingredient.name"
              v-autogrow
              rows="1"
              required
              maxlength="200"
              class="grow name-input"
              :placeholder="$t('recipes.form.name')"
              data-testid="ingredient-name"
              @keydown.enter.prevent
            />
          </label>
          <label class="sr-only-label field-aisle">
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
        <button type="button" class="btn-text add-btn" @click="addIngredient">
          + {{ $t('recipes.form.addIngredient') }}
        </button>
      </fieldset>

      <fieldset class="panel">
        <legend>{{ $t('recipes.form.steps') }} *</legend>
        <div v-for="(step, index) in form.steps" :key="index" class="step-row">
          <span class="step-num" aria-hidden="true">{{ index + 1 }}</span>
          <label class="sr-only-label step-text">
            <span class="sr-only">{{ $t('recipes.form.stepLabel', { n: index + 1 }) }}</span>
            <textarea
              v-model="form.steps[index]"
              v-autogrow
              :required="index === 0"
              maxlength="2000"
              rows="2"
              class="grow"
              :placeholder="$t('recipes.form.stepLabel', { n: index + 1 })"
            />
          </label>
          <div class="step-actions">
            <button type="button" class="btn-text row-btn" :disabled="index === 0" :aria-label="$t('recipes.form.moveStepUp', { n: index + 1 })" @click="moveStep(index, -1)">↑</button>
            <button type="button" class="btn-text row-btn" :disabled="index === form.steps.length - 1" :aria-label="$t('recipes.form.moveStepDown', { n: index + 1 })" @click="moveStep(index, 1)">↓</button>
            <button type="button" class="btn-text row-btn" :disabled="form.steps.length === 1" :aria-label="$t('recipes.form.removeStep', { n: index + 1 })" @click="removeStep(index)">✕</button>
          </div>
        </div>
        <button type="button" class="btn-text add-btn" @click="addStep">
          + {{ $t('recipes.form.addStep') }}
        </button>
      </fieldset>
    </section>

    <aside class="side" data-testid="form-side">
      <div class="photo-block">
        <img
          v-if="form.imageUrl && !imageFailed"
          :src="form.imageUrl"
          alt=""
          class="photo-preview"
          data-testid="image-preview"
          @error="imageFailed = true"
        >
        <p v-else-if="form.imageUrl && imageFailed" class="hint photo-broken" role="alert">
          {{ $t('recipes.form.photoBroken') }}
        </p>
        <label>
          {{ $t('recipes.form.photoUrl') }}
          <input
            v-model="form.imageUrl"
            type="url"
            maxlength="1000"
            :placeholder="$t('recipes.form.photoPlaceholder')"
            data-testid="image-url"
            @input="imageFailed = false"
          >
        </label>
        <button
          v-if="form.imageUrl"
          type="button"
          class="btn-text photo-remove"
          data-testid="image-remove"
          @click="form.imageUrl = ''; imageFailed = false"
        >✕ {{ $t('recipes.form.photoRemove') }}</button>
      </div>

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

      <label class="check-row">
        <input v-model="form.freezable" type="checkbox" name="freezable">
        {{ $t('recipes.form.freezable') }}
      </label>

      <div class="tags-block">
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
      </div>

      <label>
        {{ $t('recipes.form.notes') }}
        <textarea v-model="form.notes" v-autogrow name="notes" maxlength="5000" rows="3" class="grow" />
      </label>

      <p v-if="error" class="error" role="alert">{{ error }}</p>

      <div class="actions">
        <button type="submit" :disabled="pending" data-testid="recipe-submit">
          {{ mode === 'create' ? $t('recipes.form.submitCreate') : $t('recipes.form.submitEdit') }}
        </button>
        <NuxtLink
          :to="mode === 'edit' && recipe ? localePath(`/recettes/${recipe.id}`) : localePath('/recettes')"
          class="btn btn-text"
        >{{ $t('common.cancel') }}</NuxtLink>
      </div>
    </aside>
  </form>
</template>

<style scoped>
.recipe-form {
  display: grid;
  grid-template-columns: minmax(0, 1fr) 320px;
  gap: var(--hb-space-6);
  align-items: start;
}

.content {
  display: grid;
  gap: var(--hb-space-4);
  min-width: 0;
}

/* Panneau collant : Enregistrer reste visible pendant la relecture d'une
   longue liste d'ingrédients ou d'étapes. */
.side {
  position: sticky;
  top: var(--hb-space-4);
  display: grid;
  gap: var(--hb-space-4);
  border: 1px solid var(--md-sys-color-outline-variant);
  border-radius: var(--md-sys-shape-corner-large);
  background: var(--md-sys-color-surface-container-lowest);
  padding: var(--hb-space-4);
  min-width: 0;
}

@media (max-width: 860px) {
  .recipe-form {
    grid-template-columns: minmax(0, 1fr);
  }

  .side {
    position: static;
  }
}

.title-input {
  font: var(--md-sys-typescale-title-large);
}

/* Zones auto-ajustées (v-autogrow) : la hauteur suit le contenu, la poignée
   de redimensionnement n'a plus d'utilité. */
.grow {
  resize: none;
}

.panel {
  border: 1px solid var(--md-sys-color-outline-variant);
  border-radius: var(--md-sys-shape-corner-large);
  background: var(--md-sys-color-surface-container-lowest);
  padding: var(--hb-space-4);
  display: grid;
  gap: var(--hb-space-3);
  margin: 0;
  min-width: 0;
}

.panel legend {
  font: var(--md-sys-typescale-title-medium);
  padding-inline: var(--hb-space-1);
}

.panel .hint {
  margin: 0;
}

/* Le nom, seule donnée qu'on relit vraiment, prend toute la place restante. */
.ingredient-head,
.ingredient-row {
  display: grid;
  grid-template-columns: 72px 112px minmax(0, 1fr) 176px 44px;
  gap: var(--hb-space-2);
  align-items: start;
}

.ingredient-head {
  font: var(--md-sys-typescale-label-medium);
  color: var(--md-sys-color-on-surface-variant);
  padding-inline: var(--hb-space-1);
}

.name-input {
  min-height: 44px;
}

@media (max-width: 860px) {
  .ingredient-head {
    display: none;
  }

  /* Zones nommées : sans elles, le placement automatique glissait le rayon
     dans la colonne étroite du ✕. */
  .ingredient-row {
    grid-template-columns: 90px minmax(0, 1fr) 44px;
    grid-template-areas:
      "qty unit del"
      "name name name"
      "aisle aisle aisle";
    padding-bottom: var(--hb-space-3);
    border-bottom: 1px solid var(--md-sys-color-outline-variant);
  }

  .ingredient-row .field-qty { grid-area: qty; }
  .ingredient-row .field-unit { grid-area: unit; }
  .ingredient-row .ingredient-name { grid-area: name; }
  .ingredient-row .field-aisle { grid-area: aisle; }
  .ingredient-row .row-btn { grid-area: del; }
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
  display: grid;
  grid-template-columns: 32px minmax(0, 1fr) auto;
  gap: var(--hb-space-2);
  align-items: start;
}

.step-num {
  width: 32px;
  height: 32px;
  margin-top: 6px;
  border-radius: var(--md-sys-shape-corner-full);
  background: var(--md-sys-color-secondary-container);
  color: var(--md-sys-color-on-secondary-container);
  display: grid;
  place-items: center;
  font: var(--md-sys-typescale-title-small);
}

/* Actions empilées : elles ne volent plus de largeur au texte de l'étape. */
.step-actions {
  display: grid;
}

.row-btn {
  min-height: 44px;
  min-width: 44px;
  padding: 0;
}

/* À la souris, des boutons plus compacts suffisent ; au doigt on garde
   la cible tactile complète. */
@media (pointer: fine) {
  .step-actions .row-btn {
    min-height: 32px;
  }
}

.add-btn {
  justify-self: start;
}

.two-cols {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: var(--hb-space-3);
}

.narrow {
  max-width: 120px;
}

.photo-block {
  display: grid;
  gap: var(--hb-space-2);
  justify-items: start;
}

.photo-preview {
  width: 100%;
  aspect-ratio: 3 / 2;
  object-fit: cover;
  border-radius: var(--md-sys-shape-corner-medium);
  border: 1px solid var(--md-sys-color-outline-variant);
}

.photo-block label {
  width: 100%;
}

.photo-broken {
  color: var(--md-sys-color-error);
}

.photo-remove {
  min-height: 36px;
}

.tags-block {
  display: grid;
  gap: var(--hb-space-2);
}

.tags-block .hint {
  margin: 0;
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
  display: grid;
  gap: var(--hb-space-2);
  border-top: 1px solid var(--md-sys-color-outline-variant);
  padding-top: var(--hb-space-4);
}

.actions .btn {
  text-align: center;
}
</style>
