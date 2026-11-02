SELECT
 (SELECT count(*)<=341 AND COALESCE(sum(reserved_bytes),0)<=67108864 FROM profiled_settlement_evaluations)
 AND NOT EXISTS(SELECT 1 FROM profiled_settlement_evaluations GROUP BY attempt_id HAVING count(*)>16 OR count(*) FILTER(WHERE disposition='accepted')>1)
 AND NOT EXISTS(SELECT 1 FROM profiled_settlement_evaluations e
 LEFT JOIN profiled_result_reports r ON r.report_id=e.report_id AND r.attempt_id=e.attempt_id
 LEFT JOIN profiled_stopped_receipts stopped ON stopped.attempt_id=e.attempt_id
 LEFT JOIN source_transfers s ON s.import_operation_id=e.source_operation_id AND s.attempt_id=e.attempt_id
 LEFT JOIN human_decision_requests d ON d.decision_id=e.decision_id
 LEFT JOIN human_decision_finality f ON f.decision_id=d.decision_id
 LEFT JOIN review_subjects subject ON subject.subject_digest=d.subject_digest AND subject.report_id=e.report_id
 LEFT JOIN users actor ON actor.id=e.actor_id
 WHERE r.report_id IS NULL OR stopped.attempt_id IS NULL OR s.transfer_id IS NULL OR f.decision_id IS NULL OR subject.subject_digest IS NULL OR actor.id IS NULL
 OR subject.subject_profile<>'source_and_effects_review_v1' OR s.state<>'retained' OR s.node_id<>r.node_id
 OR e.request->>'operation_id' IS DISTINCT FROM e.operation_id::TEXT OR e.request->>'report_id' IS DISTINCT FROM e.report_id::TEXT
 OR e.request->>'decision_id' IS DISTINCT FROM e.decision_id::TEXT OR e.request->>'source_operation_id' IS DISTINCT FROM e.source_operation_id::TEXT
 OR e.request#>>'{subject,digest}' IS DISTINCT FROM d.subject_digest OR e.receipt->'subject' IS DISTINCT FROM e.request->'subject'
 OR e.receipt->>'operation_id' IS DISTINCT FROM e.operation_id::TEXT OR e.receipt->>'attempt_id' IS DISTINCT FROM e.attempt_id::TEXT
 OR e.receipt->>'disposition' IS DISTINCT FROM e.disposition OR e.receipt->>'release_id' IS DISTINCT FROM e.release_id::TEXT
 OR (e.disposition='accepted' AND (
  NOT EXISTS(SELECT 1 FROM profiled_result_heads WHERE attempt_id=e.attempt_id AND report_id=e.report_id)
  OR NOT EXISTS(SELECT 1 FROM profiled_verified_releases r JOIN profiled_task_completions c USING(attempt_id,release_id) WHERE r.attempt_id=e.attempt_id AND r.release_id=e.release_id AND r.settlement_operation_id=e.operation_id AND c.evaluation_operation_id=e.operation_id AND c.operation_id::TEXT=e.receipt->>'completion_operation_id')
  OR NOT EXISTS(SELECT 1 FROM profiled_verified_releases release JOIN profiled_authority_hot hot USING(attempt_id) JOIN profiled_stopped_receipts stopped USING(attempt_id)
    WHERE release.attempt_id=e.attempt_id AND release.release_id=e.release_id AND release.stop_operation_id=stopped.operation_id
    AND hot.state->>'status'='finished' AND hot.state#>>'{stop,kind}'='stopped' AND hot.state#>>'{retention,kind}'='retained' AND hot.state#>>'{effects,kind}'='quiescent'
    AND release.stop_evidence_digest=hot.state#>>'{stop,receipt,digest}' AND release.source_receipt_digest=hot.state#>>'{retention,receipt,digest}' AND release.effect_receipt_digest=hot.state#>>'{effects,receipt,digest}')
  OR NOT EXISTS(SELECT 1 FROM agent_launch_bindings WHERE attempt_id=e.attempt_id)
  OR EXISTS(SELECT 1 FROM agent_launch_bindings b JOIN mailbox_operations m ON m.sender_key=settlement_sender_key(b.binding) WHERE b.attempt_id=e.attempt_id)
  OR EXISTS(SELECT 1 FROM agent_graph_operations o WHERE o.attempt_id=e.attempt_id AND (o.phase NOT IN ('confirmed','conflict','rejected') OR o.lease_id IS NOT NULL OR o.lease_expires_at_ms IS NOT NULL))
 )))
 AND NOT EXISTS(SELECT 1 FROM profiled_task_completions c LEFT JOIN profiled_settlement_evaluations e ON e.operation_id=c.evaluation_operation_id AND e.attempt_id=c.attempt_id LEFT JOIN profiled_verified_releases r ON r.attempt_id=c.attempt_id AND r.release_id=c.release_id LEFT JOIN profiled_launches l ON l.attempt_id=c.attempt_id LEFT JOIN execution_attempts a ON a.attempt_id=c.attempt_id
 WHERE e.operation_id IS NULL OR r.release_id IS NULL OR l.attempt_id IS NULL OR a.attempt_id IS NULL OR e.disposition<>'accepted' OR e.release_id<>c.release_id OR r.settlement_operation_id<>e.operation_id
 OR e.receipt->>'completion_operation_id' IS DISTINCT FROM c.operation_id::TEXT OR c.workspace_id::TEXT IS DISTINCT FROM l.execution_spec#>>'{task,workspace_id}' OR c.task_id::TEXT IS DISTINCT FROM l.execution_spec#>>'{task,task_id}'
 OR c.expected_version::TEXT IS DISTINCT FROM l.execution_spec#>>'{task,version}' OR c.expected_content_revision::TEXT IS DISTINCT FROM l.execution_spec#>>'{task,content_revision}'
 OR (c.state<>'confirmed' AND NOT a.holds_task) OR (c.state='confirmed' AND (a.holds_task OR c.receipt IS NULL OR c.outbox IS NULL)))
 AND NOT EXISTS(SELECT 1 FROM profiled_verified_releases r LEFT JOIN profiled_settlement_evaluations e ON e.operation_id=r.settlement_operation_id AND e.attempt_id=r.attempt_id WHERE r.settlement_operation_id IS NOT NULL AND (e.operation_id IS NULL OR e.disposition<>'accepted' OR e.release_id<>r.release_id))
