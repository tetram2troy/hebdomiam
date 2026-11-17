-- Durcissement de l'authentification (revue de sécurité avant fusion).

-- Révocation des sessions scellées : chaque cookie porte l'époque du compte au
-- moment de son scellement. L'incrémenter (changement ou réinitialisation de
-- mot de passe, 2FA activée ou désactivée, déconnexion) invalide d'un coup
-- tous les cookies antérieurs, y compris un cookie volé.
ALTER TABLE "users" ADD COLUMN "session_epoch" integer DEFAULT 0 NOT NULL;

-- Invitation en attente : distincte de « courriel non vérifié ». Sans cette
-- colonne, tout compte au courriel non vérifié (ex. l'admin de bootstrap)
-- pouvait recevoir une nouvelle invitation, et donc un nouveau mot de passe
-- choisi par celui qui détient le lien.
ALTER TABLE "users" ADD COLUMN "invite_pending" boolean DEFAULT false NOT NULL;
UPDATE "users" SET "invite_pending" = true
WHERE "email_verified_at" IS NULL
  AND EXISTS (
    SELECT 1 FROM "email_tokens" t
    WHERE t."user_id" = "users"."id" AND t."type" = 'invite'
      AND t."used_at" IS NULL AND t."expires_at" > now()
  );

-- Anti-rejeu TOTP : dernier pas de temps (30 s) accepté ; un code déjà
-- utilisé, ou plus ancien, est refusé même dans sa fenêtre de tolérance.
ALTER TABLE "users" ADD COLUMN "totp_last_step" bigint;
