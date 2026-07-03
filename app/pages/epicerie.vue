<script setup lang="ts">
import { AISLES, type Aisle } from '#shared/schemas/recipe'
import type { GroceryItemDto, GroceryListDto } from '#shared/types/grocery'

definePageMeta({ middleware: 'auth' })

const localePath = useLocalePath()
const { t, locale } = useI18n()

const { data } = await useFetch<{ list: GroceryListDto | null }>('/api/grocery-list/current')
const list = computed(() => data.value?.list ?? null)

const error = ref('')
const busyId = ref('')
const newLabel = ref('')
const adding = ref(false)
const editingId = ref('')

const editingItem = computed(() =>
  list.value?.items.find(i => i.id === editingId.value) ?? null)

const weekLabel = computed(() => {
  if (!list.value) return ''
  const [year, month, day] = list.value.weekStartDate.split('-').map(Number)
  return new Intl.DateTimeFormat(locale.value === 'fr' ? 'fr-CA' : 'en-CA', {
    day: 'numeric',
    month: 'long',
  }).format(new Date(year!, month! - 1, day!))
})

const numberFormat = computed(() =>
  new Intl.NumberFormat(locale.value === 'fr' ? 'fr-CA' : 'en-CA', { maximumFractionDigits: 2 }))

/** Rangées à acheter, groupées par rayon dans l'ordre du magasin. */
const aisleSections = computed(() => {
  if (!list.value) return []
  const sections = new Map<Aisle, GroceryItemDto[]>()
  for (const item of list.value.items) {
    if (item.checked) continue
    if (!sections.has(item.aisle)) sections.set(item.aisle, [])
    sections.get(item.aisle)!.push(item)
  }
  return [...sections.entries()].map(([aisle, items]) => ({ aisle, items }))
})

const checkedItems = computed(() => list.value?.items.filter(i => i.checked) ?? [])
const progressPct = computed(() =>
  list.value && list.value.totalCount > 0
    ? Math.round(100 * list.value.checkedCount / list.value.totalCount)
    : 0)

function formatQuantity(item: GroceryItemDto): string {
  if (item.quantity === null) return ''
  const value = numberFormat.value.format(item.quantity)
  if (item.unit === null || item.unit === 'unit') return value
  return `${value} ${t(`units.${item.unit}`)}`
}

function formatSource(source: GroceryItemDto['sources'][number]): string {
  const qty = numberFormat.value.format(source.quantity)
  const unit = source.unit && source.unit !== 'unit' ? ` ${t(`units.${source.unit}`)}` : ''
  return `${source.recipeTitle} (${qty}${unit})`
}

async function patchItem(item: GroceryItemDto, patch: Record<string, unknown>) {
  error.value = ''
  busyId.value = item.id
  try {
    const response = await $fetch<{ list: GroceryListDto }>(`/api/grocery-list/items/${item.id}`, {
      method: 'PATCH',
      body: patch,
    })
    data.value = response
  }
  catch (err) {
    error.value = errorMessage(err, t)
  }
  finally {
    busyId.value = ''
  }
}

function toggle(item: GroceryItemDto) {
  return patchItem(item, { checked: !item.checked })
}

async function addItem() {
  const label = newLabel.value.trim()
  if (!label) return
  error.value = ''
  adding.value = true
  try {
    const response = await $fetch<{ list: GroceryListDto }>('/api/grocery-list/items', {
      method: 'POST',
      body: { label },
    })
    data.value = response
    newLabel.value = ''
  }
  catch (err) {
    error.value = errorMessage(err, t)
  }
  finally {
    adding.value = false
  }
}

async function removeItem(item: GroceryItemDto) {
  error.value = ''
  try {
    const response = await $fetch<{ list: GroceryListDto }>(`/api/grocery-list/items/${item.id}`, {
      method: 'DELETE',
    })
    data.value = response
    editingId.value = ''
  }
  catch (err) {
    error.value = errorMessage(err, t)
  }
}
</script>

<template>
  <div class="store">
    <!-- Pas de plan validé cette semaine -->
    <div v-if="!list" class="empty" data-testid="no-list">
      <h1>{{ $t('grocery.emptyTitle') }}</h1>
      <p class="meta">{{ $t('grocery.emptyBody') }}</p>
      <NuxtLink :to="localePath('/planifier')" class="btn">{{ $t('grocery.goPlan') }}</NuxtLink>
    </div>

    <template v-else>
      <header class="shead">
        <h1>{{ $t('grocery.title') }} — {{ $t('home.weekOf', { date: weekLabel }) }}</h1>
        <div class="progress">
          <div class="bar" role="progressbar" :aria-valuenow="list.checkedCount" :aria-valuemin="0" :aria-valuemax="list.totalCount">
            <i :style="{ width: `${progressPct}%` }" />
          </div>
          <p class="lab" data-testid="progress" aria-live="polite">
            {{ $t('grocery.progress', { checked: list.checkedCount, total: list.totalCount }) }}
          </p>
        </div>
      </header>

      <p v-if="error" class="error" role="alert">{{ error }}</p>

      <form class="addfree" @submit.prevent="addItem">
        <input
          v-model="newLabel"
          type="text"
          maxlength="120"
          :aria-label="$t('grocery.addLabel')"
          :placeholder="$t('grocery.addPlaceholder')"
        >
        <button type="submit" :disabled="adding || !newLabel.trim()" data-testid="add-item">+</button>
      </form>

      <p v-if="list.totalCount > 0 && aisleSections.length === 0" class="all-done" data-testid="all-done">
        {{ $t('grocery.allDone') }}
      </p>

      <section v-for="section in aisleSections" :key="section.aisle" class="aisle-section">
        <h2 class="aisle" :data-testid="`aisle-${section.aisle}`">
          {{ $t(`aisles.${section.aisle}`) }}
          <span class="cnt">{{ $t('grocery.remaining', { n: section.items.length }, section.items.length) }}</span>
        </h2>
        <div
          v-for="item in section.items"
          :key="item.id"
          class="row"
          data-testid="grocery-item"
        >
          <button
            type="button"
            class="check"
            :disabled="busyId === item.id"
            :aria-label="$t('grocery.checkLabel', { label: item.label })"
            @click="toggle(item)"
          >
            <span class="box" aria-hidden="true" />
            <span class="lbl">{{ item.label }}<span v-if="item.isManual" class="chip chip-sm manual">{{ $t('grocery.manualBadge') }}</span></span>
            <span class="qty">{{ formatQuantity(item) }}</span>
          </button>
          <button
            type="button"
            class="btn-text edit"
            :aria-label="$t('grocery.editLabel', { label: item.label })"
            :aria-expanded="editingId === item.id"
            @click="editingId = editingId === item.id ? '' : item.id"
          >✎</button>
        </div>
      </section>

      <!-- Feuille de correction -->
      <div v-if="editingItem" class="sheet" role="dialog" :aria-label="$t('grocery.editLabel', { label: editingItem.label })" data-testid="edit-sheet">
        <div class="sheet-head">
          <h2>{{ editingItem.label }}<span v-if="formatQuantity(editingItem)"> — {{ formatQuantity(editingItem) }}</span></h2>
          <button type="button" class="btn-text" @click="editingId = ''">✕ {{ $t('grocery.close') }}</button>
        </div>
        <p v-if="editingItem.sources.length" class="meta">
          {{ $t('grocery.sources') }} {{ editingItem.sources.map(formatSource).join(' + ') }}
        </p>
        <label class="sheet-aisle">
          {{ $t('grocery.aisleLabel') }}
          <select
            :value="editingItem.aisle"
            data-testid="aisle-select"
            @change="patchItem(editingItem, { aisle: ($event.target as HTMLSelectElement).value })"
          >
            <option v-for="aisle in AISLES" :key="aisle" :value="aisle">{{ $t(`aisles.${aisle}`) }}</option>
          </select>
        </label>
        <button type="button" class="btn-danger" data-testid="remove-item" @click="removeItem(editingItem)">
          {{ $t('grocery.remove') }}
        </button>
      </div>

      <!-- Dans le panier -->
      <section v-if="checkedItems.length" class="cart">
        <h2 class="aisle cart-head">{{ $t('grocery.inCart', { n: checkedItems.length }) }}</h2>
        <div v-for="item in checkedItems" :key="item.id" class="row" data-testid="cart-item">
          <button
            type="button"
            class="check done"
            :disabled="busyId === item.id"
            :aria-label="$t('grocery.uncheckLabel', { label: item.label })"
            @click="toggle(item)"
          >
            <span class="box checked" aria-hidden="true">✓</span>
            <span class="lbl">{{ item.label }}</span>
            <span class="qty">{{ formatQuantity(item) }}</span>
          </button>
        </div>
      </section>
    </template>
  </div>
</template>

<style scoped>
/* Mode magasin : une main, pouce, colonne mobile même sur grand écran. */
.store {
  max-width: 520px;
  margin: 0 auto;
}

.empty {
  text-align: center;
  margin: var(--hb-space-12) auto;
  display: grid;
  gap: var(--hb-space-3);
  justify-items: center;
}

.shead h1 {
  font: var(--md-sys-typescale-headline-small);
  margin-bottom: var(--hb-space-2);
}

.progress .bar {
  height: 8px;
  border-radius: var(--md-sys-shape-corner-full);
  background: var(--md-sys-color-surface-container-high);
  overflow: hidden;
}

.progress .bar i {
  display: block;
  height: 100%;
  background: var(--md-sys-color-primary);
}

@media (prefers-reduced-motion: no-preference) {
  .progress .bar i {
    transition: width var(--md-sys-motion-duration-medium) var(--md-sys-motion-easing-standard);
  }
}

.progress .lab {
  font: var(--md-sys-typescale-label-large);
  color: var(--md-sys-color-on-surface-variant);
  margin: var(--hb-space-1) 0 0;
  font-variant-numeric: tabular-nums;
}

.addfree {
  display: flex;
  gap: var(--hb-space-2);
  margin: var(--hb-space-4) 0;
}

.addfree input {
  border-radius: var(--md-sys-shape-corner-full);
  padding-inline: var(--hb-space-4);
}

.addfree button {
  min-width: var(--hb-touch-target-store);
  min-height: var(--hb-touch-target-store);
  font-size: 22px;
  padding: 0;
  flex: none;
}

.all-done {
  color: var(--md-sys-color-primary);
  font-weight: 700;
}

.aisle {
  position: sticky;
  top: 0;
  z-index: 1;
  display: flex;
  justify-content: space-between;
  align-items: baseline;
  gap: var(--hb-space-3);
  background: var(--md-sys-color-surface);
  font: var(--md-sys-typescale-label-large);
  text-transform: uppercase;
  letter-spacing: 0.08em;
  color: var(--md-sys-color-on-surface-variant);
  padding: var(--hb-space-3) 0 var(--hb-space-1);
  margin: 0;
}

.aisle .cnt {
  font: var(--md-sys-typescale-label-medium);
  text-transform: none;
  letter-spacing: 0;
  font-variant-numeric: tabular-nums;
}

.row {
  display: flex;
  align-items: stretch;
  gap: var(--hb-space-1);
  border-bottom: 1px solid var(--md-sys-color-outline-variant);
}

.check {
  flex: 1;
  display: flex;
  align-items: center;
  gap: var(--hb-space-3);
  min-height: var(--hb-touch-target-store);
  background: transparent;
  border: none;
  border-radius: var(--md-sys-shape-corner-small);
  color: var(--md-sys-color-on-surface);
  font: var(--md-sys-typescale-title-medium);
  padding: var(--hb-space-2) var(--hb-space-1);
  text-align: left;
}

.check:hover {
  background: var(--md-sys-color-surface-container-low);
}

.box {
  flex: none;
  width: 28px;
  height: 28px;
  border-radius: var(--md-sys-shape-corner-medium);
  border: 2.5px solid var(--md-sys-color-outline);
  display: grid;
  place-items: center;
  font-size: 16px;
  font-weight: 800;
}

.box.checked {
  background: var(--md-sys-color-primary);
  border-color: var(--md-sys-color-primary);
  color: var(--md-sys-color-on-primary);
}

.lbl {
  flex: 1;
  min-width: 0;
  display: flex;
  align-items: center;
  gap: var(--hb-space-2);
  flex-wrap: wrap;
}

.manual {
  padding: 1px 8px;
  font: var(--md-sys-typescale-label-small);
}

.qty {
  font: var(--md-sys-typescale-body-large);
  font-weight: 700;
  color: var(--md-sys-color-on-surface-variant);
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
}

.edit {
  min-width: 44px;
  padding: 0;
  align-self: center;
  min-height: 44px;
}

.done .lbl {
  text-decoration: line-through;
  opacity: 0.55;
  font-weight: 400;
}

.done .qty {
  opacity: 0.55;
}

.sheet {
  border: 2px solid var(--md-sys-color-primary);
  border-radius: var(--md-sys-shape-corner-large);
  background: var(--md-sys-color-surface-container-lowest);
  padding: var(--hb-space-4);
  margin: var(--hb-space-4) 0;
  display: grid;
  gap: var(--hb-space-3);
  justify-items: start;
}

.sheet-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--hb-space-3);
  width: 100%;
}

.sheet-head h2 {
  margin: 0;
  font: var(--md-sys-typescale-title-large);
}

.sheet-aisle {
  width: 100%;
  max-width: 320px;
}

.cart {
  margin-top: var(--hb-space-6);
  border-top: 2px solid var(--md-sys-color-outline-variant);
}

.cart-head {
  color: var(--md-sys-color-primary);
}
</style>
