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
struct SupervisorAuthor {
    kind: &'static str,
    actor_id: Uuid,
    generation: u64,
    session_id: Uuid,
}
#[derive(Serialize)]
struct DispatchRef {
    dispatch_id: Uuid,
    host_operation_id: Uuid,
    batch_digest: String,
}
/// A supervisor reply, anchored at the max cursor of the batch it answers.
#[derive(Serialize)]
struct Reply {
    message_id: Uuid,
    thread_id: Uuid,
    conversation_id: Uuid,
    cursor: String,
    author: SupervisorAuthor,
    body: String,
    body_digest: String,
    created_at_ms: u64,
    dispatch: DispatchRef,
    logical_admission: &'static str,
    host_delivery: &'static str,
    agent_receipt: Option<Value>,
}
#[derive(Serialize)]
#[serde(untagged)]
enum Item {
    Human(Message),
    Supervisor(Reply),
}
#[derive(Serialize)]
pub(super) struct History {
    contract: &'static str,
    conversation: Conversation,
    runtime: Runtime,
    direction: &'static str,
    through_cursor: String,
    items: Vec<Item>,
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
    let mut owners = Vec::new();
    for row in rows {
        if owners.len() >= usize::from(limit) {
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
        owners.push(item);
    }
    if result.has_more {
        result.next_cursor = Some(owners.last().ok_or_else(unavailable)?.cursor.clone());
    }
    if !forward {
        owners.reverse();
    }
    let ids: Vec<Uuid> = owners.iter().map(|m| m.message_id).collect();
    let cursors: Vec<i64> = owners
        .iter()
        .map(|m| m.cursor.parse::<i64>().map_err(|_| unavailable()))
        .collect::<Result<_>>()?;
    let carriers: Vec<(Uuid, Uuid, i64)> = sqlx::query("SELECT b.message_id,d.actor_id,d.generation FROM central_supervisor_mail_members b JOIN central_supervisor_dispatches d ON d.actor_id=b.actor_id AND d.reservation_id=b.reservation_id WHERE b.message_id=ANY($1) LIMIT 64")
        .bind(&ids)
        .fetch_all(&mut *tx)
        .await
        .map_err(db)?
        .into_iter()
        .map(|r| (r.get("message_id"), r.get("actor_id"), r.get("generation")))
        .collect();
    let replies = sqlx::query("SELECT r.dispatch_id,d.host_operation_id,d.batch_digest,d.actor_id,d.generation,d.session_id,m.*,(SELECT max(mm.cursor) FROM central_supervisor_mail_members mm WHERE mm.actor_id=d.actor_id AND mm.reservation_id=d.reservation_id) AS anchor FROM central_supervisor_replies r JOIN central_supervisor_dispatches d ON d.dispatch_id=r.dispatch_id JOIN mailbox_messages m ON m.message_id=r.reply_message_id WHERE (SELECT max(mm.cursor) FROM central_supervisor_mail_members mm WHERE mm.actor_id=d.actor_id AND mm.reservation_id=d.reservation_id)=ANY($1) ORDER BY m.cursor LIMIT 64")
        .bind(&cursors)
        .fetch_all(&mut *tx)
        .await
        .map_err(db)?
        .into_iter()
        .map(|row| reply(row, id))
        .collect::<Result<Vec<_>>>()?;
    tx.commit().await.map_err(db)?;
    let sessions = dispatch::sessions(&service.pool);
    for owner_item in owners.iter_mut() {
        if let Some((_, actor, generation)) = carriers.iter().find(|c| c.0 == owner_item.message_id)
        {
            let generation = u64::try_from(*generation).map_err(|_| unavailable())?;
            let snapshot = sessions
                .generation(*actor, generation)
                .await
                .map_err(|_| unavailable())?;
            owner_item.host_delivery =
                dispatch::delivery(snapshot.as_ref().map(|s| s.run.dispatch()));
        }
    }
    let mut replies: Vec<(i64, Reply)> = replies;
    for owner_item in owners {
        let at = owner_item.cursor.clone();
        result.items.push(Item::Human(owner_item));
        let mut i = 0;
        while i < replies.len() {
            if replies[i].0.to_string() == at {
                result.items.push(Item::Supervisor(replies.remove(i).1));
            } else {
                i += 1;
            }
        }
    }
    if serde_json::to_vec(&result)
        .map_err(|_| unavailable())?
        .len()
        > PAGE_BYTES
    {
        return Err(unavailable());
    }
    Ok(Json(result))
}
fn reply(row: sqlx::postgres::PgRow, id: Uuid) -> Result<(i64, Reply)> {
    let sender: Value = row.get("sender");
    let actor_id: Uuid = row.get("actor_id");
    let generation = u64::try_from(row.get::<i64, _>("generation")).map_err(|_| unavailable())?;
    let session_id: Uuid = row.get("session_id");
    let body: String = row.get("body");
    let cursor: i64 = row.get("cursor");
    let at: i64 = row.get("created_at_ms");
    let anchor: Option<i64> = row.get("anchor");
    let expected = json!({"kind":"central_supervisor","actor_id":actor_id,"generation":generation,"session_id":session_id});
    if sender != expected
        || row.get::<Value, _>("recipient") != reply::reply_recipient(id)
        || row.get::<Uuid, _>("thread_id") != id
        || body.trim().is_empty()
        || body.len() > 16384
        || cursor < 0
        || at <= 0
    {
        return Err(unavailable());
    }
    let anchor = anchor.filter(|a| *a > 0).ok_or_else(unavailable)?;
    Ok((
        anchor,
        Reply {
            message_id: row.get("message_id"),
            thread_id: id,
            conversation_id: id,
            cursor: anchor.to_string(),
            author: SupervisorAuthor {
                kind: "supervisor",
                actor_id,
                generation,
                session_id,
            },
            body_digest: hash(body.as_bytes()),
            body,
            created_at_ms: at as u64,
            dispatch: DispatchRef {
                dispatch_id: row.get("dispatch_id"),
                host_operation_id: row.get("host_operation_id"),
                batch_digest: row.get("batch_digest"),
            },
            logical_admission: "queued",
            host_delivery: "not_attempted",
            agent_receipt: None,
        },
    ))
}
