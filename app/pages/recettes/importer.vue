<script setup lang="ts">
import type { ImportDraft } from '#shared/types/import'

definePageMeta({ middleware: 'auth' })

const localePath = useLocalePath()
const { t } = useI18n()

const mode = ref<'url' | 'photo'>('url')
const url = ref('')
const files = ref<File[]>([])
const pending = ref(false)
const error = ref('')

const draftState = useState<ImportDraft | null>('import-draft', () => null)

async function toDraft(request: Promise<{ draft: ImportDraft }>) {
  error.value = ''
  pending.value = true
  try {
    const { draft } = await request
    draftState.value = draft
    await navigateTo(localePath('/recettes/nouvelle'))
  }
  catch (err) {
    error.value = errorMessage(err, t)
  }
  finally {
    pending.value = false
  }
}

function importUrl() {
  if (!url.value.trim()) return
  return toDraft($fetch<{ draft: ImportDraft }>('/api/import/url', {
    method: 'POST',
    body: { url: url.value.trim() },
  }))
}

function onFilesPicked(event: Event) {
  const input = event.target as HTMLInputElement
  files.value = [...(input.files ?? [])].slice(0, 3)
}

function importPhotos() {
  if (files.value.length === 0) return
  const form = new FormData()
  for (const file of files.value) form.append('photos', file)
  return toDraft($fetch<{ draft: ImportDraft }>('/api/import/photo', {
    method: 'POST',
    body: form,
  }))
}
</script>

<template>
  <div class="import-page">
    <h1>{{ $t('import.title') }}</h1>

    <div class="modes" role="tablist">
      <button
        type="button"
        class="chip chip-filter"
        role="tab"
        :aria-selected="mode === 'url'"
        :aria-pressed="mode === 'url'"
        data-testid="tab-url"
        @click="mode = 'url'"
      >🔗 {{ $t('import.tabUrl') }}</button>
      <button
        type="button"
        class="chip chip-filter"
        role="tab"
        :aria-selected="mode === 'photo'"
        :aria-pressed="mode === 'photo'"
        data-testid="tab-photo"
        @click="mode = 'photo'"
      >📷 {{ $t('import.tabPhoto') }}</button>
    </div>

    <p v-if="error" class="error" role="alert">{{ error }}</p>
    <p v-if="pending" class="meta" aria-live="polite">{{ $t('import.extracting') }}</p>

    <form v-if="mode === 'url'" class="panel" @submit.prevent="importUrl">
      <label>
        {{ $t('import.urlLabel') }}
        <input
          v-model="url"
          type="url"
          required
          maxlength="2000"
          :placeholder="$t('import.urlPlaceholder')"
          data-testid="import-url"
        >
      </label>
      <p class="hint">{{ $t('import.urlHint') }}</p>
      <button type="submit" :disabled="pending" data-testid="extract-url">
        {{ $t('import.extract') }}
      </button>
    </form>

    <form v-else class="panel" @submit.prevent="importPhotos">
      <label>
        {{ $t('import.photoLabel') }}
        <input
          type="file"
          accept="image/jpeg,image/png,image/webp"
          multiple
          data-testid="import-photos"
          @change="onFilesPicked"
        >
      </label>
      <p class="hint">{{ $t('import.photoHint') }}</p>
      <button type="submit" :disabled="pending || files.length === 0" data-testid="extract-photos">
        {{ $t('import.extract') }}
      </button>
    </form>

    <NuxtLink :to="localePath('/recettes/nouvelle')" class="btn btn-text">
      {{ $t('import.manual') }}
    </NuxtLink>
  </div>
</template>

<style scoped>
.import-page {
  /* Assez large pour lire une URL de recette entière. */
  max-width: 860px;
  display: grid;
  gap: var(--hb-space-4);
  justify-items: start;
}

.modes {
  display: flex;
  gap: var(--hb-space-2);
}

.panel {
  width: 100%;
  background: var(--md-sys-color-surface-container-lowest);
  border: 1px solid var(--md-sys-color-outline-variant);
  border-radius: var(--md-sys-shape-corner-large);
  padding: var(--hb-space-5);
  justify-items: start;
}

.panel button[type='submit'] {
  min-height: var(--hb-touch-target-store);
  padding-inline: var(--hb-space-8);
}
</style>
