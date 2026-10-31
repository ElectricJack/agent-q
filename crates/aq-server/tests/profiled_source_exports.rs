//! Real SQL reservations with synthetic source-observation bytes. This does not
//! qualify Git export I/O or manufacture a verified source export receipt.
mod common;
use aq_admission::{
    ArtifactKind, ArtifactRef, Digest, ObjectFormat, SourceAdmission, SourceSelection, Text,
};
use aq_server::profiled_execution::{
    artifacts,
    config::{ProfiledConfig, ProjectPolicy},
    policies,
    source_exports::{self, Request, Reservation},
};
use aq_server::source_retention::{Runtime, SourceRetentionConfig};
use common::profiled::{Fixture, NOW, fixture, insert_typed};
use std::sync::{Arc, atomic::Ordering};
use uuid::Uuid;
async fn setup() -> (Fixture, Request, tempfile::TempDir) {
    let mut f = fixture().await;
    let dir = tempfile::tempdir().unwrap();
    let source = f.profiled.source.clone();
    let project = f.selection.lineage.project_id;
    let handle = Uuid::new_v4();
    let config = ProfiledConfig {
        revision: 1,
        qualified_runtimes: vec![],
        context_qualifications: vec![],
        projects: vec![ProjectPolicy {
            project_id: project,
            enabled: true,
            priority: 0,
            source: SourceSelection::Immutable {
                object: aq_admission::ObjectId {
                    format: ObjectFormat::Sha1,
                    hex: source.base.hex.clone(),
                },
            },
            retention_handle: handle,
        }],
    };
    let policy = config.project_policy_record(project).unwrap();
    let mut cfg = (*f.state.config).clone();
    cfg.profiled_execution = Some(Arc::new(config));
    f.state.config = Arc::new(cfg);
    let max_bundle = 65536;
    f.state.source_retention = Some(Arc::new(
        Runtime::new(SourceRetentionConfig {
            git_socket: dir.path().join("absent.sock"),
            git_root: dir.path().join("git"),
            staging_root: dir.path().join("stage"),
            max_bundle_bytes: max_bundle,
            max_staging_bytes: max_bundle + 1_048_576 + 4096,
            bulk_concurrency: 1,
            incomplete_ttl_secs: 60,
        })
        .unwrap(),
    ));
    let admission = SourceAdmission {
        schema_version: 1,
        project_id: project,
        project_revision: 1,
        repository_id: source.repository_id,
        repository_binding_generation: source.binding_generation,
        policy: policy.artifact().unwrap(),
        selection: policy.source,
        resolved_object: aq_admission::ObjectId {
            format: ObjectFormat::Sha1,
            hex: source.base.hex.clone(),
        },
        observed_at_ms: NOW - 10,
        expires_at_ms: NOW + 29990,
    };
    let mut value = serde_json::json!({"schema_version":1,"admission":admission,"retention_handle":handle,"evidence":{"synthetic_component_fixture":true}});
    value.sort_all_objects();
    let bytes = serde_json::to_vec(&value).unwrap();
    let artifact = ArtifactRef {
        id: Text::new(format!(
            "aq.source-admission-observation/{}",
            Digest::of(&bytes).as_str()
        ))
        .unwrap(),
        version: Text::new("1").unwrap(),
        digest: Digest::of(&bytes),
    };
    let task_context =
        common::profiled::artifact("synthetic-source-owner-task-context", b"synthetic task");
    f.profiled
        .offer
        .manifest
        .context
        .references
        .push(aq_session_protocol::ContextRef {
            artifact: task_context.clone(),
            provenance: Text::new("synthetic-source-owner-component").unwrap(),
            delivery: aq_session_protocol::ContextDelivery::Startup,
            bytes: 14,
            tokens: 4,
            required: true,
        });
    f.profiled.offer.manifest.bindings.source = artifact.clone();
    let selection_digest = aq_selection::digest(&f.selection).unwrap();
    f.profiled.offer.manifest.selection_explanation = ArtifactRef {
        id: Text::new("sealed-selection").unwrap(),
        version: Text::new("1").unwrap(),
        digest: selection_digest.clone(),
    };
    f.profiled.offer.binding.launch_manifest_digest =
        aq_session_protocol::canonical_digest(&f.profiled.offer.manifest).unwrap();
    let spec = aq_admission::execution_spec::ProfiledExecutionSpec {
        schema_version: 1,
        lifecycle: aq_admission::transport::lifecycle_binding(&f.profiled.offer).unwrap(),
        project_revision: 1,
        task: aq_admission::TaskRevision {
            workspace_id: f.selection.lineage.workspace_id,
            task_id: f.selection.lineage.task_id,
            version: f.profiled.offer.manifest.bindings.task_revision,
            content_revision: 1,
        },
        source: admission.clone(),
        source_artifact: artifact.clone(),
        outcome_artifact: f.profiled.offer.manifest.bindings.outcome.clone(),
        graph_access_profile: f
            .profiled
            .offer
            .manifest
            .bindings
            .graph_access_profile
            .clone(),
        task_observation: common::profiled::artifact(
            "synthetic-task-observation",
            b"graph-context",
        ),
        task_context,
        selection_digest,
    };
    f.profiled.offer.binding.execution_spec_digest = spec.artifact().unwrap().digest;
    insert_typed(&f, &spec).await;
    policies::install(&f.state).await.unwrap();
    artifacts::retain(
        &f.state,
        artifacts::Retain {
            artifact_id: Uuid::new_v4(),
            project_id: project,
            owner: artifacts::OwnerKind::SourceRetention,
            artifact: artifact.clone(),
            kind: ArtifactKind::Context,
            bytes: &bytes,
            retain_until_ms: NOW + 600000,
        },
    )
    .await
    .unwrap();
    let request = Request {
        schema_version: 1,
        operation_id: Uuid::new_v4(),
        project_id: project,
        project_revision: 1,
        repository_id: source.repository_id,
        logical_identity: source.logical_identity.clone(),
        binding_generation: source.binding_generation,
        base: source.base.clone(),
        source_retention_handle: handle,
        source_observation: artifact,
    };
    (f, request, dir)
}
#[tokio::test]
async fn durable_reservation_precedes_io_exact_retry_survives_capacity_without_grant() {
    let (f, request, _dir) = setup().await;
    let pool = f.db.pool().await;
    let before: i64 = sqlx::query_scalar("SELECT count(*) FROM execution_attempts")
        .fetch_one(&pool)
        .await
        .unwrap();
    let Reservation::Pending(token) = source_exports::reserve(&f.state, request.clone())
        .await
        .unwrap()
    else {
        panic!("no I/O completion exists")
    };
    assert_eq!(token.request_digest(), &request.digest().unwrap());
    assert_eq!(token.reserved_bytes(), 1_114_112);
    assert!(matches!(
        source_exports::reserve(&f.state, request.clone())
            .await
            .unwrap(),
        Reservation::Pending(_)
    ));
    let mut another = request.clone();
    another.operation_id = Uuid::new_v4();
    assert!(source_exports::reserve(&f.state, another).await.is_err());
    let mut conflict = request.clone();
    conflict.logical_identity = "different".into();
    assert!(source_exports::reserve(&f.state, conflict).await.is_err());
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM execution_attempts")
            .fetch_one(&pool)
            .await
            .unwrap(),
        before
    );
    assert_eq!(sqlx::query_scalar::<_,i64>("SELECT count(*) FROM profiled_source_exports WHERE state='pending' AND verified_bytes IS NULL AND charged_bytes=reserved_bytes").fetch_one(&pool).await.unwrap(),1);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM profiled_owner_journals")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
}
#[tokio::test]
async fn pending_unknown_export_is_not_pruned_and_stale_observation_cannot_reserve_new_operation() {
    let (f, request, _dir) = setup().await;
    let pool = f.db.pool().await;
    source_exports::reserve(&f.state, request.clone())
        .await
        .unwrap();
    f.time.store((NOW + 31000) as i64, Ordering::SeqCst);
    assert!(matches!(
        source_exports::reserve(&f.state, request.clone())
            .await
            .unwrap(),
        Reservation::Pending(_)
    ));
    let mut another = request.clone();
    another.operation_id = Uuid::new_v4();
    assert!(source_exports::reserve(&f.state, another).await.is_err());
    assert!(
        sqlx::query("DELETE FROM profiled_source_exports WHERE operation_id=$1")
            .bind(request.operation_id)
            .execute(&pool)
            .await
            .is_err()
    );
    assert!(
        sqlx::query("UPDATE profiled_source_exports SET charged_bytes=1 WHERE operation_id=$1")
            .bind(request.operation_id)
            .execute(&pool)
            .await
            .is_err()
    );
    assert!(
        sqlx::query("UPDATE profiled_source_exports SET request_digest=$2 WHERE operation_id=$1")
            .bind(request.operation_id)
            .bind("f".repeat(64))
            .execute(&pool)
            .await
            .is_err()
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT sum(charged_bytes)::BIGINT FROM profiled_source_exports"
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        1_114_112
    );
}
#[tokio::test]
async fn unknown_source_scope_and_unretained_observation_never_mint_owned_io_token() {
    let (f, request, _dir) = setup().await;
    let mut changed = request.clone();
    changed.source_observation.digest = Digest::of(b"missing");
    assert!(source_exports::reserve(&f.state, changed).await.is_err());
    changed = request.clone();
    changed.base.hex = "c".repeat(40);
    assert!(source_exports::reserve(&f.state, changed).await.is_err());
    changed = request;
    changed.source_retention_handle = Uuid::new_v4();
    assert!(source_exports::reserve(&f.state, changed).await.is_err());
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM profiled_source_exports")
            .fetch_one(&f.db.pool().await)
            .await
            .unwrap(),
        0
    );
}
#[tokio::test]
async fn reservation_clock_is_rechecked_after_observed_quota_lock_wait() {
    let (f, request, _dir) = setup().await;
    let pool = f.db.pool().await;
    let mut blocker = pool.begin().await.unwrap();
    sqlx::query("SELECT pg_advisory_xact_lock(719247031)")
        .execute(&mut *blocker)
        .await
        .unwrap();
    let blocker_pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *blocker)
        .await
        .unwrap();
    let state = f.state.clone();
    let pending = tokio::spawn(async move { source_exports::reserve(&state, request).await });
    tokio::time::timeout(std::time::Duration::from_secs(1),async {
        loop {
            let waiting:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE datname=current_database() AND $1=ANY(pg_blocking_pids(pid)) AND wait_event_type='Lock')").bind(blocker_pid).fetch_one(&pool).await.unwrap();
            if waiting {break;}
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    }).await.expect("observed actual quota lock wait");
    f.time.store((NOW + 31000) as i64, Ordering::SeqCst);
    blocker.commit().await.unwrap();
    assert!(pending.await.unwrap().is_err());
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM profiled_source_exports")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
}

#[tokio::test]
async fn retired_policy_allows_only_original_fixed_scope_pending_reconciliation() {
    let (mut f, request, _dir) = setup().await;
    source_exports::reserve(&f.state, request.clone())
        .await
        .unwrap();
    let mut config = (*f.state.config).clone();
    let mut native = (**config.profiled_execution.as_ref().unwrap()).clone();
    native.revision += 1;
    native.projects[0].enabled = false;
    config.profiled_execution = Some(Arc::new(native));
    f.state.config = Arc::new(config);
    policies::install(&f.state).await.unwrap();
    f.time.store((NOW + 31000) as i64, Ordering::SeqCst);
    let Reservation::Pending(original) = source_exports::reserve(&f.state, request.clone())
        .await
        .unwrap()
    else {
        panic!("reconciliation is not completion")
    };
    assert_eq!(original.request_digest(), &request.digest().unwrap());
    let mut changed = request.clone();
    changed.operation_id = Uuid::new_v4();
    assert!(source_exports::reserve(&f.state, changed).await.is_err());
    changed = request;
    changed.logical_identity = "different-scope".into();
    assert!(source_exports::reserve(&f.state, changed).await.is_err());
    assert_eq!(sqlx::query_scalar::<_,i64>("SELECT count(*) FROM profiled_source_exports WHERE state='pending' AND verified_bytes IS NULL AND charged_bytes=reserved_bytes").fetch_one(&f.db.pool().await).await.unwrap(),1);
}
// Structural component fixture only. Actual Git export/hash/retention is
// independently qualified by source_retention::profiled_base's pinned-peer test.
fn synthetic_fact(
    token: &source_exports::ReservedExport,
) -> aq_server::source_retention::profiled_base::VerifiedExport {
    let r = token.request();
    let producer = Uuid::new_v4();
    let bundle = b"synthetic component bundle";
    let manifest = serde_json::json!({"format_version":1,"repository_id":r.repository_id,"repository_identity":r.logical_identity,"binding_generation":r.binding_generation,"transfer_id":r.operation_id,"object_format":"sha1","base":r.base,"source":r.base,"prerequisites":[],"bundle_size":bundle.len(),"bundle_sha256":Digest::of(bundle),"producer_installation_id":producer,"reference_only":false,"caller_correlation":{"owner":"aq.profiled-base-v1","project_id":r.project_id,"project_revision":r.project_revision,"source_observation":r.source_observation}});
    let raw = serde_json::to_string(&manifest).unwrap();
    let retention = serde_json::json!({"version":1,"id":r.operation_id,"repository_id":r.repository_id,"binding_generation":r.binding_generation,"generation":1,"resource":"transfer","refs":[{"name":format!("refs/aq-git/transfers/{}",r.operation_id),"oid":r.base.hex}],"state":"active","dependencies":[],"release_operation":null,"release_reason":null});
    let mut value = serde_json::json!({"schema_version":1,"request_digest":token.request_digest(),"export_operation_id":r.operation_id,"manifest_json":raw,"manifest_digest":Digest::of(raw.as_bytes()),"bundle_bytes":bundle.len(),"bundle_digest":Digest::of(bundle),"producer_installation_id":producer,"export_response":{"artifact_handle":r.operation_id,"retention_handle":r.operation_id,"manifest":manifest},"retention_inspection":retention,"observed_at_ms":NOW,"expires_at_ms":NOW+30000});
    value.sort_all_objects();
    let raw = serde_json::to_vec(&value).unwrap();
    aq_server::source_retention::profiled_base::VerifiedExport::decode_stored(token, &raw).unwrap()
}
#[tokio::test]
async fn completion_retains_original_fact_releases_only_excess_reservation_and_refuses_refresh() {
    let (f, request, _dir) = setup().await;
    let pool = f.db.pool().await;
    let Reservation::Pending(token) = source_exports::reserve(&f.state, request.clone())
        .await
        .unwrap()
    else {
        panic!()
    };
    let fact = synthetic_fact(&token);
    let original = fact.canonical_bytes().unwrap();
    source_exports::finalize(&f.state, &token, &fact)
        .await
        .unwrap();
    source_exports::finalize(&f.state, &token, &fact)
        .await
        .unwrap();
    let charged: i64 = sqlx::query_scalar(
        "SELECT charged_bytes FROM profiled_source_exports WHERE operation_id=$1",
    )
    .bind(request.operation_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        charged as u64,
        fact.bundle_bytes() + fact.manifest_bytes().len() as u64
    );
    let mut another = request.clone();
    another.operation_id = Uuid::new_v4();
    assert!(matches!(
        source_exports::reserve(&f.state, another).await.unwrap(),
        Reservation::Pending(_)
    ));
    let mut altered: serde_json::Value = serde_json::from_slice(&original).unwrap();
    altered["observed_at_ms"] = serde_json::json!(NOW + 1);
    altered["expires_at_ms"] = serde_json::json!(NOW + 30001);
    altered.sort_all_objects();
    let changed = aq_server::source_retention::profiled_base::VerifiedExport::decode_stored(
        &token,
        &serde_json::to_vec(&altered).unwrap(),
    )
    .unwrap();
    assert!(
        source_exports::finalize(&f.state, &token, &changed)
            .await
            .is_err()
    );
    f.time.store((NOW + 31000) as i64, Ordering::SeqCst);
    let mut historical = f.state.clone();
    historical.source_retention = None;
    let Reservation::Verified {
        canonical_verified, ..
    } = source_exports::reserve(&historical, request).await.unwrap()
    else {
        panic!()
    };
    assert_eq!(canonical_verified, original);
}
#[tokio::test]
async fn linked_attempt_wrapper_has_zero_extra_charge_only_for_exact_verified_export() {
    let (f, request, _dir) = setup().await;
    let pool = f.db.pool().await;
    let Reservation::Pending(token) = source_exports::reserve(&f.state, request.clone())
        .await
        .unwrap()
    else {
        panic!()
    };
    let fact = synthetic_fact(&token);
    source_exports::finalize(&f.state, &token, &fact)
        .await
        .unwrap();
    let mut observation = serde_json::json!({"request_digest":token.request_digest(),"export_operation_id":request.operation_id,"inspection":serde_json::from_slice::<serde_json::Value>(&fact.canonical_bytes().unwrap()).unwrap()["retention_inspection"],"observed_at_ms":NOW,"expires_at_ms":NOW+30000});
    observation.sort_all_objects();
    let retained = serde_json::from_value(observation).unwrap();
    let mut tx = pool.begin().await.unwrap();
    let manifest = source_exports::link_locked(
        &f.state,
        &mut tx,
        &f.profiled.offer.binding.session,
        &token,
        &fact,
        &retained,
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    assert_eq!(manifest.artifact_id, request.operation_id);
    use tower::ServiceExt;
    let router = aq_server::build_worker_router(f.state.clone());
    for suffix in [String::new(), "/objects.bundle?offset=0&limit=1024".into()] {
        let response = router
            .clone()
            .oneshot(
                axum::http::Request::builder()
                    .uri(format!(
                        "/v1/execution/sources/base/{}{}",
                        f.profiled.offer.binding.attempt_id, suffix
                    ))
                    .header("authorization", format!("Bearer {}", f.credential))
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), axum::http::StatusCode::FORBIDDEN);
    }

    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT reserved_bytes FROM source_bases WHERE attempt_id=$1")
            .bind(f.profiled.offer.binding.attempt_id)
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    assert!(sqlx::query("UPDATE source_bases SET manifest=jsonb_set(manifest,'{bundle_digest}',to_jsonb($2::TEXT)) WHERE attempt_id=$1").bind(f.profiled.offer.binding.attempt_id).bind("f".repeat(64)).execute(&pool).await.is_err());
    assert!(
        sqlx::query("UPDATE profiled_source_exports SET charged_bytes=1 WHERE operation_id=$1")
            .bind(request.operation_id)
            .execute(&pool)
            .await
            .is_err()
    );
    assert!(sqlx::query("UPDATE source_bases SET profiled_export_operation_id=NULL,reserved_bytes=1 WHERE attempt_id=$1").bind(f.profiled.offer.binding.attempt_id).execute(&pool).await.is_err());
    assert!(
        sqlx::query("DELETE FROM source_bases WHERE attempt_id=$1")
            .bind(f.profiled.offer.binding.attempt_id)
            .execute(&pool)
            .await
            .is_err()
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM profiled_owner_journals")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
}
#[tokio::test]
async fn source_link_rechecks_freshness_after_observed_owned_export_lock_wait() {
    let (f, request, _dir) = setup().await;
    let pool = f.db.pool().await;
    let Reservation::Pending(token) = source_exports::reserve(&f.state, request.clone())
        .await
        .unwrap()
    else {
        panic!()
    };
    let fact = synthetic_fact(&token);
    source_exports::finalize(&f.state, &token, &fact)
        .await
        .unwrap();
    let value = serde_json::json!({"request_digest":token.request_digest(),"export_operation_id":request.operation_id,"inspection":serde_json::from_slice::<serde_json::Value>(&fact.canonical_bytes().unwrap()).unwrap()["retention_inspection"],"observed_at_ms":NOW,"expires_at_ms":NOW+30000});
    let retained = serde_json::from_value(value).unwrap();
    let mut blocker = pool.begin().await.unwrap();
    sqlx::query(
        "SELECT operation_id FROM profiled_source_exports WHERE operation_id=$1 FOR UPDATE",
    )
    .bind(request.operation_id)
    .execute(&mut *blocker)
    .await
    .unwrap();
    let blocker_pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *blocker)
        .await
        .unwrap();
    let state = f.state.clone();
    let session = f.profiled.offer.binding.session.clone();
    let pending_pool = f.db.pool().await;
    let pending = tokio::spawn(async move {
        let mut tx = pending_pool.begin().await.unwrap();
        let result =
            source_exports::link_locked(&state, &mut tx, &session, &token, &fact, &retained).await;
        if result.is_ok() {
            tx.commit().await.unwrap();
        }
        result
    });
    let observed=tokio::time::timeout(std::time::Duration::from_secs(1),async {
        loop {
            let waiting:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE datname=current_database() AND $1=ANY(pg_blocking_pids(pid)) AND wait_event_type='Lock')").bind(blocker_pid).fetch_one(&pool).await.unwrap();
            if waiting {break;}tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    }).await;
    if observed.is_err() {
        blocker.rollback().await.unwrap();
        panic!(
            "no observed owner wait; task outcome {:?}",
            pending.await.unwrap()
        );
    }
    f.time.store((NOW + 31000) as i64, Ordering::SeqCst);
    blocker.commit().await.unwrap();
    assert!(pending.await.unwrap().is_err());
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM profiled_launch_source_exports")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM source_bases")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
}
#[tokio::test]
async fn worst_case_pending_metadata_is_bounded_and_original_replay_remains_available() {
    let (mut f, request, _dir) = setup().await;
    let mut config = f.state.source_retention.as_ref().unwrap().config.clone();
    config.max_staging_bytes = 268_435_456;
    f.state.source_retention = Some(Arc::new(Runtime::new(config).unwrap()));
    let mut accepted = 0;
    for _ in 0..64 {
        let mut next = request.clone();
        next.operation_id = if accepted == 0 {
            request.operation_id
        } else {
            Uuid::new_v4()
        };
        match source_exports::reserve(&f.state, next).await {
            Ok(Reservation::Pending(_)) => accepted += 1,
            Err(aq_server::error::AppError::PayloadTooLarge(_)) => break,
            _ => panic!("unexpected source reservation outcome"),
        }
    }
    assert!(accepted > 0 && accepted < 64);
    let bytes:i64=sqlx::query_scalar("SELECT sum(octet_length(request_bytes)+COALESCE(octet_length(verified_bytes),4194304))::BIGINT FROM profiled_source_exports").fetch_one(&f.db.pool().await).await.unwrap();
    assert!(bytes <= 67_108_864);
    assert!(matches!(
        source_exports::reserve(&f.state, request).await.unwrap(),
        Reservation::Pending(_)
    ));
}
