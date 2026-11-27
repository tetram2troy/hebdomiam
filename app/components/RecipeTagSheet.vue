<script setup lang="ts">
import type { TagSummary } from '#shared/types/recipe'

// « Tous les tags » de la liste de recettes : la rangée de filtres ne fait
// qu'une ligne défilante, ce panneau donne accès à tous les tags d'un coup,
// avec leur nombre de recettes et une recherche. Feuille basse sur mobile
// (atteignable au pouce), dialogue centré à partir de 768 px.
// <dialog> natif comme HbConfirmDialog : focus piégé, Échap ferme, et la
// couche supérieure passe au-dessus de la barre d'onglets.
const props = defineProps<{
  tags: TagSummary[]
  active?: string
}>()

const emit = defineEmits<{ select: [name: string] }>()

const dialog = ref<HTMLDialogElement | null>(null)
const query = ref('')

const visibleTags = computed(() => filterTags(props.tags, query.value))

function open() {
  query.value = ''
  dialog.value?.showModal()
}

function close() {
  dialog.value?.close()
}

function choose(name: string) {
  emit('select', name)
  close()
}

defineExpose({ open, close })
</script>

<template>
  <!-- Le clic sur le fond (cible = le dialog lui-même) ferme ; le contenu est
       dans .sheet-body pour qu'un clic dans la marge intérieure ne ferme pas. -->
  <dialog ref="dialog" class="sheet" :aria-label="$t('recipes.allTagsTitle')" data-testid="tag-sheet" @click.self="close">
    <div class="sheet-body">
      <div class="sheet-head">
        <h2>{{ $t('recipes.allTagsTitle') }}</h2>
        <button type="button" class="btn-text" data-testid="tag-sheet-close" @click="close">✕ {{ $t('common.close') }}</button>
      </div>
      <input
        v-model="query"
        type="search"
        :aria-label="$t('recipes.tagSearch')"
        :placeholder="$t('recipes.tagSearch')"
        data-testid="tag-sheet-search"
      >
      <p v-if="!visibleTags.length" class="meta none" role="status">{{ $t('recipes.noTagMatch') }}</p>
      <div v-else class="sheet-tags" role="group" :aria-label="$t('recipes.filters')">
        <button
          v-for="tag in visibleTags"
          :key="tag.name"
          type="button"
          class="chip chip-filter"
          :aria-pressed="active === tag.name"
          data-testid="tag-sheet-option"
          @click="choose(tag.name)"
        >
          {{ tag.name }} <span class="count">· {{ tag.recipeCount }}</span>
        </button>
      </div>
    </div>
  </dialog>
</template>

<style scoped>
.sheet {
  /* Ancrée en bas, pleine largeur : atteignable au pouce. */
  margin: auto 0 0;
  width: 100%;
  max-width: 100%;
  /* Hauteur fixe, pas « au contenu » : pendant la recherche la liste rétrécit,
     et une feuille qui se tasse en bas d'écran finirait sous le clavier du
     téléphone. Ainsi le champ et les premiers résultats restent en haut. */
  height: min(80dvh, 640px);
  max-height: none;
  overflow: hidden;
  padding: 0;
  border: none;
  border-radius: var(--md-sys-shape-corner-extra-large) var(--md-sys-shape-corner-extra-large) 0 0;
  background: var(--md-sys-color-surface-container-lowest);
  color: var(--md-sys-color-on-surface);
  box-shadow: var(--md-sys-elevation-level3);
}

.sheet::backdrop {
  background: var(--md-sys-color-scrim);
}

/* ≥ 768 px : dialogue centré — une feuille pleine largeur collée en bas
   d'un grand écran serait loin du bouton et inutilement large. */
@media (min-width: 768px) {
  .sheet {
    margin: auto;
    width: min(560px, calc(100vw - var(--hb-space-8)));
    height: min(70dvh, 560px);
    border-radius: var(--md-sys-shape-corner-extra-large);
  }
}

/* En-tête et recherche fixes ; seule la liste des tags défile. */
.sheet-body {
  display: flex;
  flex-direction: column;
  gap: var(--hb-space-3);
  height: 100%;
  padding: var(--hb-space-4) var(--hb-space-4) 0;
}

.sheet-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--hb-space-3);
}

.sheet-head h2 {
  margin: 0;
  font: var(--md-sys-typescale-title-large);
}

.sheet-tags {
  display: flex;
  flex-wrap: wrap;
  align-content: flex-start;
  gap: var(--hb-space-2);
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  /* Marge négative + padding : la place de l'anneau de focus, que le
     défilement rognerait sur les côtés. */
  margin: 0 calc(-1 * var(--hb-space-1));
  padding: var(--hb-space-1) var(--hb-space-1) calc(var(--hb-space-6) + env(safe-area-inset-bottom, 0px));
}

.count {
  color: var(--md-sys-color-on-surface-variant);
  font-variant-numeric: tabular-nums;
}

.none {
  margin: 0;
}
</style>
