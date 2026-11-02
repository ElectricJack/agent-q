//! Full current human identity across owner I/O; UUID/role alone is insufficient.
use crate::{
    AppState,
    error::AppError,
    store::{Role, User},
};
use sqlx::{Postgres, Row, Transaction};
#[derive(Clone)]
pub(crate) struct ActorPin {
    actor: User,
    revision: String,
}
fn denied() -> AppError {
    AppError::Forbidden("settlement owner authorization changed".into())
}
fn matches(row: &sqlx::postgres::PgRow, user: &User) -> bool {
    user.role == Role::Owner
        && row.get::<String, _>("role") == "owner"
        && row.get::<String, _>("email") == user.email
        && row.get::<Option<String>, _>("issuer") == user.issuer
        && row.get::<Option<String>, _>("subject") == user.subject
}
impl ActorPin {
    pub(crate) async fn observe(state: &AppState, actor: &User) -> Result<Self, AppError> {
        let row=sqlx::query("SELECT role,email,issuer,subject,xmin::text AS authorization_revision FROM users WHERE id=$1").bind(actor.id).fetch_optional(&state.store.pool).await.map_err(|_|AppError::Internal)?.ok_or_else(denied)?;
        if !matches(&row, actor) {
            return Err(denied());
        }
        Ok(Self {
            actor: actor.clone(),
            revision: row.get("authorization_revision"),
        })
    }
    pub(crate) fn revision(&self) -> &str {
        &self.revision
    }
    pub(crate) fn id(&self) -> uuid::Uuid {
        self.actor.id
    }
    pub(crate) fn identity_digest(&self) -> Result<String, AppError> {
        aq_agent_api::digest(&serde_json::json!({"id":self.actor.id,"email":self.actor.email,"issuer":self.actor.issuer,"subject":self.actor.subject})).map_err(|_|AppError::Internal)
    }
    pub(crate) async fn current_locked(
        state: &AppState,
        tx: &mut Transaction<'_, Postgres>,
        id: uuid::Uuid,
    ) -> Result<Self, AppError> {
        let row=sqlx::query("SELECT role,email,issuer,subject,xmin::text AS authorization_revision FROM users WHERE id=$1 FOR SHARE").bind(id).fetch_optional(&mut **tx).await.map_err(|_|AppError::Internal)?.ok_or_else(denied)?;
        if row.get::<String, _>("role") != "owner" {
            return Err(denied());
        }
        let _ = state;
        Ok(Self {
            actor: User {
                id,
                email: row.get("email"),
                issuer: row.get("issuer"),
                subject: row.get("subject"),
                role: Role::Owner,
            },
            revision: row.get("authorization_revision"),
        })
    }
    /// Acquire only AFTER common authority, never user->Node/attempt.
    pub(crate) async fn fence(&self, tx: &mut Transaction<'_, Postgres>) -> Result<(), AppError> {
        let row=sqlx::query("SELECT role,email,issuer,subject,xmin::text AS authorization_revision FROM users WHERE id=$1 FOR SHARE").bind(self.actor.id).fetch_optional(&mut **tx).await.map_err(|_|AppError::Internal)?.ok_or_else(denied)?;
        if !matches(&row, &self.actor)
            || row.get::<String, _>("authorization_revision") != self.revision
        {
            return Err(denied());
        }
        Ok(())
    }
}
