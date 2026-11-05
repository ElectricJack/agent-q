-- B4 (MS1 retry): a replacement that was offered but never granted (a valid
-- never-granted closure, no task claim) linked the shared predecessor export
-- and then blocked every later successor of the same task, because the 0053
-- guard required each previous consumer to be a released failed attempt. A
-- never-granted attempt of the same task never ran, so it no longer blocks;
-- every other consumer rule is unchanged.
CREATE OR REPLACE FUNCTION profiled_replacement_source_guard() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE task UUID;
BEGIN
 SELECT task_id INTO STRICT task FROM execution_attempts WHERE attempt_id=NEW.attempt_id;
 IF EXISTS(
  SELECT 1 FROM profiled_launch_source_exports x
  JOIN execution_attempts a USING(attempt_id)
  LEFT JOIN profiled_replacement_releases r USING(attempt_id)
  LEFT JOIN profiled_authority_hot h USING(attempt_id)
  WHERE x.operation_id=NEW.operation_id AND x.attempt_id<>NEW.attempt_id
   AND NOT (a.task_id=task AND NOT a.holds_task AND profiled_never_granted_valid(a.attempt_id))
   AND (a.task_id<>task OR a.holds_task OR r.task_id IS DISTINCT FROM task
        OR r.retained_source_receipt_digest IS DISTINCT FROM h.state#>>'{retention,receipt,digest}')
 ) THEN RAISE EXCEPTION 'shared source requires released predecessor retention receipt' USING ERRCODE='23514'; END IF;
 RETURN NEW;
END $$;
UPDATE meta SET value='95' WHERE key='state_format_version';
