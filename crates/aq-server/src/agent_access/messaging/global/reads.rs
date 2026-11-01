use super::*;
#[derive(Serialize)]
struct Limits {
    body_bytes: u32,
    context_refs: u8,
    attachments: u8,
    history_page: u8,
}
#[derive(Serialize)]
pub(super) struct Discovery {
    contract: &'static str,
    conversation: Conversation,
    runtime: Runtime,
    limits: Limits,
}
pub(super) async fn discovery(
    Extension(service): Extension<Arc<Service>>,
    RequireOwner(user): RequireOwner,
) -> Result<Json<Discovery>> {
    let mut tx = service.tx().await?;
    owner(&mut tx, &user).await?;
    let conversation = definition(&mut tx, None, false).await?;
    tx.commit().await.map_err(db)?;
    Ok(Json(Discovery {
        contract: CONTRACT,
        conversation,
        runtime: runtime(),
        limits: Limits {
            body_bytes: 16384,
            context_refs: 0,
            attachments: 0,
            history_page: 16,
        },
    }))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Page {
    before: Option<String>,
    after: Option<String>,
    limit: Option<u8>,
}
#[derive(Serialize)]
struct Human {
    kind: &'static str,
    user_id: Uuid,
}
#[derive(Serialize)]
struct Message {
    message_id: Uuid,
    thread_id: Uuid,
    conversation_id: Uuid,
    cursor: String,
    author: Human,
    body: String,
    body_digest: String,
    created_at_ms: u64,
    logical_admission: &'static str,
    host_delivery: &'static str,
    agent_receipt: Option<Value>,
}
#[derive(Serialize)]
pub(super) struct History {
    contract: &'static str,
    conversation: Conversation,
    runtime: Runtime,
    direction: &'static str,
    through_cursor: String,
    items: Vec<Message>,
    next_cursor: Option<String>,
    has_more: bool,
}
fn message(row: sqlx::postgres::PgRow, id: Uuid) -> Result<Message> {
    let sender: Value = row.get("sender");
    let recipient_value: Value = row.get("recipient");
    let thread: Uuid = row.get("thread_id");
    let message_id: Uuid = row.get("message_id");
    let body: String = row.get("body");
    let cursor: i64 = row.get("cursor");
    let at: i64 = row.get("created_at_ms");
    let payload: String = row.get("payload_digest");
    let user = sender
        .get("user_id")
        .and_then(Value::as_str)
        .and_then(|s| s.parse::<Uuid>().ok())
        .filter(|u| !u.is_nil())
        .ok_or_else(unavailable)?;
    if sender != author(user)
        || recipient_value != recipient(id)
        || thread != id
        || message_id.is_nil()
        || body.trim().is_empty()
        || body.len() > 16384
        || body.contains('\0')
        || cursor <= 0
        || at <= 0
        || payload.len() != 64
        || !payload
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(unavailable());
    }
    Ok(Message {
        message_id,
        thread_id: thread,
        conversation_id: id,
        cursor: cursor.to_string(),
        author: Human {
            kind: "human",
            user_id: user,
        },
        body_digest: hash(body.as_bytes()),
        body,
        created_at_ms: at as u64,
        logical_admission: "queued",
        host_delivery: "not_attempted",
        agent_receipt: None,
    })
}
pub(super) async fn history(
    Extension(service): Extension<Arc<Service>>,
    RequireOwner(user): RequireOwner,
    Path(id): Path<Uuid>,
    Query(page): Query<Page>,
) -> Result<Json<History>> {
    if id.is_nil() || page.before.is_some() && page.after.is_some() {
        return Err(invalid());
    }
    let limit = page.limit.unwrap_or(16);
    if !(1..=16).contains(&limit) {
        return Err(invalid());
    }
    let before = page.before.as_deref().map(positive).transpose()?;
    let after = page.after.as_deref().map(cursor).transpose()?;
    let forward = after.is_some();
    let mut tx = service.tx().await?;
    owner(&mut tx, &user).await?;
    let conversation = definition(&mut tx, Some(id), false).await?;
    let mailbox = key(id)?;
    let next: Option<i64> =
        sqlx::query_scalar("SELECT next_cursor FROM mailbox_heads WHERE mailbox_key=$1")
            .bind(&mailbox)
            .fetch_optional(&mut *tx)
            .await
            .map_err(db)?;
    let through = next
        .unwrap_or(1)
        .checked_sub(1)
        .filter(|v| *v >= 0)
        .ok_or_else(unavailable)?;
    let sql = if forward {
        "SELECT * FROM mailbox_messages WHERE mailbox_key=$1 AND cursor>$2 AND cursor<=$3 ORDER BY cursor LIMIT $4"
    } else {
        "SELECT * FROM mailbox_messages WHERE mailbox_key=$1 AND cursor<$2 AND cursor<=$3 ORDER BY cursor DESC LIMIT $4"
    };
    let anchor = if forward {
        after.unwrap_or(0)
    } else {
        before.unwrap_or(through.checked_add(1).ok_or_else(unavailable)?)
    };
    let rows = sqlx::query(sql)
        .bind(mailbox)
        .bind(anchor)
        .bind(through)
        .bind(i64::from(limit) + 1)
        .fetch_all(&mut *tx)
        .await
        .map_err(db)?;
    let mut result = History {
        contract: CONTRACT,
        conversation,
        runtime: runtime(),
        direction: if forward { "forward" } else { "backward" },
        through_cursor: through.to_string(),
        items: vec![],
        next_cursor: None,
        has_more: false,
    };
    let mut bytes = serde_json::to_vec(&result)
        .map_err(|_| unavailable())?
        .len()
        + 256;
    for row in rows {
        if result.items.len() >= usize::from(limit) {
            result.has_more = true;
            break;
        }
        let item = message(row, id)?;
        let size = serde_json::to_vec(&item).map_err(|_| unavailable())?.len() + 1;
        if bytes.checked_add(size).is_none_or(|n| n > PAGE_BYTES) {
            result.has_more = true;
            break;
        }
        bytes += size;
        result.items.push(item);
    }
    if result.has_more {
        result.next_cursor = Some(result.items.last().ok_or_else(unavailable)?.cursor.clone());
    }
    if !forward {
        result.items.reverse();
    }
    if serde_json::to_vec(&result)
        .map_err(|_| unavailable())?
        .len()
        > PAGE_BYTES
    {
        return Err(unavailable());
    }
    tx.commit().await.map_err(db)?;
    Ok(Json(result))
}
