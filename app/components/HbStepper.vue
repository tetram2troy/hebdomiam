<script setup lang="ts">
const props = withDefaults(defineProps<{
  label: string
  min?: number
  max?: number
}>(), { min: 1, max: 9 })

const model = defineModel<number>({ required: true })

function step(delta: number) {
  model.value = Math.min(props.max, Math.max(props.min, model.value + delta))
}
</script>

<template>
  <div class="stepper" role="group" :aria-label="label">
    <button
      type="button"
      class="btn-outline step-btn"
      :disabled="model <= min"
      :aria-label="$t('common.decrease', { label })"
      @click="step(-1)"
    >−</button>
    <span class="val" aria-live="polite">{{ model }}</span>
    <button
      type="button"
      class="btn-outline step-btn"
      :disabled="model >= max"
      :aria-label="$t('common.increase', { label })"
      @click="step(1)"
    >+</button>
  </div>
</template>

<style scoped>
.stepper {
  display: inline-flex;
  align-items: center;
  gap: var(--hb-space-3);
}

.step-btn {
  width: var(--hb-touch-target);
  min-height: var(--hb-touch-target);
  padding: 0;
  font-size: 20px;
}

.val {
  font-family: var(--md-sys-typescale-font-brand);
  font-size: 24px;
  font-weight: 700;
  min-width: 36px;
  text-align: center;
  font-variant-numeric: tabular-nums;
}
</style>
