<script setup lang="ts">
import type { AdminUserDto } from '#shared/types/admin'

definePageMeta({ middleware: ['auth', 'admin'] })

const { t, locale } = useI18n()
const { user: me } = useUserSession()

const { data: userList, refresh } = await useFetch<AdminUserDto[]>('/api/admin/users')

const error = ref('')
const notice = ref('')

function dateLabel(iso: string): string {
  return new Intl.DateTimeFormat(locale.value === 'fr' ? 'fr-CA' : 'en-CA', { dateStyle: 'medium' }).format(new Date(iso))
}

// ── Création d'un compte (invitation) ───────────────────────────────────────
const newName = ref('')
const newEmail = ref('')
const creating = ref(false)
const inviteUrl = ref('')
const copied = ref(false)

async function createUser() {
  error.value = ''
  notice.value = ''
  inviteUrl.value = ''
  copied.value = false
  creating.value = true
  try {
    const result = await $fetch<{ user: AdminUserDto, inviteUrl: string }>('/api/admin/users', {
      method: 'POST',
      body: { name: newName.value, email: newEmail.value },
    })
    inviteUrl.value = result.inviteUrl
    newName.value = ''
    newEmail.value = ''
    notice.value = t('admin.created')
    await refresh()
  }
  catch (err) {
    error.value = errorMessage(err, t)
  }
  finally {
    creating.value = false
  }
}

async function copyInvite() {
  try {
    await navigator.clipboard.writeText(inviteUrl.value)
    copied.value = true
  }
  catch {
    // Presse-papiers indisponible (permissions, HTTP) : l'URL reste sélectionnable.
  }
}

// ── Actions sur un compte ────────────────────────────────────────────────────
async function patchUser(target: AdminUserDto, patch: { role?: 'admin' | 'user', disabled?: boolean }) {
  error.value = ''
  notice.value = ''
  try {
    await $fetch(`/api/admin/users/${target.id}`, { method: 'PATCH', body: patch })
    notice.value = t('admin.saved')
    await refresh()
  }
  catch (err) {
    error.value = errorMessage(err, t)
  }
}

async function resendInvite(target: AdminUserDto) {
  error.value = ''
  notice.value = ''
  copied.value = false
  try {
    const result = await $fetch<{ inviteUrl: string }>(`/api/admin/users/${target.id}/invite`, { method: 'POST' })
    inviteUrl.value = result.inviteUrl
    notice.value = t('admin.inviteResent')
  }
  catch (err) {
    error.value = errorMessage(err, t)
  }
}

const deleteDialog = ref<{ open: () => void } | null>(null)
const deleteTarget = ref<AdminUserDto | null>(null)

function askDelete(target: AdminUserDto) {
  deleteTarget.value = target
  deleteDialog.value?.open()
}

async function confirmDelete() {
  if (!deleteTarget.value) return
  error.value = ''
  notice.value = ''
  try {
    await $fetch(`/api/admin/users/${deleteTarget.value.id}`, { method: 'DELETE' })
    notice.value = t('admin.deleted')
    await refresh()
  }
  catch (err) {
    error.value = errorMessage(err, t)
  }
  finally {
    deleteTarget.value = null
  }
}
</script>

<template>
  <div class="admin">
    <h1>{{ $t('admin.title') }}</h1>
    <p v-if="error" class="error" role="alert">{{ error }}</p>
    <p v-if="notice" class="notice" role="status" data-testid="admin-notice">{{ notice }}</p>

    <!-- Créer un compte -->
    <section class="panel">
      <h2>{{ $t('admin.createTitle') }}</h2>
      <p class="hint">{{ $t('admin.createHint') }}</p>
      <form class="create" @submit.prevent="createUser">
        <label>
          {{ $t('auth.name') }}
          <input v-model="newName" type="text" maxlength="100" required data-testid="admin-create-name">
        </label>
        <label>
          {{ $t('auth.email') }}
          <input v-model="newEmail" type="email" required data-testid="admin-create-email">
        </label>
        <button type="submit" :disabled="creating" data-testid="admin-create-submit">{{ $t('admin.createSubmit') }}</button>
      </form>
      <div v-if="inviteUrl" class="invite" data-testid="admin-invite">
        <p class="meta">{{ $t('admin.inviteUrlLabel') }}</p>
        <div class="invite-row">
          <input
            :value="inviteUrl"
            readonly
            data-testid="admin-invite-url"
            :aria-label="$t('admin.inviteUrlLabel')"
            @focus="($event.target as HTMLInputElement).select()"
          >
          <button type="button" class="btn-tonal" data-testid="admin-invite-copy" @click="copyInvite">
            {{ copied ? $t('admin.copied') : $t('admin.copy') }}
          </button>
        </div>
      </div>
    </section>

    <!-- Comptes -->
    <section class="panel">
      <h2>{{ $t('admin.usersTitle') }}</h2>
      <div v-for="account in userList" :key="account.id" class="srow" data-testid="admin-user-row">
        <span class="lab">
          <b>
            {{ account.name }}
            <span v-if="account.id === me?.id" class="chip">{{ $t('admin.you') }}</span>
            <span v-else-if="account.role === 'admin'" class="chip">{{ $t('admin.roleAdmin') }}</span>
            <span v-if="account.disabled" class="chip warn">{{ $t('admin.disabled') }}</span>
            <span v-else-if="!account.emailVerified" class="chip warn">{{ $t('admin.invitePending') }}</span>
          </b>
          <span class="hint">{{ account.email }} · {{ $t('admin.since', { date: dateLabel(account.createdAt) }) }}</span>
        </span>
        <!-- Jamais d'action sur son propre compte : il se gère dans les réglages
             (et le serveur refuse de toute façon, garde « self »). -->
        <span v-if="account.id !== me?.id" class="actions">
          <button
            v-if="!account.emailVerified && !account.disabled"
            type="button"
            class="btn-outline small"
            data-testid="admin-row-resend"
            @click="resendInvite(account)"
          >{{ $t('admin.resendInvite') }}</button>
          <!-- Promotion réservée aux comptes activés : un admin qui ne peut pas
               se connecter n'est pas une relève (garde lastAdmin côté serveur). -->
          <button
            v-if="account.emailVerified"
            type="button"
            class="btn-outline small"
            data-testid="admin-row-role"
            @click="patchUser(account, { role: account.role === 'admin' ? 'user' : 'admin' })"
          >{{ account.role === 'admin' ? $t('admin.demote') : $t('admin.promote') }}</button>
          <button
            type="button"
            class="btn-outline small"
            data-testid="admin-row-disable"
            @click="patchUser(account, { disabled: !account.disabled })"
          >{{ account.disabled ? $t('admin.enable') : $t('admin.disable') }}</button>
          <button
            type="button"
            class="btn-danger small"
            data-testid="admin-row-delete"
            @click="askDelete(account)"
          >{{ $t('common.delete') }}</button>
        </span>
      </div>
    </section>

    <HbConfirmDialog
      ref="deleteDialog"
      :title="$t('admin.deleteTitle')"
      :body="$t('admin.deleteBody', { name: deleteTarget?.name ?? '' })"
      :confirm-label="$t('admin.deleteConfirm')"
      danger
      @confirm="confirmDelete"
    />
  </div>
</template>

<style scoped>
.admin {
  max-width: 760px;
  display: grid;
  gap: var(--hb-space-4);
}

.admin h1 {
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

.panel h2 {
  font: var(--md-sys-typescale-label-large);
  text-transform: uppercase;
  letter-spacing: 0.08em;
  color: var(--md-sys-color-on-surface-variant);
  margin: 0 0 var(--hb-space-2);
}

.create {
  display: flex;
  align-items: end;
  gap: var(--hb-space-3);
  flex-wrap: wrap;
  margin-top: var(--hb-space-3);
}

.create label {
  flex: 1;
  min-width: 200px;
}

.invite {
  background: var(--md-sys-color-surface-container-low);
  border-radius: var(--md-sys-shape-corner-medium);
  padding: var(--hb-space-3) var(--hb-space-4);
  margin-top: var(--hb-space-3);
  display: grid;
  gap: var(--hb-space-2);
}

.invite-row {
  display: flex;
  gap: var(--hb-space-2);
  flex-wrap: wrap;
}

.invite-row input {
  flex: 1;
  min-width: 220px;
  font: var(--md-sys-typescale-body-small);
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
  display: flex;
  align-items: center;
  gap: var(--hb-space-2);
  flex-wrap: wrap;
  font: var(--md-sys-typescale-title-medium);
}

.lab .hint {
  display: block;
}

.actions {
  display: flex;
  gap: var(--hb-space-2);
  flex-wrap: wrap;
}

.small {
  min-height: 40px;
  padding: 6px 16px;
}

.chip.warn {
  background: var(--md-sys-color-tertiary-container);
  color: var(--md-sys-color-on-tertiary-container);
}
</style>
