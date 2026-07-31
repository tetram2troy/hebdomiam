<script setup lang="ts">
import type { ProfileDto } from '#shared/types/settings'

definePageMeta({ middleware: 'auth' })

const localePath = useLocalePath()
const switchLocalePath = useSwitchLocalePath()
const { t } = useI18n()
const { clear } = useUserSession()
const { theme, apply: applyTheme } = useTheme()

const { data: profile } = await useFetch<ProfileDto>('/api/settings')

const error = ref('')
const notice = ref('')

async function patchProfile(patch: Record<string, unknown>) {
  error.value = ''
  notice.value = ''
  try {
    profile.value = await $fetch<ProfileDto>('/api/settings', { method: 'PATCH', body: patch })
    notice.value = t('settings.saved')
  }
  catch (err) {
    error.value = errorMessage(err, t)
  }
}

async function changeLanguage(locale: 'fr' | 'en') {
  await patchProfile({ locale })
  await navigateTo(switchLocalePath(locale), { external: false })
}

// ── Nom ─────────────────────────────────────────────────────────────────────
const editingName = ref(false)
const nameDraft = ref('')

function startNameEdit() {
  nameDraft.value = profile.value?.name ?? ''
  editingName.value = true
}

async function saveName() {
  await patchProfile({ name: nameDraft.value.trim() })
  editingName.value = false
}

// ── Vérification du courriel ────────────────────────────────────────────────
const verifySending = ref(false)

async function resendVerify() {
  verifySending.value = true
  error.value = ''
  try {
    await $fetch('/api/auth/verify/request', { method: 'POST' })
    notice.value = t('settings.verifySent')
  }
  catch (err) {
    error.value = errorMessage(err, t)
  }
  finally {
    verifySending.value = false
  }
}

// ── Mot de passe ────────────────────────────────────────────────────────────
const showPasswordForm = ref(false)
const currentPassword = ref('')
const newPassword = ref('')

async function changePassword() {
  error.value = ''
  try {
    await $fetch('/api/settings/password', {
      method: 'POST',
      body: { currentPassword: currentPassword.value, newPassword: newPassword.value },
    })
    notice.value = t('settings.passwordChanged')
    showPasswordForm.value = false
    currentPassword.value = ''
    newPassword.value = ''
  }
  catch (err) {
    error.value = errorMessage(err, t)
  }
}

// ── 2FA TOTP ────────────────────────────────────────────────────────────────
const totpSetup = ref<{ secret: string, qrSvg: string } | null>(null)
const totpCode = ref('')
const totpBackup = ref('')
const showTotpDisable = ref(false)
const totpDisablePassword = ref('')

async function startTotp() {
  error.value = ''
  try {
    totpSetup.value = await $fetch<{ secret: string, otpauthUrl: string, qrSvg: string }>('/api/2fa/setup', { method: 'POST' })
  }
  catch (err) {
    error.value = errorMessage(err, t)
  }
}

async function confirmTotp() {
  error.value = ''
  try {
    const { backupCode } = await $fetch<{ backupCode: string }>('/api/2fa/activate', {
      method: 'POST',
      body: { code: totpCode.value },
    })
    totpBackup.value = backupCode
    totpSetup.value = null
    totpCode.value = ''
    if (profile.value) profile.value = { ...profile.value, totpEnabled: true }
  }
  catch (err) {
    error.value = errorMessage(err, t)
  }
}

async function disableTotp() {
  error.value = ''
  try {
    await $fetch('/api/2fa/disable', { method: 'POST', body: { password: totpDisablePassword.value } })
    showTotpDisable.value = false
    totpDisablePassword.value = ''
    totpBackup.value = ''
    if (profile.value) profile.value = { ...profile.value, totpEnabled: false }
    notice.value = t('settings.saved')
  }
  catch (err) {
    error.value = errorMessage(err, t)
  }
}

// ── Déconnexion et suppression ──────────────────────────────────────────────
async function logout() {
  await $fetch('/api/auth/logout', { method: 'POST' })
  await clear()
  await navigateTo(localePath('/login'))
}

const showDelete = ref(false)
const deletePassword = ref('')

async function deleteAccount() {
  error.value = ''
  try {
    await $fetch('/api/account', { method: 'DELETE', body: { password: deletePassword.value } })
    await clear()
    // Vers la connexion : l'inscription publique est fermée, /register est un cul-de-sac.
    await navigateTo(localePath('/login'))
  }
  catch (err) {
    error.value = errorMessage(err, t)
  }
}
</script>

<template>
  <div v-if="profile" class="settings">
    <h1>{{ $t('settings.title') }}</h1>
    <p v-if="error" class="error" role="alert">{{ error }}</p>
    <p v-if="notice" class="notice" role="status" data-testid="settings-notice">{{ notice }}</p>

    <!-- Profil -->
    <section class="panel">
      <h2>{{ $t('settings.profile') }}</h2>
      <div class="srow">
        <span class="lab"><b>{{ $t('settings.nameLabel') }}</b><span class="hint">{{ $t('settings.nameHint') }}</span></span>
        <template v-if="editingName">
          <input v-model="nameDraft" type="text" maxlength="100" class="name-input" :aria-label="$t('settings.nameLabel')">
          <button type="button" class="btn-tonal" data-testid="save-name" @click="saveName">{{ $t('common.save') }}</button>
        </template>
        <template v-else>
          <span class="meta" data-testid="profile-name">{{ profile.name }}</span>
          <button type="button" class="btn-outline small" @click="startNameEdit">{{ $t('common.edit') }}</button>
        </template>
      </div>
      <div class="srow">
        <span class="lab"><b>{{ $t('settings.emailLabel') }}</b><span class="hint">{{ profile.email }}</span></span>
        <span v-if="profile.emailVerifiedAt" class="chip">{{ $t('settings.verified') }}</span>
        <template v-else>
          <span class="chip warn">{{ $t('settings.unverified') }}</span>
          <button type="button" class="btn-outline small" :disabled="verifySending" data-testid="resend-verify" @click="resendVerify">
            {{ $t('settings.resendVerify') }}
          </button>
        </template>
      </div>
    </section>

    <!-- Préférences -->
    <section class="panel">
      <h2>{{ $t('settings.preferences') }}</h2>
      <div class="srow">
        <span class="lab"><b>{{ $t('settings.language') }}</b><span class="hint">{{ $t('settings.languageHint') }}</span></span>
        <div class="seg" role="group" :aria-label="$t('settings.language')">
          <button type="button" :class="{ on: profile.locale === 'fr' }" @click="changeLanguage('fr')">Français</button>
          <button type="button" :class="{ on: profile.locale === 'en' }" data-testid="lang-en" @click="changeLanguage('en')">English</button>
        </div>
      </div>
      <div class="srow">
        <span class="lab"><b>{{ $t('settings.units') }}</b><span class="hint">{{ $t('settings.unitsHint') }}</span></span>
        <div class="seg" role="group" :aria-label="$t('settings.units')">
          <button type="button" :class="{ on: profile.unitSystem === 'metric' }" data-testid="units-metric" @click="patchProfile({ unitSystem: 'metric' })">{{ $t('settings.metric') }}</button>
          <button type="button" :class="{ on: profile.unitSystem === 'imperial' }" data-testid="units-imperial" @click="patchProfile({ unitSystem: 'imperial' })">{{ $t('settings.imperial') }}</button>
        </div>
      </div>
      <div class="srow">
        <span class="lab"><b>{{ $t('settings.theme') }}</b><span class="hint">{{ $t('settings.themeHint') }}</span></span>
        <div class="seg" role="group" :aria-label="$t('settings.theme')">
          <button type="button" :class="{ on: theme === 'system' }" @click="applyTheme('system')">{{ $t('settings.themeSystem') }}</button>
          <button type="button" :class="{ on: theme === 'light' }" @click="applyTheme('light')">{{ $t('settings.themeLight') }}</button>
          <button type="button" :class="{ on: theme === 'dark' }" data-testid="theme-dark" @click="applyTheme('dark')">{{ $t('settings.themeDark') }}</button>
        </div>
      </div>
    </section>

    <!-- Planification -->
    <section class="panel">
      <h2>{{ $t('settings.planning') }}</h2>
      <div class="srow">
        <span class="lab"><b>{{ $t('settings.rotation') }}</b><span class="hint">{{ $t('settings.rotationHint') }}</span></span>
        <HbStepper
          :model-value="profile.rotationWeeks"
          :label="$t('settings.rotation')"
          :min="1"
          :max="12"
          @update:model-value="value => patchProfile({ rotationWeeks: value })"
        />
        <span class="meta">{{ $t('settings.weeks') }}</span>
      </div>
      <div class="srow">
        <span class="lab"><b>{{ $t('settings.defaultDishes') }}</b></span>
        <HbStepper
          :model-value="profile.defaultDishCount"
          :label="$t('settings.defaultDishes')"
          :min="1"
          :max="10"
          @update:model-value="value => patchProfile({ defaultDishCount: value })"
        />
      </div>
      <div class="srow">
        <span class="lab"><b>{{ $t('settings.defaultPeople') }}</b></span>
        <HbStepper
          :model-value="profile.defaultPeopleCount"
          :label="$t('settings.defaultPeople')"
          :min="1"
          :max="12"
          @update:model-value="value => patchProfile({ defaultPeopleCount: value })"
        />
      </div>
      <div class="srow">
        <span class="lab"><b>{{ $t('settings.defaultDays') }}</b></span>
        <HbStepper
          :model-value="profile.defaultDaysCount"
          :label="$t('settings.defaultDays')"
          :min="1"
          :max="7"
          @update:model-value="value => patchProfile({ defaultDaysCount: value })"
        />
      </div>
    </section>

    <!-- Sécurité -->
    <section class="panel">
      <h2>{{ $t('settings.security') }}</h2>
      <div class="srow">
        <span class="lab"><b>{{ $t('settings.password') }}</b></span>
        <button type="button" class="btn-outline small" data-testid="change-password" @click="showPasswordForm = !showPasswordForm">
          {{ $t('settings.passwordChange') }}
        </button>
      </div>
      <form v-if="showPasswordForm" class="subform" @submit.prevent="changePassword">
        <label>
          {{ $t('settings.currentPassword') }}
          <input v-model="currentPassword" type="password" autocomplete="current-password" required>
        </label>
        <label>
          {{ $t('settings.newPassword') }}
          <input v-model="newPassword" type="password" autocomplete="new-password" required minlength="12">
        </label>
        <button type="submit" data-testid="submit-password">{{ $t('common.save') }}</button>
      </form>

      <div class="srow">
        <span class="lab"><b>{{ $t('settings.totp') }}</b><span class="hint">{{ profile.totpEnabled ? $t('settings.totpOn') : $t('settings.totpOff') }}</span></span>
        <button v-if="!profile.totpEnabled" type="button" class="btn-outline small" data-testid="totp-enable" @click="startTotp">
          {{ $t('settings.totpEnable') }}
        </button>
        <button v-else type="button" class="btn-outline small" data-testid="totp-disable" @click="showTotpDisable = !showTotpDisable">
          {{ $t('settings.totpDisable') }}
        </button>
      </div>
      <div v-if="totpSetup" class="subform" data-testid="totp-setup">
        <p class="meta">{{ $t('settings.totpScan') }}</p>
        <!-- SVG généré côté serveur par la librairie qrcode (contenu de confiance) -->
        <!-- eslint-disable-next-line vue/no-v-html -->
        <div class="qr" v-html="totpSetup.qrSvg" />
        <p class="meta secret" data-testid="totp-secret">{{ $t('settings.totpSecret', { secret: totpSetup.secret }) }}</p>
        <form @submit.prevent="confirmTotp">
          <label>
            {{ $t('auth.totpCode') }}
            <input v-model="totpCode" type="text" inputmode="numeric" autocomplete="one-time-code" required minlength="6" maxlength="16" class="name-input">
          </label>
          <button type="submit" data-testid="totp-confirm">{{ $t('settings.totpConfirm') }}</button>
        </form>
      </div>
      <p v-if="totpBackup" class="backup" data-testid="totp-backup">{{ $t('settings.totpBackup', { code: totpBackup }) }}</p>
      <form v-if="showTotpDisable" class="subform" @submit.prevent="disableTotp">
        <p class="meta">{{ $t('settings.totpDisableBody') }}</p>
        <label>
          {{ $t('auth.password') }}
          <input v-model="totpDisablePassword" type="password" autocomplete="current-password" required>
        </label>
        <button type="submit" class="btn-danger" data-testid="totp-disable-confirm">{{ $t('settings.totpDisable') }}</button>
      </form>

      <div class="srow">
        <span class="lab"><b>{{ $t('settings.logout') }}</b><span class="hint">{{ $t('settings.logoutHint') }}</span></span>
        <button type="button" class="btn-outline small" @click="logout">{{ $t('auth.logout') }}</button>
      </div>
    </section>

    <!-- Zone sensible -->
    <section class="panel danger">
      <h2>{{ $t('settings.danger') }}</h2>
      <div class="srow">
        <span class="lab"><b>{{ $t('settings.deleteAccount') }}</b><span class="hint">{{ $t('settings.deleteHint') }}</span></span>
        <button type="button" class="btn-danger small" data-testid="delete-account" @click="showDelete = !showDelete">
          {{ $t('settings.deleteAccount') }}…
        </button>
      </div>
      <form v-if="showDelete" class="subform" @submit.prevent="deleteAccount">
        <p class="meta">{{ $t('settings.deleteConfirmBody') }}</p>
        <label>
          {{ $t('auth.password') }}
          <input v-model="deletePassword" type="password" autocomplete="current-password" required>
        </label>
        <button type="submit" class="btn-danger" data-testid="delete-account-confirm">{{ $t('settings.deleteSubmit') }}</button>
      </form>
    </section>
  </div>
</template>

<style scoped>
.settings {
  max-width: 760px;
  display: grid;
  gap: var(--hb-space-4);
}

.settings h1 {
  margin: 0;
}

.notice {
  color: var(--md-sys-color-primary);
  font-weight: 700;
  margin: 0;
}

.panel {
  background: var(--md-sys-color-surface-container-lowest);
  border: 1px solid var(--md-sys-color-outline-variant);
  border-radius: var(--md-sys-shape-corner-large);
  padding: var(--hb-space-4) var(--hb-space-5);
}

.panel.danger {
  border-color: var(--md-sys-color-error);
}

.panel h2 {
  font: var(--md-sys-typescale-label-large);
  text-transform: uppercase;
  letter-spacing: 0.08em;
  color: var(--md-sys-color-on-surface-variant);
  margin: 0 0 var(--hb-space-2);
}

.panel.danger h2 {
  color: var(--md-sys-color-error);
}

.srow {
  display: flex;
  align-items: center;
  gap: var(--hb-space-4);
  padding: var(--hb-space-3) 0;
  border-top: 1px solid var(--md-sys-color-surface-container-high);
  min-height: 56px;
  flex-wrap: wrap;
}

.srow:first-of-type {
  border-top: none;
}

.lab {
  flex: 1;
  min-width: 220px;
}

.lab b {
  display: block;
  font: var(--md-sys-typescale-title-medium);
}

.lab .hint {
  display: block;
}

.seg {
  display: inline-flex;
  border: 2px solid var(--md-sys-color-outline);
  border-radius: var(--md-sys-shape-corner-full);
  overflow: hidden;
}

.seg button {
  border: none;
  border-radius: 0;
  background: transparent;
  color: var(--md-sys-color-on-surface-variant);
  min-height: 42px;
  padding: 8px 16px;
  font: var(--md-sys-typescale-label-large);
}

.seg button.on {
  background: var(--md-sys-color-primary);
  color: var(--md-sys-color-on-primary);
}

.small {
  min-height: 40px;
  padding: 6px 16px;
}

.name-input {
  max-width: 220px;
}

.chip.warn {
  background: var(--md-sys-color-tertiary-container);
  color: var(--md-sys-color-on-tertiary-container);
}

.subform {
  display: grid;
  gap: var(--hb-space-3);
  background: var(--md-sys-color-surface-container-low);
  border-radius: var(--md-sys-shape-corner-medium);
  padding: var(--hb-space-4);
  margin: var(--hb-space-2) 0;
  justify-items: start;
}

.subform label {
  width: 100%;
  max-width: 360px;
}

.qr {
  background: white;
  border-radius: var(--md-sys-shape-corner-medium);
  padding: var(--hb-space-2);
  width: 236px;
}

.secret {
  overflow-wrap: anywhere;
}

.backup {
  background: var(--md-sys-color-tertiary-container);
  color: var(--md-sys-color-on-tertiary-container);
  border-radius: var(--md-sys-shape-corner-medium);
  padding: var(--hb-space-3) var(--hb-space-4);
  font-weight: 600;
}
</style>
