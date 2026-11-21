-- Clés d'API personnelles : un outil externe (Claude Code) enregistre une
-- recette au nom du compte sans cookie de session.
--
-- Seule l'empreinte SHA-256 de la clé est stockée : la clé (256 bits d'aléa)
-- n'est montrée qu'une fois, à la création. Un hachage lent (Argon2) n'apporte
-- rien face à un secret de cette entropie, et l'empreinte rapide permet de
-- retrouver la clé par index unique à chaque appel.
CREATE TABLE "api_keys" (
  "id" uuid PRIMARY KEY DEFAULT gen_random_uuid() NOT NULL,
  "user_id" uuid NOT NULL REFERENCES "users"("id") ON DELETE CASCADE,
  "name" text NOT NULL,
  -- Début de la clé (« hbd_ab12cd34 ») : seul fragment réaffichable, pour
  -- reconnaître une clé dans la liste sans en révéler le secret.
  "prefix" text NOT NULL,
  "token_hash" text NOT NULL UNIQUE,
  "created_at" timestamp with time zone DEFAULT now() NOT NULL,
  "last_used_at" timestamp with time zone
);

CREATE INDEX "api_keys_user_id_idx" ON "api_keys" ("user_id");
