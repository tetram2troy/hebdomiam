// Miroir de api/src/repo/api_keys.rs.

export interface ApiKeyDto {
  id: string
  name: string
  /** Début de la clé (« hbd_ab12cd34 ») — seul fragment réaffichable. */
  prefix: string
  createdAt: string
  lastUsedAt: string | null
}

/** Réponse de création : `token` n'est rendu qu'une seule fois. */
export interface NewApiKey {
  key: ApiKeyDto
  token: string
}
