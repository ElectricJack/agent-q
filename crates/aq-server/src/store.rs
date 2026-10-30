//! PostgreSQL foundation store. All timestamps are supplied by the caller.

use crate::STATE_FORMAT_VERSION;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::postgres::PgPoolOptions;
use sqlx::{FromRow, PgPool};
use std::time::Duration;
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error(
        "state format version {found} is newer than this server supports ({supported}); upgrade the server"
    )]
    IncompatibleState { found: u32, supported: u32 },
    #[error("a user with that email already exists")]
    EmailTaken,
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("migration error: {0}")]
    Migrate(#[from] sqlx::migrate::MigrateError),
    #[error("corrupt data: {0}")]
    Corrupt(String),
}

pub type Result<T> = std::result::Result<T, StoreError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Owner,
    Viewer,
}

impl Role {
    pub fn as_str(&self) -> &'static str {
        match self {
            Role::Owner => "owner",
            Role::Viewer => "viewer",
        }
    }
}

impl std::str::FromStr for Role {
    type Err = StoreError;
    fn from_str(s: &str) -> Result<Self> {
        match s {
            "owner" => Ok(Role::Owner),
            "viewer" => Ok(Role::Viewer),
            other => Err(StoreError::Corrupt(format!("unknown role {other:?}"))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct User {
    pub id: Uuid,
    pub email: String,
    pub role: Role,
    pub issuer: Option<String>,
    pub subject: Option<String>,
}

#[derive(Debug, Clone)]
pub struct NewNode {
    pub name: String,
    pub credential_hash: String,
    pub worker_version: String,
    pub protocol_version: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Node {
    pub id: Uuid,
    pub name: String,
    pub worker_version: String,
    pub protocol_version: u32,
    pub enrolled_at: DateTime<Utc>,
    pub last_seen_at: Option<DateTime<Utc>>,
    pub revoked_at: Option<DateTime<Utc>>,
}

#[derive(FromRow)]
struct UserRow {
    id: Uuid,
    email: String,
    role: String,
    issuer: Option<String>,
    subject: Option<String>,
}

impl TryFrom<UserRow> for User {
    type Error = StoreError;
    fn try_from(r: UserRow) -> Result<Self> {
        Ok(User {
            id: r.id,
            email: r.email,
            role: r.role.parse()?,
            issuer: r.issuer,
            subject: r.subject,
        })
    }
}

#[derive(FromRow)]
struct NodeRow {
    id: Uuid,
    name: String,
    worker_version: String,
    protocol_version: i32,
    enrolled_at: DateTime<Utc>,
    last_seen_at: Option<DateTime<Utc>>,
    revoked_at: Option<DateTime<Utc>>,
}

impl From<NodeRow> for Node {
    fn from(r: NodeRow) -> Self {
        Node {
            id: r.id,
            name: r.name,
            worker_version: r.worker_version,
            protocol_version: r.protocol_version.max(0) as u32,
            enrolled_at: r.enrolled_at,
            last_seen_at: r.last_seen_at,
            revoked_at: r.revoked_at,
        }
    }
}

const USER_COLS: &str = "id, email, role, issuer, subject";
const NODE_COLS: &str =
    "id, name, worker_version, protocol_version, enrolled_at, last_seen_at, revoked_at";

fn map_user_insert_err(e: sqlx::Error) -> StoreError {
    if let sqlx::Error::Database(db) = &e
        && db.is_unique_violation()
        && db.constraint() == Some("users_email_lower")
    {
        return StoreError::EmailTaken;
    }
    StoreError::Database(e)
}

#[derive(Clone)]
pub struct Store {
    pool: PgPool,
}

impl Store {
    pub async fn connect(database_url: &str) -> Result<Store> {
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .acquire_timeout(Duration::from_secs(5))
            .connect(database_url)
            .await?;
        Self::from_pool(pool).await
    }

    pub async fn from_pool(pool: PgPool) -> Result<Store> {
        sqlx::migrate!("./migrations").run(&pool).await?;
        let store = Store { pool };
        store.check_state_format().await?;
        Ok(store)
    }

    async fn check_state_format(&self) -> Result<()> {
        let value: String =
            sqlx::query_scalar("SELECT value FROM meta WHERE key = 'state_format_version'")
                .fetch_one(&self.pool)
                .await?;
        let found: u32 = value
            .parse()
            .map_err(|_| StoreError::Corrupt(format!("state_format_version {value:?}")))?;
        if found > STATE_FORMAT_VERSION {
            return Err(StoreError::IncompatibleState {
                found,
                supported: STATE_FORMAT_VERSION,
            });
        }
        Ok(())
    }

    pub async fn find_user_by_binding(&self, issuer: &str, subject: &str) -> Result<Option<User>> {
        sqlx::query_as::<_, UserRow>(&format!(
            "SELECT {USER_COLS} FROM users WHERE issuer = $1 AND subject = $2"
        ))
        .bind(issuer)
        .bind(subject)
        .fetch_optional(&self.pool)
        .await?
        .map(User::try_from)
        .transpose()
    }

    /// Binds an invited (unbound) user, matched by email case-insensitively.
    pub async fn bind_invited_user(
        &self,
        email: &str,
        issuer: &str,
        subject: &str,
        now: DateTime<Utc>,
    ) -> Result<Option<User>> {
        sqlx::query_as::<_, UserRow>(&format!(
            "UPDATE users SET issuer = $2, subject = $3, bound_at = $4 \
             WHERE lower(email) = lower($1) AND issuer IS NULL RETURNING {USER_COLS}"
        ))
        .bind(email)
        .bind(issuer)
        .bind(subject)
        .bind(now)
        .fetch_optional(&self.pool)
        .await?
        .map(User::try_from)
        .transpose()
    }

    /// Creates the first owner exactly once; later calls return `None`.
    pub async fn bootstrap_owner(
        &self,
        email: &str,
        issuer: &str,
        subject: &str,
        now: DateTime<Utc>,
    ) -> Result<Option<User>> {
        let mut tx = self.pool.begin().await?;
        let claimed = sqlx::query(
            "INSERT INTO meta (key, value) VALUES ('owner_bootstrap_completed_at', $1) \
             ON CONFLICT DO NOTHING",
        )
        .bind(now.to_rfc3339())
        .execute(&mut *tx)
        .await?;
        if claimed.rows_affected() == 0 {
            return Ok(None);
        }
        let row = sqlx::query_as::<_, UserRow>(&format!(
            "INSERT INTO users (id, email, role, issuer, subject, created_at, bound_at) \
             VALUES ($1, $2, 'owner', $3, $4, $5, $5) RETURNING {USER_COLS}"
        ))
        .bind(Uuid::new_v4())
        .bind(email)
        .bind(issuer)
        .bind(subject)
        .bind(now)
        .fetch_one(&mut *tx)
        .await
        .map_err(map_user_insert_err)?;
        tx.commit().await?;
        Ok(Some(row.try_into()?))
    }

    pub async fn invite_user(&self, email: &str, role: Role, now: DateTime<Utc>) -> Result<User> {
        sqlx::query_as::<_, UserRow>(&format!(
            "INSERT INTO users (id, email, role, created_at) VALUES ($1, $2, $3, $4) \
             RETURNING {USER_COLS}"
        ))
        .bind(Uuid::new_v4())
        .bind(email)
        .bind(role.as_str())
        .bind(now)
        .fetch_one(&self.pool)
        .await
        .map_err(map_user_insert_err)?
        .try_into()
    }

    pub async fn list_users(&self) -> Result<Vec<User>> {
        sqlx::query_as::<_, UserRow>(&format!(
            "SELECT {USER_COLS} FROM users ORDER BY created_at, email"
        ))
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(User::try_from)
        .collect()
    }

    pub async fn create_enrollment(
        &self,
        code_hash: &str,
        created_by: Uuid,
        now: DateTime<Utc>,
        expires_at: DateTime<Utc>,
    ) -> Result<Uuid> {
        let id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO enrollment_codes (id, code_hash, created_by, created_at, expires_at) \
             VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(id)
        .bind(code_hash)
        .bind(created_by)
        .bind(now)
        .bind(expires_at)
        .execute(&self.pool)
        .await?;
        Ok(id)
    }

    /// Atomically consumes a valid code and registers the node.
    pub async fn redeem_enrollment(
        &self,
        code_hash: &str,
        node: NewNode,
        now: DateTime<Utc>,
    ) -> Result<Option<Node>> {
        let node_id = Uuid::new_v4();
        let mut tx = self.pool.begin().await?;
        let claimed = sqlx::query(
            "UPDATE enrollment_codes SET used_at = $1, node_id = $2 \
             WHERE code_hash = $3 AND used_at IS NULL AND expires_at > $1",
        )
        .bind(now)
        .bind(node_id)
        .bind(code_hash)
        .execute(&mut *tx)
        .await?;
        if claimed.rows_affected() != 1 {
            return Ok(None);
        }
        let row = sqlx::query_as::<_, NodeRow>(&format!(
            "INSERT INTO nodes (id, name, credential_hash, worker_version, protocol_version, \
             enrolled_at) VALUES ($1, $2, $3, $4, $5, $6) RETURNING {NODE_COLS}"
        ))
        .bind(node_id)
        .bind(&node.name)
        .bind(&node.credential_hash)
        .bind(&node.worker_version)
        .bind(node.protocol_version as i32)
        .bind(now)
        .fetch_one(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(Some(row.into()))
    }

    /// Returns revoked nodes too so callers can tell revoked from unknown.
    pub async fn find_node_by_credential(&self, credential_hash: &str) -> Result<Option<Node>> {
        Ok(sqlx::query_as::<_, NodeRow>(&format!(
            "SELECT {NODE_COLS} FROM nodes WHERE credential_hash = $1"
        ))
        .bind(credential_hash)
        .fetch_optional(&self.pool)
        .await?
        .map(Node::from))
    }

    pub async fn record_heartbeat(
        &self,
        node_id: Uuid,
        worker_version: &str,
        protocol_version: u32,
        now: DateTime<Utc>,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE nodes SET last_seen_at = $2, worker_version = $3, protocol_version = $4 \
             WHERE id = $1",
        )
        .bind(node_id)
        .bind(now)
        .bind(worker_version)
        .bind(protocol_version as i32)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Returns true if the node was newly revoked.
    pub async fn revoke_node(&self, node_id: Uuid, now: DateTime<Utc>) -> Result<bool> {
        let r =
            sqlx::query("UPDATE nodes SET revoked_at = $2 WHERE id = $1 AND revoked_at IS NULL")
                .bind(node_id)
                .bind(now)
                .execute(&self.pool)
                .await?;
        Ok(r.rows_affected() == 1)
    }

    pub async fn list_nodes(&self) -> Result<Vec<Node>> {
        Ok(sqlx::query_as::<_, NodeRow>(&format!(
            "SELECT {NODE_COLS} FROM nodes ORDER BY enrolled_at, name"
        ))
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(Node::from)
        .collect())
    }
}
