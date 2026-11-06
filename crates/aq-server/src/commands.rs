//! The generic command surface (spec 0006 §4.3): `POST /api/v1/commands/{name}`
//! and `GET /api/v1/commands` on both listeners. One `Caller` is resolved per
//! request, the grant and scope are checked and audited, and the command's
//! own route serves it in process (`aq-command-registry` builds the request).
use crate::{AppState, agent_access::grant_check as grants, authz::CurrentUser, store::Role};
use aq_agent_grants::{AuditEntry, Decision, Refusal, RefusalCode};
use aq_command_registry::{self as registry, CommandRequest, Principal, Registry};
use axum::{
    Extension, Json, Router,
    body::{Body, Bytes},
    extract::{Path, Query, State},
    http::{HeaderMap, HeaderValue, Request, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde_json::{Value, json};
use std::sync::OnceLock;
use tower::ServiceExt;
use uuid::Uuid;

/// The registry: the static catalog plus every registered `ActionKind`.
pub fn registry() -> &'static Registry {
    static REGISTRY: OnceLock<Registry> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        let actions = crate::dashboard_api::actions::existing::schemas();
        Registry::new(actions.specs()).expect("command registry")
    })
}
/// `protocol/commands/v1/commands.json`, generated from the registry.
pub fn document() -> Value {
    let actions = crate::dashboard_api::actions::existing::schemas();
    registry::document(registry(), actions.schemas())
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Listener {
    Admin,
    Machine,
}
#[derive(Clone)]
struct Surface {
    inner: Router,
    listener: Listener,
}
/// The caller of one command (§6.1).
pub(crate) enum Caller {
    Owner(crate::store::User),
    Viewer(crate::store::User),
    Supervisor(crate::agent_access::supervisor_conversation::reply::SupervisorPrincipal),
    Agent(grants::AgentCaller),
}
impl Caller {
    fn principal(&self) -> Option<Principal> {
        match self {
            Self::Owner(_) => Some(Principal::Owner),
            Self::Viewer(_) => None,
            Self::Supervisor(_) => Some(Principal::Supervisor),
            Self::Agent(_) => Some(Principal::Agent),
        }
    }
}

/// Mount the command surface in front of a listener's finished router.
pub fn mount(inner: Router, state: AppState, listener: Listener) -> Router {
    Router::new()
        .route("/api/v1/commands", get(discover))
        .route("/api/v1/commands/{name}", post(invoke))
        .layer(Extension(Surface {
            inner: inner.clone(),
            listener,
        }))
        .layer(axum::middleware::from_fn_with_state(
            state.store.admission.clone(),
            crate::admission::gate,
        ))
        .with_state(state)
        .merge(inner)
}

fn refuse(r: Refusal, request_id: Uuid) -> Response {
    let status = StatusCode::from_u16(r.code.status()).unwrap_or(StatusCode::FORBIDDEN);
    let body = r.with_request(request_id).body();
    (status, [("Cache-Control", "no-store")], Json(body)).into_response()
}
fn error(code: RefusalCode, command: &str, message: &str, request_id: Uuid) -> Response {
    refuse(Refusal::error(code, command, message), request_id)
}

async fn caller(state: &AppState, s: &Surface, headers: &HeaderMap) -> Result<Caller, RefusalCode> {
    if s.listener == Listener::Admin {
        let (mut parts, _) = Request::new(()).into_parts();
        parts.headers = headers.clone();
        use axum::extract::FromRequestParts;
        let CurrentUser(user) = CurrentUser::from_request_parts(&mut parts, state)
            .await
            .map_err(|_| RefusalCode::Unauthorized)?;
        return Ok(match user.role {
            Role::Owner => Caller::Owner(user),
            _ => Caller::Viewer(user),
        });
    }
    let bearer = headers
        .get("authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .ok_or(RefusalCode::Unauthorized)?;
    if bearer.starts_with("aqa_") {
        return grants::agent_caller(state, headers)
            .await
            .map(Caller::Agent);
    }
    let mut tx = state
        .store
        .pool
        .begin()
        .await
        .map_err(|_| RefusalCode::Internal)?;
    let at = crate::execution::api::now(state);
    let p = crate::agent_access::supervisor_conversation::reply::authenticate_in_transaction(
        &state.store.pool,
        &mut tx,
        bearer,
        "enqueue",
        at,
    )
    .await
    .map_err(|_| RefusalCode::Unauthorized)?;
    tx.commit().await.map_err(|_| RefusalCode::Internal)?;
    Ok(Caller::Supervisor(p))
}

fn whoami(c: &Caller) -> Value {
    match c {
        Caller::Owner(u) | Caller::Viewer(u) => {
            json!({"api_version":1,"principal":if u.role == Role::Owner {"owner"} else {"viewer"},"user_id":u.id,"email":u.email})
        }
        Caller::Supervisor(p) => {
            json!({"api_version":1,"principal":"supervisor","session_id":p.session_id,"actor_id":p.actor_id,"generation":p.generation.to_string(),"scopes":p.scopes})
        }
        Caller::Agent(a) => a.whoami(),
    }
}

/// `GET /api/v1/commands[?all=true]`: what this caller may run, filtered by
/// the same check `invoke` applies ("published means runnable").
async fn discover(
    State(state): State<AppState>,
    Extension(s): Extension<Surface>,
    Query(q): Query<std::collections::BTreeMap<String, String>>,
    headers: HeaderMap,
) -> Response {
    let request_id = Uuid::new_v4();
    let c = match caller(&state, &s, &headers).await {
        Ok(c) => c,
        Err(code) => {
            return error(
                code,
                "commands",
                "Authenticate to list commands.",
                request_id,
            );
        }
    };
    let all = q.get("all").is_some_and(|v| v == "true");
    let (mut runnable, mut unavailable) = (Vec::new(), Vec::new());
    for spec in registry().commands() {
        let entry = json!({"name":spec.name,"kind":spec.kind,"summary":spec.summary,"cli":spec.cli(),"grant":spec.grant});
        match available(&c, spec) {
            Ok(scope) => {
                let mut e = entry;
                e["scope"] = json!(scope);
                runnable.push(e);
            }
            Err(code) if all => {
                let mut e = entry;
                e["reason"] = json!(code.as_str());
                unavailable.push(e);
            }
            Err(_) => {}
        }
    }
    let mut body = json!({"api_version":1,"principal":whoami(&c)["principal"],"commands":runnable});
    if all {
        body["unavailable"] = json!(unavailable);
    }
    (StatusCode::OK, [("Cache-Control", "no-store")], Json(body)).into_response()
}
/// Ok(held scope) when `c` may run `spec` at all.
fn available(c: &Caller, spec: &registry::CommandSpec) -> Result<Option<String>, RefusalCode> {
    let p = c.principal().ok_or(RefusalCode::NotAvailable)?;
    spec.route(p).ok_or(RefusalCode::NotAvailable)?;
    let Caller::Agent(a) = c else {
        return Ok(None);
    };
    let Some(grant) = spec.grant else {
        return Ok(None);
    };
    if spec.owner_only() {
        return Err(RefusalCode::OwnerOnly);
    }
    let g = a.grants.get(grant).ok_or(RefusalCode::GrantMissing)?;
    Ok(Some(g.scope.to_string()))
}

/// `POST /api/v1/commands/{name}`.
async fn invoke(
    State(state): State<AppState>,
    Extension(s): Extension<Surface>,
    Path(name): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let request_id = Uuid::new_v4();
    let Some(spec) = registry().get(&name) else {
        return error(
            RefusalCode::UnknownCommand,
            &name,
            "No such command; `aq commands` lists them.",
            request_id,
        );
    };
    let request: CommandRequest = match serde_json::from_slice::<Value>(&body)
        .ok()
        .filter(|_| body.len() <= 65536)
        .and_then(|v| serde_json::from_value(v).ok())
    {
        Some(r) => r,
        None if body.is_empty() => CommandRequest::default(),
        None => {
            return error(
                RefusalCode::InvalidRequest,
                spec.name,
                "The body must be a CommandRequest.",
                request_id,
            );
        }
    };
    let c = match caller(&state, &s, &headers).await {
        Ok(c) => c,
        Err(code) => {
            return error(
                code,
                spec.name,
                "The caller is not authenticated.",
                request_id,
            );
        }
    };
    let principal = c.principal();
    let Some(route) = principal.and_then(|p| spec.route(p)) else {
        let code = if matches!(c, Caller::Agent(_)) && spec.owner_only() {
            RefusalCode::OwnerOnly
        } else {
            RefusalCode::NotAvailable
        };
        let r = Refusal::error(
            code,
            spec.name,
            "This command has no route for this principal.",
        );
        audit(
            &state,
            &c,
            spec,
            None,
            &Decision::Deny(r.clone()),
            request.operation_id,
            request_id,
        )
        .await;
        return refuse(r, request_id);
    };
    let mut request = request;
    let binding = match &c {
        Caller::Agent(a) => {
            let checked = a.authorize(spec, &mut request.params);
            audit(
                &state,
                &c,
                spec,
                Some(&checked.1),
                &checked.0,
                request.operation_id,
                request_id,
            )
            .await;
            if let Decision::Deny(r) = checked.0 {
                return refuse(r, request_id);
            }
            Some(a.binding.clone())
        }
        _ => {
            let allow = Decision::Allow(aq_agent_grants::Grant::new(
                "owner",
                aq_agent_grants::Scope::ProjectSelf,
            ));
            audit(
                &state,
                &c,
                spec,
                None,
                &allow,
                request.operation_id,
                request_id,
            )
            .await;
            None
        }
    };
    if route.body == registry::Body::Local {
        return (
            StatusCode::OK,
            [("Cache-Control", "no-store")],
            Json(whoami(&c)),
        )
            .into_response();
    }
    let f = match registry::forward(spec, route, &request, binding.as_ref(), request_id) {
        Ok(f) => f,
        Err(e) => return error(RefusalCode::InvalidRequest, spec.name, e.0, request_id),
    };
    let mut forwarded = Request::builder().method(f.method).uri(&f.uri);
    for (k, v) in &headers {
        if !matches!(
            k.as_str(),
            "content-length" | "transfer-encoding" | "content-type" | "host"
        ) {
            forwarded = forwarded.header(k, v);
        }
    }
    forwarded = forwarded.header("X-AQ-Refusal-Version", "1");
    let forwarded = match f.body {
        Some(b) => forwarded
            .header("content-type", "application/json")
            .body(Body::from(b.to_string())),
        None => forwarded.body(Body::empty()),
    };
    let Ok(forwarded) = forwarded else {
        return error(
            RefusalCode::InvalidRequest,
            spec.name,
            "The command request is not routable.",
            request_id,
        );
    };
    // The command layer has checked and audited this call; the route's own
    // check still runs but does not audit it again.
    let mut response = match grants::DISPATCHED
        .scope((), s.inner.clone().oneshot(forwarded))
        .await
    {
        Ok(r) => r.into_response(),
        Err(never) => match never {},
    };
    if let Ok(v) = HeaderValue::from_str(spec.name) {
        response.headers_mut().insert("X-AQ-Command", v);
    }
    response
}

async fn audit(
    state: &AppState,
    c: &Caller,
    spec: &registry::CommandSpec,
    target: Option<&aq_agent_grants::Target>,
    decision: &Decision,
    operation_id: Option<Uuid>,
    request_id: Uuid,
) {
    let (kind, id, ids) = match c {
        Caller::Owner(u) => ("owner", u.id, None),
        Caller::Viewer(u) => ("viewer", u.id, None),
        Caller::Supervisor(p) => ("supervisor", p.session_id, None),
        Caller::Agent(a) => ("agent", a.ids.attempt_id, Some(a.ids)),
    };
    let mut e = AuditEntry {
        caller_kind: kind,
        caller_id: id,
        attempt_id: ids.map(|i| i.attempt_id),
        task_id: ids.map(|i| i.task_id),
        project_id: ids.map(|i| i.project_id),
        command: spec.name.into(),
        grant: spec.grant.map(Into::into),
        target: target.map_or(json!({}), |t| t.audit()),
        allowed: matches!(decision, Decision::Allow(_)),
        reason_code: None,
        operation_id,
        request_id,
    };
    if let Decision::Deny(r) = decision {
        e.reason_code = Some(r.code.as_str());
    }
    grants::audit(state, &e).await;
}
