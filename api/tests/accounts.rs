//! Comptes : inscription bootstrap, invitations, jetons, administration
//! (garde-fous anti-lockout), réglages, 2FA.

mod common;

use chrono::{Duration, Utc};
use common::{TestDb, UserOptions, admin, create_user, recipe, user};
use hebdomiam::crypto::{hash_password, sha256_hex, verify_password};
use hebdomiam::domain::catalog::{EmailTokenType, Role};
use hebdomiam::repo::admin::{self as admin_repo, AdminRefusal, AdminUserPatch, CreateInvited, RenewInvite};
use hebdomiam::repo::email_tokens;
use hebdomiam::repo::settings::{self, DeleteAccount, PasswordCheck, ProfilePatch};
use hebdomiam::repo::twofa::{self, Activation, LoginMethod};
use hebdomiam::repo::users::{self, AcceptInvitation, RegisterResult};
use hebdomiam::totp;

#[tokio::test]
async fn bootstrap_premier_inscrit_admin_puis_ferme() {
    let t = TestDb::new().await;
    assert_eq!(users::count_users(&t.pool).await.unwrap(), 0);
    let RegisterResult::Created(first) = users::register(&t.pool, "a@example.com", "A", "h").await.unwrap() else {
        panic!("inscription bootstrap refusée");
    };
    assert_eq!(first.role, Role::Admin);
    assert!(matches!(
        users::register(&t.pool, "b@example.com", "B", "h").await.unwrap(),
        RegisterResult::Closed
    ));
    // Même si le seul compte est désactivé, l'inscription reste fermée.
    sqlx::query("UPDATE users SET disabled_at = now()")
        .execute(&t.pool)
        .await
        .unwrap();
    assert!(matches!(
        users::register(&t.pool, "c@example.com", "C", "h").await.unwrap(),
        RegisterResult::Closed
    ));
    t.drop().await;
}

#[tokio::test]
async fn jetons_usage_unique_type_et_expiration() {
    let t = TestDb::new().await;
    let id = user(&t.pool).await;
    let mut conn = t.pool.acquire().await.unwrap();
    let raw = email_tokens::create(&mut conn, id, EmailTokenType::Verify, 60)
        .await
        .unwrap();
    let stored: String = sqlx::query_scalar("SELECT token_hash FROM email_tokens")
        .fetch_one(&t.pool)
        .await
        .unwrap();
    assert_eq!(stored, sha256_hex(&raw), "seule l'empreinte est stockée");

    assert_eq!(
        email_tokens::consume(&mut conn, EmailTokenType::Reset, &raw)
            .await
            .unwrap(),
        None,
        "mauvais type"
    );
    assert_eq!(
        email_tokens::consume(&mut conn, EmailTokenType::Verify, &raw)
            .await
            .unwrap(),
        Some(id)
    );
    assert_eq!(
        email_tokens::consume(&mut conn, EmailTokenType::Verify, &raw)
            .await
            .unwrap(),
        None,
        "usage unique"
    );

    let expired = email_tokens::create(&mut conn, id, EmailTokenType::Reset, 60)
        .await
        .unwrap();
    sqlx::query("UPDATE email_tokens SET expires_at = $1 WHERE token_hash = $2")
        .bind(Utc::now() - Duration::minutes(1))
        .bind(sha256_hex(&expired))
        .execute(&t.pool)
        .await
        .unwrap();
    assert_eq!(
        email_tokens::consume(&mut conn, EmailTokenType::Reset, &expired)
            .await
            .unwrap(),
        None
    );
    drop(conn);
    t.drop().await;
}

#[tokio::test]
async fn invitation_creee_acceptee_puis_brulee() {
    let t = TestDb::new().await;
    let CreateInvited::Created { user: invited, token } =
        admin_repo::create_invited_user(&t.pool, "invite@example.com", "Invitée")
            .await
            .unwrap()
    else {
        panic!("création refusée");
    };
    assert_eq!(invited.role, Role::User);
    assert!(!invited.email_verified);
    assert!(matches!(
        admin_repo::create_invited_user(&t.pool, "invite@example.com", "X")
            .await
            .unwrap(),
        CreateInvited::EmailTaken
    ));

    // Renouvellement : l'ancien lien meurt.
    let RenewInvite::Renewed { token: renewed, .. } = admin_repo::renew_invite(&t.pool, invited.id).await.unwrap()
    else {
        panic!("renouvellement refusé");
    };
    assert!(matches!(
        users::accept_invitation(&t.pool, &token, "h".into()).await.unwrap(),
        AcceptInvitation::InvalidToken
    ));

    assert!(
        matches!(users::accept_invitation(&t.pool, &renewed, "mot-de-passe-tres-long".into()).await.unwrap(), AcceptInvitation::Accepted(id) if id == invited.id)
    );
    let row = users::find_by_id(&t.pool, invited.id).await.unwrap().unwrap();
    assert!(row.email_verified_at.is_some());
    assert!(verify_password(row.password_hash, "mot-de-passe-tres-long".into()).await);
    assert!(matches!(
        admin_repo::renew_invite(&t.pool, invited.id).await.unwrap(),
        RenewInvite::AlreadyActive
    ));

    // Un jeton survivant ne peut pas écraser le mot de passe d'un compte actif.
    let mut conn = t.pool.acquire().await.unwrap();
    let survivor = email_tokens::create(&mut conn, invited.id, EmailTokenType::Invite, 60)
        .await
        .unwrap();
    assert!(matches!(
        users::accept_invitation(&t.pool, &survivor, "autre".into())
            .await
            .unwrap(),
        AcceptInvitation::AlreadyActive
    ));
    drop(conn);
    t.drop().await;
}

#[tokio::test]
async fn invitation_refusee_pour_compte_desactive() {
    let t = TestDb::new().await;
    let id = create_user(
        &t.pool,
        UserOptions {
            verified: false,
            disabled: true,
            invite_pending: true,
            ..Default::default()
        },
    )
    .await;
    assert!(matches!(
        admin_repo::renew_invite(&t.pool, id).await.unwrap(),
        RenewInvite::Disabled
    ));
    let mut conn = t.pool.acquire().await.unwrap();
    let token = email_tokens::create(&mut conn, id, EmailTokenType::Invite, 60)
        .await
        .unwrap();
    assert!(matches!(
        users::accept_invitation(&t.pool, &token, "h".into()).await.unwrap(),
        AcceptInvitation::Disabled
    ));
    drop(conn);
    t.drop().await;
}

#[tokio::test]
async fn admin_garde_fous_self_et_dernier_admin() {
    let t = TestDb::new().await;
    let me = admin(&t.pool).await;
    let other = admin(&t.pool).await;
    let demote = AdminUserPatch {
        role: Some(Role::User),
        disabled: None,
    };

    assert!(matches!(
        admin_repo::update_user(&t.pool, me, me, &demote).await.unwrap(),
        Err(AdminRefusal::SelfTarget)
    ));
    assert!(matches!(
        admin_repo::delete_user(&t.pool, me, me).await.unwrap(),
        Err(AdminRefusal::SelfTarget)
    ));

    // Deux admins actifs : rétrograder l'autre est permis…
    let demoted = admin_repo::update_user(&t.pool, me, other, &demote)
        .await
        .unwrap()
        .ok()
        .unwrap();
    assert_eq!(demoted.role, Role::User);
    // …mais plus maintenant : `me` est le dernier (vu depuis un autre admin
    // jamais activé, qui ne compte pas comme relève).
    let pending = create_user(
        &t.pool,
        UserOptions {
            role: Role::Admin,
            verified: false,
            ..Default::default()
        },
    )
    .await;
    let disable = AdminUserPatch {
        role: None,
        disabled: Some(true),
    };
    assert!(matches!(
        admin_repo::update_user(&t.pool, pending, me, &disable).await.unwrap(),
        Err(AdminRefusal::LastAdmin)
    ));
    assert!(matches!(
        admin_repo::delete_user(&t.pool, pending, me).await.unwrap(),
        Err(AdminRefusal::LastAdmin)
    ));

    let unknown = uuid::Uuid::new_v4();
    assert!(matches!(
        admin_repo::update_user(&t.pool, me, unknown, &demote).await.unwrap(),
        Err(AdminRefusal::NotFound)
    ));
    t.drop().await;
}

#[tokio::test]
async fn admin_desactivation_conserve_la_date_et_suppression_en_cascade() {
    let t = TestDb::new().await;
    let me = admin(&t.pool).await;
    let target = user(&t.pool).await;
    recipe(&t.pool, target, "Soupe").await;
    let disable = AdminUserPatch {
        role: None,
        disabled: Some(true),
    };
    admin_repo::update_user(&t.pool, me, target, &disable)
        .await
        .unwrap()
        .ok()
        .unwrap();
    let first: Option<chrono::DateTime<Utc>> = sqlx::query_scalar("SELECT disabled_at FROM users WHERE id = $1")
        .bind(target)
        .fetch_one(&t.pool)
        .await
        .unwrap();
    admin_repo::update_user(&t.pool, me, target, &disable)
        .await
        .unwrap()
        .ok()
        .unwrap();
    let second: Option<chrono::DateTime<Utc>> = sqlx::query_scalar("SELECT disabled_at FROM users WHERE id = $1")
        .bind(target)
        .fetch_one(&t.pool)
        .await
        .unwrap();
    assert_eq!(first, second);

    admin_repo::delete_user(&t.pool, me, target)
        .await
        .unwrap()
        .ok()
        .unwrap();
    let recipes: i64 = sqlx::query_scalar("SELECT count(*) FROM recipes")
        .fetch_one(&t.pool)
        .await
        .unwrap();
    assert_eq!(recipes, 0);
    // Liste blanche : la liste admin n'expose aucun champ métier.
    let listed = serde_json::to_value(admin_repo::list_users(&t.pool).await.unwrap()).unwrap();
    let keys: Vec<&str> = listed[0].as_object().unwrap().keys().map(String::as_str).collect();
    assert_eq!(keys.len(), 7, "{keys:?}");
    t.drop().await;
}

#[tokio::test]
async fn reglages_profil_mot_de_passe_et_suppression() {
    let t = TestDb::new().await;
    let hash = hash_password("ancien-mot-de-passe".into()).await;
    let id = create_user(
        &t.pool,
        UserOptions {
            password_hash: hash,
            ..Default::default()
        },
    )
    .await;

    let profile = settings::get_profile(&t.pool, id).await.unwrap().unwrap();
    assert_eq!(
        (profile.default_dish_count, profile.rotation_weeks, profile.totp_enabled),
        (4, 3, false)
    );
    let patch = ProfilePatch {
        default_dish_count: Some(3),
        ..Default::default()
    };
    let updated = settings::update_profile(&t.pool, id, &patch).await.unwrap().unwrap();
    assert_eq!((updated.default_dish_count, updated.default_days_count), (3, 7));

    assert!(matches!(
        settings::change_password(&t.pool, id, "faux", "nouveau-mot-de-passe")
            .await
            .unwrap(),
        PasswordCheck::InvalidPassword
    ));
    assert!(matches!(
        settings::change_password(&t.pool, id, "ancien-mot-de-passe", "nouveau-mot-de-passe")
            .await
            .unwrap(),
        PasswordCheck::Ok(1)
    ));

    recipe(&t.pool, id, "Chili").await;
    assert!(matches!(
        settings::delete_account(&t.pool, id, "faux").await.unwrap(),
        DeleteAccount::InvalidPassword
    ));
    assert!(matches!(
        settings::delete_account(&t.pool, id, "nouveau-mot-de-passe")
            .await
            .unwrap(),
        DeleteAccount::Ok
    ));
    assert_eq!(users::count_users(&t.pool).await.unwrap(), 0);
    let recipes: i64 = sqlx::query_scalar("SELECT count(*) FROM recipes")
        .fetch_one(&t.pool)
        .await
        .unwrap();
    assert_eq!(recipes, 0);
    t.drop().await;
}

#[tokio::test]
async fn dernier_admin_ne_peut_pas_s_auto_supprimer() {
    let t = TestDb::new().await;
    let hash = hash_password("mot-de-passe-admin".into()).await;
    let me = create_user(
        &t.pool,
        UserOptions {
            role: Role::Admin,
            password_hash: hash.clone(),
            ..Default::default()
        },
    )
    .await;
    // Ni un admin désactivé ni un admin jamais activé ne comptent comme relève.
    create_user(
        &t.pool,
        UserOptions {
            role: Role::Admin,
            disabled: true,
            ..Default::default()
        },
    )
    .await;
    create_user(
        &t.pool,
        UserOptions {
            role: Role::Admin,
            verified: false,
            ..Default::default()
        },
    )
    .await;
    assert!(matches!(
        settings::delete_account(&t.pool, me, "mot-de-passe-admin")
            .await
            .unwrap(),
        DeleteAccount::LastAdmin
    ));
    admin(&t.pool).await;
    assert!(matches!(
        settings::delete_account(&t.pool, me, "mot-de-passe-admin")
            .await
            .unwrap(),
        DeleteAccount::Ok
    ));
    t.drop().await;
}

#[tokio::test]
async fn deux_facteurs_cycle_complet() {
    let t = TestDb::new().await;
    let hash = hash_password("mot-de-passe-2fa".into()).await;
    let id = create_user(
        &t.pool,
        UserOptions {
            password_hash: hash,
            ..Default::default()
        },
    )
    .await;
    let now = || Utc::now().timestamp() as u64;

    assert!(matches!(
        twofa::activate(&t.pool, id, "mot-de-passe-2fa", "123456")
            .await
            .unwrap(),
        Activation::NoSetup
    ));
    let setup = twofa::start_setup(&t.pool, id, "a@b.c").await.unwrap().unwrap();
    assert!(setup.otpauth_url.contains(&setup.secret));
    assert_eq!(
        twofa::verify_login(&t.pool, id, &totp::generate(&setup.secret, now()).unwrap())
            .await
            .unwrap(),
        None,
        "pas encore active"
    );
    assert!(matches!(
        twofa::activate(&t.pool, id, "mot-de-passe-2fa", "000000")
            .await
            .unwrap(),
        Activation::InvalidCode
    ));

    // Sans le mot de passe, un cookie volé ne peut pas enrôler son authenticateur.
    assert!(matches!(
        twofa::activate(&t.pool, id, "faux", &totp::generate(&setup.secret, now()).unwrap())
            .await
            .unwrap(),
        Activation::InvalidPassword
    ));
    let Activation::Activated {
        backup_code,
        session_epoch,
    } = twofa::activate(
        &t.pool,
        id,
        "mot-de-passe-2fa",
        &totp::generate(&setup.secret, now()).unwrap(),
    )
    .await
    .unwrap()
    else {
        panic!("activation refusée");
    };
    assert_eq!(backup_code.len(), 12);
    assert_eq!(session_epoch, 1, "les autres sessions sont révoquées à l'activation");
    assert!(matches!(
        twofa::activate(
            &t.pool,
            id,
            "mot-de-passe-2fa",
            &totp::generate(&setup.secret, now()).unwrap()
        )
        .await
        .unwrap(),
        Activation::NoSetup
    ));

    // Anti-rejeu : le code qui a servi à l'activation ne reconnecte pas…
    let used = totp::generate(&setup.secret, now()).unwrap();
    assert_eq!(
        twofa::verify_login(&t.pool, id, &used).await.unwrap(),
        None,
        "code déjà utilisé"
    );
    // …mais le code du pas suivant, oui — une seule fois.
    let next = totp::generate(&setup.secret, now() + 30).unwrap();
    assert_eq!(
        twofa::verify_login(&t.pool, id, &next).await.unwrap(),
        Some(LoginMethod::Totp)
    );
    assert_eq!(
        twofa::verify_login(&t.pool, id, &next).await.unwrap(),
        None,
        "rejeu refusé"
    );
    assert_eq!(
        twofa::verify_login(&t.pool, id, &backup_code).await.unwrap(),
        Some(LoginMethod::Backup)
    );
    assert_eq!(
        twofa::verify_login(&t.pool, id, &backup_code).await.unwrap(),
        None,
        "secours à usage unique"
    );

    // 2FA active : un nouveau setup est refusé et ne touche à rien (un cookie
    // volé ne retire pas le second facteur sans le mot de passe).
    assert!(twofa::start_setup(&t.pool, id, "a@b.c").await.unwrap().is_none());
    let still = users::find_by_id(&t.pool, id).await.unwrap().unwrap();
    assert_eq!(still.totp_secret.as_deref(), Some(setup.secret.as_str()));
    assert!(still.totp_enabled_at.is_some());

    assert!(twofa::disable(&t.pool, id, "faux").await.unwrap().is_none());
    assert_eq!(twofa::disable(&t.pool, id, "mot-de-passe-2fa").await.unwrap(), Some(2));
    let row = users::find_by_id(&t.pool, id).await.unwrap().unwrap();
    assert!(row.totp_secret.is_none() && row.totp_enabled_at.is_none() && row.totp_backup_hash.is_none());
    t.drop().await;
}

#[tokio::test]
async fn reinitialisation_ferme_sessions_liens_et_invitation() {
    let t = TestDb::new().await;
    // Compte invité qui s'active par « mot de passe oublié » plutôt que par le lien.
    let CreateInvited::Created {
        user: invited,
        token: invite,
    } = admin_repo::create_invited_user(&t.pool, "invite2@example.com", "Invité")
        .await
        .unwrap()
    else {
        panic!("création refusée");
    };
    let mut conn = t.pool.acquire().await.unwrap();
    let reset_a = email_tokens::create(&mut conn, invited.id, EmailTokenType::Reset, 60)
        .await
        .unwrap();
    let reset_b = email_tokens::create(&mut conn, invited.id, EmailTokenType::Reset, 60)
        .await
        .unwrap();
    drop(conn);

    assert_eq!(
        users::reset_password(&t.pool, "jeton-inconnu", "x".into())
            .await
            .unwrap(),
        None
    );
    let reset = users::reset_password(&t.pool, &reset_a, "nouveau-mot-de-passe-long".into())
        .await
        .unwrap();
    assert_eq!(reset, Some(invited.id));

    let row = users::find_by_id(&t.pool, invited.id).await.unwrap().unwrap();
    assert!(verify_password(row.password_hash.clone(), "nouveau-mot-de-passe-long".into()).await);
    assert_eq!(row.session_epoch, 1, "sessions existantes révoquées");
    assert!(
        row.email_verified_at.is_some(),
        "le lien prouve la possession du courriel"
    );
    assert!(!row.invite_pending);

    // L'autre lien de réinitialisation et le lien d'invitation sont morts :
    // aucun ne peut plus écraser le mot de passe du compte désormais en usage.
    assert_eq!(
        users::reset_password(&t.pool, &reset_b, "pirate-mot-de-passe".into())
            .await
            .unwrap(),
        None
    );
    assert!(matches!(
        users::accept_invitation(&t.pool, &invite, "pirate-mot-de-passe".into())
            .await
            .unwrap(),
        AcceptInvitation::InvalidToken
    ));
    assert!(matches!(
        admin_repo::renew_invite(&t.pool, invited.id).await.unwrap(),
        RenewInvite::AlreadyActive
    ));
    let after = users::find_by_id(&t.pool, invited.id).await.unwrap().unwrap();
    assert_eq!(after.password_hash, row.password_hash);
    t.drop().await;
}

#[tokio::test]
async fn pas_d_invitation_pour_un_compte_en_usage_au_courriel_non_verifie() {
    // L'admin de bootstrap n'a jamais cliqué son lien de vérification : un
    // autre admin ne doit pas pouvoir lui « renvoyer une invitation » pour
    // choisir son mot de passe à sa place.
    let t = TestDb::new().await;
    let bootstrap = create_user(
        &t.pool,
        UserOptions {
            role: Role::Admin,
            verified: false,
            ..Default::default()
        },
    )
    .await;
    assert!(matches!(
        admin_repo::renew_invite(&t.pool, bootstrap).await.unwrap(),
        RenewInvite::AlreadyActive
    ));
    t.drop().await;
}
