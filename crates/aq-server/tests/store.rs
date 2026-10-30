use aq_server::store::{NewNode, Role, Store, StoreError};
use aq_server::tokens::{new_enrollment_code, new_node_credential, verifier};
use chrono::{DateTime, Duration, TimeZone, Utc};
use sqlx::PgPool;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};

fn t0() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 1, 1, 12, 0, 0).unwrap()
}

fn new_node(name: &str) -> NewNode {
    NewNode {
        name: name.into(),
        credential_hash: verifier(&new_node_credential()),
        worker_version: "0.1.0".into(),
        protocol_version: 1,
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn reconnect_preserves_state(pool_opts: PgPoolOptions, conn: PgConnectOptions) {
    let pool = pool_opts.clone().connect_with(conn.clone()).await.unwrap();
    let store = Store::from_pool(pool).await.unwrap();
    store
        .bootstrap_owner("o@x.com", "iss", "sub", t0())
        .await
        .unwrap()
        .unwrap();
    drop(store);

    let pool2 = pool_opts.connect_with(conn).await.unwrap();
    let store2 = Store::from_pool(pool2).await.unwrap();
    let user = store2
        .find_user_by_binding("iss", "sub")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(user.email, "o@x.com");
    assert_eq!(user.role, Role::Owner);
}

#[sqlx::test(migrations = "./migrations")]
async fn bootstrap_owner_only_once(pool: PgPool) {
    let store = Store::from_pool(pool).await.unwrap();
    assert!(
        store
            .bootstrap_owner("a@x.com", "i", "1", t0())
            .await
            .unwrap()
            .is_some()
    );
    assert!(
        store
            .bootstrap_owner("b@x.com", "i", "2", t0())
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(store.list_users().await.unwrap().len(), 1);
}

#[sqlx::test(migrations = "./migrations")]
async fn email_unique_case_insensitive(pool: PgPool) {
    let store = Store::from_pool(pool).await.unwrap();
    store
        .invite_user("a@x.com", Role::Viewer, t0())
        .await
        .unwrap();
    let err = store
        .invite_user("A@x.com", Role::Viewer, t0())
        .await
        .unwrap_err();
    assert!(matches!(err, StoreError::EmailTaken), "{err:?}");
}

#[sqlx::test(migrations = "./migrations")]
async fn invited_user_binds_once(pool: PgPool) {
    let store = Store::from_pool(pool).await.unwrap();
    store
        .invite_user("a@x.com", Role::Viewer, t0())
        .await
        .unwrap();
    let u = store
        .bind_invited_user("A@x.com", "i", "1", t0())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(u.role, Role::Viewer);
    assert_eq!(u.subject.as_deref(), Some("1"));
    assert!(
        store
            .bind_invited_user("a@x.com", "i", "2", t0())
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        store
            .bind_invited_user("none@x.com", "i", "3", t0())
            .await
            .unwrap()
            .is_none()
    );
}

async fn owner_and_code(store: &Store, expires_in: Duration) -> (String, DateTime<Utc>) {
    let owner = store
        .bootstrap_owner("o@x.com", "i", "o", t0())
        .await
        .unwrap()
        .unwrap();
    let code = new_enrollment_code();
    let hash = verifier(&code);
    let expires = t0() + expires_in;
    store
        .create_enrollment(&hash, owner.id, t0(), expires)
        .await
        .unwrap();
    (hash, expires)
}

#[sqlx::test(migrations = "./migrations")]
async fn enrollment_code_single_use(pool: PgPool) {
    let store = Store::from_pool(pool).await.unwrap();
    let (hash, _) = owner_and_code(&store, Duration::minutes(10)).await;
    let node = store
        .redeem_enrollment(&hash, new_node("n1"), t0() + Duration::minutes(1))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(node.name, "n1");
    assert!(
        store
            .redeem_enrollment(&hash, new_node("n2"), t0() + Duration::minutes(2))
            .await
            .unwrap()
            .is_none()
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn enrollment_code_expires(pool: PgPool) {
    let store = Store::from_pool(pool).await.unwrap();
    let (hash, expires) = owner_and_code(&store, Duration::minutes(10)).await;
    let r = store
        .redeem_enrollment(&hash, new_node("n"), expires + Duration::seconds(1))
        .await
        .unwrap();
    assert!(r.is_none());
}

#[sqlx::test(migrations = "./migrations")]
async fn concurrent_redeem_yields_one_node(pool: PgPool) {
    let store = Store::from_pool(pool).await.unwrap();
    let (hash, _) = owner_and_code(&store, Duration::minutes(10)).await;
    let now = t0() + Duration::minutes(1);
    let (a, b) = tokio::join!(
        store.redeem_enrollment(&hash, new_node("a"), now),
        store.redeem_enrollment(&hash, new_node("b"), now),
    );
    let (a, b) = (a.unwrap(), b.unwrap());
    assert_eq!(a.is_some() as u8 + b.is_some() as u8, 1);
    assert_eq!(store.list_nodes().await.unwrap().len(), 1);
}

#[sqlx::test(migrations = "./migrations")]
async fn node_lifecycle(pool: PgPool) {
    let store = Store::from_pool(pool).await.unwrap();
    let (hash, _) = owner_and_code(&store, Duration::minutes(10)).await;
    let nn = new_node("n");
    let cred = nn.credential_hash.clone();
    let node = store
        .redeem_enrollment(&hash, nn, t0())
        .await
        .unwrap()
        .unwrap();
    assert!(node.last_seen_at.is_none());

    let later = t0() + Duration::seconds(15);
    store
        .record_heartbeat(node.id, "0.2.0", 1, later)
        .await
        .unwrap();
    let found = store.find_node_by_credential(&cred).await.unwrap().unwrap();
    assert_eq!(found.worker_version, "0.2.0");
    assert_eq!(found.last_seen_at, Some(later));

    assert!(store.revoke_node(node.id, later).await.unwrap());
    assert!(!store.revoke_node(node.id, later).await.unwrap());
    let found = store.find_node_by_credential(&cred).await.unwrap().unwrap();
    assert_eq!(found.revoked_at, Some(later));
    assert!(
        store
            .find_node_by_credential("nope")
            .await
            .unwrap()
            .is_none()
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn refuses_newer_state_format(pool: PgPool) {
    let store = Store::from_pool(pool.clone()).await.unwrap();
    drop(store);
    sqlx::query("UPDATE meta SET value = '2' WHERE key = 'state_format_version'")
        .execute(&pool)
        .await
        .unwrap();
    let err = Store::from_pool(pool).await.err().unwrap();
    assert!(
        matches!(
            err,
            StoreError::IncompatibleState {
                found: 2,
                supported: 1
            }
        ),
        "{err:?}"
    );
}

#[test]
fn verifier_is_deterministic_and_hex64() {
    let v = verifier("secret");
    assert_eq!(v, verifier("secret"));
    assert_ne!(v, verifier("secret2"));
    assert_eq!(v.len(), 64);
    assert!(v.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f')));
}

#[test]
fn tokens_have_prefixes_and_are_unique() {
    let (a, b) = (new_enrollment_code(), new_enrollment_code());
    assert!(a.starts_with("aqe_") && a != b);
    assert_eq!(a.len(), 4 + 27);
    let (c, d) = (new_node_credential(), new_node_credential());
    assert!(c.starts_with("aqn_") && c != d);
    assert_eq!(c.len(), 4 + 43);
}
