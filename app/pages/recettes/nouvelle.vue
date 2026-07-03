<script setup lang="ts">
import type { ImportDraft } from '#shared/types/import'

definePageMeta({ middleware: 'auth' })

// Brouillon déposé par /recettes/importer — consommé une seule fois.
const draftState = useState<ImportDraft | null>('import-draft', () => null)
const draft = draftState.value
draftState.value = null
</script>

<template>
  <div class="form-page">
    <div v-if="draft" class="draftbar" data-testid="draft-banner">
      <p class="draft-main">⚠ {{ $t('import.draftBanner') }}</p>
      <p v-if="draft.sourceUrl" class="draft-source">{{ $t('import.draftSource', { url: draft.sourceUrl }) }}</p>
      <div v-if="draft.warnings.length" class="draft-warnings" data-testid="draft-warnings">
        <b>{{ $t('import.warningsTitle') }}</b>
        <ul>
          <li v-for="warning in draft.warnings" :key="warning">{{ warning }}</li>
        </ul>
      </div>
    </div>

    <h1>{{ $t('recipes.form.createTitle') }}</h1>
    <RecipeForm mode="create" :draft="draft" />
  </div>
</template>

<style scoped>
.form-page {
  max-width: 960px;
}

.draftbar {
  background: var(--md-sys-color-tertiary-container);
  color: var(--md-sys-color-on-tertiary-container);
  border-radius: var(--md-sys-shape-corner-medium);
  padding: var(--hb-space-4);
  margin-bottom: var(--hb-space-4);
  font: var(--md-sys-typescale-body-medium);
}

.draft-main {
  font-weight: 700;
  margin: 0;
}

.draft-source {
  margin: var(--hb-space-1) 0 0;
  overflow-wrap: anywhere;
}

.draft-warnings {
  margin-top: var(--hb-space-2);
}

.draft-warnings ul {
  margin: var(--hb-space-1) 0 0;
  padding-left: var(--hb-space-5);
}
</style>
