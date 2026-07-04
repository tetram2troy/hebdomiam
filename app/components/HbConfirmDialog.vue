<script setup lang="ts">
// Confirmation modale à la place de window.confirm : <dialog> natif
// (focus piégé, Échap et clic hors de la boîte ferment sans confirmer).
defineProps<{
  title: string
  body?: string
  confirmLabel: string
  danger?: boolean
}>()

const emit = defineEmits<{ confirm: [] }>()

const dialog = ref<HTMLDialogElement | null>(null)

function open() {
  dialog.value?.showModal()
}

function close() {
  dialog.value?.close()
}

function confirm() {
  emit('confirm')
  close()
}

defineExpose({ open, close })
</script>

<template>
  <dialog ref="dialog" class="confirm" data-testid="confirm-dialog" @click.self="close">
    <h2>{{ title }}</h2>
    <p v-if="body" class="body">{{ body }}</p>
    <div class="actions">
      <button type="button" class="btn-outline" data-testid="confirm-dialog-cancel" @click="close">
        {{ $t('common.cancel') }}
      </button>
      <button
        type="button"
        :class="danger ? 'btn-danger' : ''"
        data-testid="confirm-dialog-confirm"
        @click="confirm"
      >{{ confirmLabel }}</button>
    </div>
  </dialog>
</template>

<style scoped>
.confirm {
  border: none;
  border-radius: var(--md-sys-shape-corner-large);
  background: var(--md-sys-color-surface-container-lowest);
  color: var(--md-sys-color-on-surface);
  padding: var(--hb-space-5) var(--hb-space-6);
  max-width: min(440px, calc(100vw - var(--hb-space-8)));
  box-shadow: 0 8px 32px rgb(0 0 0 / 25%);
}

.confirm::backdrop {
  background: rgb(0 0 0 / 40%);
}

.confirm h2 {
  margin: 0 0 var(--hb-space-2);
  font: var(--md-sys-typescale-title-large);
}

.body {
  margin: 0;
  color: var(--md-sys-color-on-surface-variant);
}

.actions {
  display: flex;
  justify-content: flex-end;
  gap: var(--hb-space-3);
  margin-top: var(--hb-space-5);
}
</style>
