"""Small fixture oracle for API-01 invariants, deliberately not a production gateway.

No network/client implementation, persistent authority, graph engine or credential
verification is provided. AQ-06/07 must implement and test those boundaries.
"""
from copy import deepcopy

from validate import canonical, digest, upstream_validate, validate, strict_loads


class Refused(ValueError):
    def __init__(self, code):
        self.code = code
        super().__init__(code)


def binding_check(binding, expected):
    if binding != expected:
        raise Refused('stale_binding')


def active_agent(principal, context):
    if principal['kind'] != 'agent_session':
        raise Refused('unauthorized')
    binding_check(principal['binding'], context['binding'])
    if not principal['active']:
        raise Refused('session_expired_or_revoked')


def readable(node_id, context, graph, created=()):
    node = graph['nodes'].get(node_id)
    if node is None or node['workspace_id'] != context['binding']['workspace_id']:
        return False
    scope = context['profile']['read_scope']
    return scope['mode'] == 'workspace' or node_id in scope['record_ids'] or (
        scope['include_created_by_attempt'] and node_id in created)


def require_read(node_id, context, graph, created=()):
    if not readable(node_id, context, graph, created):
        # Same response for nonexistent and out-of-scope IDs; no hidden fields.
        raise Refused('not_found')


def require_write(node_id, context, graph, created=()):
    require_read(node_id, context, graph, created)
    if node_id not in context['profile']['write_scope']['record_ids'] and node_id not in created:
        raise Refused('scope_denied')


def scoped_node(raw):
    upstream_validate('Node', raw)
    result = {k: deepcopy(raw[k]) for k in ['id', 'kind', 'title', 'body', 'acceptance_criteria', 'labels', 'task_state',
                                          'verification', 'content_type', 'retired', 'deleted']}
    result.update(version=str(raw['version']), content_revision=str(raw['content_revision']))
    return validate('ScopedNode', result)


def scoped_edge(raw):
    upstream_validate('Edge', raw)
    result = {k: deepcopy(raw[k]) for k in ['id', 'source', 'target', 'edge_type']}
    result.update(version=str(raw['version']), description=raw.get('description'))
    for field in ['source_revision', 'target_revision']:
        result[field] = None if raw[field] is None else str(raw[field])
    return validate('ScopedEdge', result)


def visible_edge(edge, context, graph):
    return edge['workspace_id'] == context['binding']['workspace_id'] and edge['edge_type'] in context['profile']['read_scope']['edge_types'] and all(
        readable(edge[k], context, graph) for k in ['source', 'target'])


def project_context(raw, request, context, graph):
    """Projects a retained realistic upstream fixture, not an HTTP traversal."""
    validate('ReadRequest', request)
    binding_check(request['binding'], context['binding'])
    query = request['query']
    if query['kind'] != 'context':
        raise Refused('invalid_request')
    upstream_validate('Context', raw)
    require_read(query['node_id'], context, graph)
    if raw['node']['id'] != query['node_id']:
        raise Refused('upstream_binding_mismatch')
    if any(n['workspace_id'] != context['binding']['workspace_id'] for n in [raw['node'], *raw['nodes'], *raw['pinned_revisions']]):
        raise Refused('upstream_binding_mismatch')
    nodes = [n for n in raw['nodes'] if readable(n['id'], context, graph)]
    nodes = nodes[:query['max_nodes']-1]
    returned = {query['node_id'], *(n['id'] for n in nodes)}
    edges = [e for e in raw['edges'] if visible_edge(e, context, graph)
             and e['source'] in returned and e['target'] in returned][:query['max_edges']]
    pins = [n for n in raw['pinned_revisions'] if readable(n['id'], context, graph)
            and any((e['source'] == n['id'] and e['source_revision'] == n['content_revision'])
                    or (e['target'] == n['id'] and e['target_revision'] == n['content_revision']) for e in edges)]
    partial = raw['truncated'] or len(nodes) < len([n for n in raw['nodes'] if readable(n['id'], context, graph)])
    partial = partial or len(edges) < len([e for e in raw['edges'] if visible_edge(e, context, graph)
                                         and e['source'] in returned and e['target'] in returned])
    result = {'kind': 'context', 'node': scoped_node(raw['node']), 'nodes': [scoped_node(n) for n in nodes],
              'edges': [scoped_edge(e) for e in edges], 'pinned_revisions': [scoped_node(n) for n in pins],
              'coverage': {'status': 'partial' if partial else 'complete', 'complete_for': 'authorized_projection',
                           'reasons': ['scan_budget'] if raw['truncated'] else (['node_budget'] if partial else [])}}
    if len(canonical(result)) > min(query['max_bytes'], context['profile']['max_response_bytes']):
        raise Refused('response_too_large')
    return validate('ContextResult', result)


def authorize_read(principal, context, request, graph):
    active_agent(principal, context)
    validate('ReadRequest', request)
    binding_check(request['binding'], context['binding'])
    query = request['query']
    if 'read.' + query['kind'] not in context['profile']['capabilities']:
        raise Refused('capability_denied')
    if 'node_id' in query:
        require_read(query['node_id'], context, graph)
    if query['kind'] == 'relationships' and not set(query['edge_types']) <= set(context['profile']['read_scope']['edge_types']):
        raise Refused('scope_denied')


def require_graph_guard(node, principal):
    if node.get('deleted') or node.get('retired') or node.get('verification') != 'unverified':
        raise Refused('guard_unavailable')
    if node['kind'] == 'knowledge' and node.get('created_by') != principal.get('graph_actor_id'):
        raise Refused('guard_unavailable')


def authorize_batch(principal, context, request, graph):
    """Checks every endpoint before translating; no effects or admissions here."""
    active_agent(principal, context)
    validate('CommandRequest', request)
    binding_check(request['binding'], context['binding'])
    capabilities = context['profile']['capabilities']
    write = context['profile']['write_scope']
    created = set()
    proposed = set()
    local = deepcopy(graph)
    kinds = {'create_finding': 'write.finding', 'correct_record': 'write.correct', 'propose_decomposition': 'write.proposal',
             'add_relationship': 'write.relationships', 'remove_relationship': 'write.relationships'}
    for command in request['commands']:
        kind = command['kind']
        if kinds[kind] not in capabilities:
            raise Refused('capability_denied')
        if kind == 'create_finding':
            node_id = command['id']
            if node_id in local['nodes'] or node_id in created:
                raise Refused('id_conflict')
            created.add(node_id)
            if len(created) > write['max_created_records']:
                raise Refused('scope_denied')
            local['nodes'][node_id] = {'workspace_id': context['binding']['workspace_id'], 'kind': 'knowledge', 'created_by': principal['graph_actor_id'], 'verification': 'unverified', 'version': 1}
        elif kind == 'correct_record':
            require_write(command['id'], context, local, created)
            node = local['nodes'][command['id']]
            require_graph_guard(node, principal)
            if not set(command['patch']) <= set(write['correct_fields']):
                raise Refused('scope_denied')
            if node['kind'] == 'task' and 'write.task_content' not in capabilities:
                raise Refused('capability_denied')
            # Ordinary service principal still cannot modify protected/retired
            # nodes; a future privileged adapter must be a separate capability.
            if node.get('verification', 'unverified') != 'unverified' or node.get('retired', False):
                raise Refused('protected_record')
        elif kind in ('add_relationship', 'remove_relationship'):
            require_write(command['source'], context, local, created)
            require_read(command['target'], context, local, created)
            require_graph_guard(local['nodes'][command['source']], principal)
            if kind == 'remove_relationship':
                edge = graph['edges'].get(command['id'])
                if edge is None or edge['source'] != command['source'] or edge['target'] != command['target']:
                    raise Refused('not_found')
                edge_type = edge['edge_type']
            else:
                edge_type = command['edge_type']
                if command['id'] in graph['edges']:
                    raise Refused('id_conflict')
            if edge_type not in write['relationship_types'] or edge_type not in context['profile']['read_scope']['edge_types']:
                raise Refused('scope_denied')
        else:
            require_write(command['parent_id'], context, local)
            require_graph_guard(local['nodes'][command['parent_id']], principal)
            if local['nodes'][command['parent_id']]['kind'] != 'task':
                raise Refused('invalid_request')
            if len(command['tasks']) > write['max_proposal_tasks']:
                raise Refused('scope_denied')
            for task in command['tasks']:
                if task['id'] in local['nodes'] or task['id'] in proposed:
                    raise Refused('id_conflict')
                proposed.add(task['id'])
            guards = {g['id']: g['expected_version'] for g in command['guards']}
            if len(guards) != len(command['guards']) or command['parent_id'] in guards:
                raise Refused('invalid_request')
            for node_id in guards:
                require_write(node_id, context, local)
                require_graph_guard(local['nodes'][node_id], principal)
            edge_ids = set()
            for edge in command['relationships']:
                if edge['edge_type'] not in write['relationship_types'] or edge['edge_type'] not in context['profile']['read_scope']['edge_types']:
                    raise Refused('scope_denied')
                if edge['id'] in graph['edges'] or edge['id'] in edge_ids:
                    raise Refused('id_conflict')
                edge_ids.add(edge['id'])
                for endpoint in ('source', 'target'):
                    node_id = edge[endpoint]
                    if node_id not in proposed:
                        # Structural changes affect both endpoints, including
                        # existing endpoints in a proposal. Read-only is insufficient.
                        require_write(node_id, context, local)
                        if node_id != command['parent_id'] and node_id not in guards:
                            raise Refused('missing_precondition')
                    if node_id in local['nodes'] and local['nodes'][node_id]['kind'] != 'task':
                        raise Refused('invalid_request')
    # A protection/author observation authorizes only that exact public version.
    # In particular, a future CAS token cannot bless a later protected state.
    for command in request['commands']:
        checks = []
        if command['kind'] == 'correct_record':
            checks.append((command['id'], command['expected_version']))
        elif command['kind'] in ('add_relationship', 'remove_relationship'):
            checks.append((command['source'], command['expected_source_version']))
        elif command['kind'] == 'propose_decomposition':
            checks.append((command['parent_id'], command['expected_parent_version']))
            checks.extend((guard['id'], guard['expected_version']) for guard in command['guards'])
        for node_id, expected in checks:
            if local['nodes'][node_id]['version'] != int(expected):
                raise Refused('version_conflict')
    return created


def translate_fixture(request, context):
    """Deterministic representative mapping validated against retained graph v1.

    Scope/authority validation must happen before this helper. The caller cannot
    choose the workspace, service actor, idempotency key or graph operation class.
    """
    provenance = {'source_id': 'aq:' + request['binding']['attempt_id'], 'kind': 'task',
                  'task_id': request['binding']['task_id'], 'attempt_id': request['binding']['attempt_id'],
                  'session_id': request['binding']['session_id']}
    operations = []
    proposal = None
    for command in request['commands']:
        kind = command['kind']
        if kind == 'create_finding':
            operations.append({'op': 'create_node', 'id': command['id'], 'kind': 'knowledge',
                               **{k: command[k] for k in ['title', 'body', 'labels', 'content_type']}, 'provenance': [provenance]})
        elif kind == 'correct_record':
            operations.append({'op': 'update_node', 'id': command['id'], 'expected_version': int(command['expected_version']), 'patch': command['patch']})
        elif kind in ('add_relationship', 'remove_relationship'):
            # Graph AddEdge lacks source CAS. A no-op source update is an atomic
            # version guard in the SAME graph mutation transaction (not a pre-read).
            operations.append({'op': 'update_node', 'id': command['source'],
                               'expected_version': int(command['expected_source_version']), 'patch': {}})
            if kind == 'add_relationship':
                operations.append({'op': 'add_edge', **{k: command[k] for k in ['id', 'source', 'target', 'edge_type', 'description']},
                                   **{k: None if command[k] is None else int(command[k]) for k in ['source_revision', 'target_revision']}})
            else:
                operations.append({'op': 'remove_edge', 'id': command['id'], 'expected_version': int(command['expected_version'])})
        else:
            proposal = command
            operations.append({'op': 'update_node', 'id': command['parent_id'],
                               'expected_version': int(command['expected_parent_version']), 'patch': {}})
            operations.extend({'op': 'update_node', 'id': g['id'], 'expected_version': int(g['expected_version']), 'patch': {}} for g in command['guards'])
            operations.extend({'op': 'create_node', 'kind': 'task', 'task_state': 'draft', **task, 'provenance': [provenance]} for task in command['tasks'])
            operations.extend({'op': 'add_edge', **edge} for edge in command['relationships'])
    if len(operations) > 100:
        raise Refused('invalid_request')
    payload = {'idempotency_key': 'aq-agent-v1:' + request['operation_id'], 'reason': request['reason'], 'operations': operations}
    if proposal:
        payload['name'] = proposal['name']
    upstream_validate('DraftRequest' if proposal else 'MutationRequest', payload)
    return {'workspace_id': request['binding']['workspace_id'], 'service_principal': 'api01-writer',
            'operation_class': 'draft_create' if proposal else 'mutate', 'payload': payload}


class OperationLedger:
    """In-memory operation fixture; no durability or database claim."""
    def __init__(self):
        self.operations = {}
        self.confirmed_events = []

    def admit(self, principal, context, request, graph):
        active_agent(principal, context)
        validate('CommandRequest', request)
        binding_check(request['binding'], context['binding'])
        operation_id = request['operation_id']
        payload_digest = digest(request)
        if operation_id in self.operations:
            existing = self.operations[operation_id]
            if existing['request'] != canonical(request):
                raise Refused('id_conflict')
            return deepcopy(existing['status'])
        authorize_batch(principal, context, request, graph)
        translated = translate_fixture(request, context)
        status = {'api_version': 1, 'operation_id': operation_id, 'binding': request['binding'], 'payload_digest': payload_digest,
                  'state': 'pending', 'admission': 'recorded', 'effect': 'not_dispatched',
                  'recovery': {'action': 'poll_existing' if context['profile']['retry_budget']['max_attempts'] > 1 else 'manual_reconciliation',
                               'after_ms': context['profile']['retry_budget']['initial_delay_ms'] if context['profile']['retry_budget']['max_attempts'] > 1 else 0,
                               'remaining_attempts': context['profile']['retry_budget']['max_attempts'] - 1,
                               'remaining_elapsed_ms': context['profile']['retry_budget']['max_elapsed_ms']}}
        self.operations[operation_id] = {'request': canonical(request), 'translated': translated, 'status': status}
        return validate('OperationStatus', deepcopy(status))

    def unknown(self, operation_id):
        record = self.operations[operation_id]
        if record['status']['state'] not in ('pending', 'unknown'):
            raise Refused('terminal_operation')
        record['status'].update(state='unknown', effect='possible')
        record['status']['recovery']['action'] = 'query_or_replay_identical'
        return validate('OperationStatus', deepcopy(record['status']))

    def confirm(self, operation_id, receipt):
        record = self.operations[operation_id]
        status = record['status']
        assert_receipt(strict_loads(record['request']), receipt)
        if status['state'] == 'confirmed':
            if status['receipt'] != receipt:
                raise Refused('id_conflict')
            return deepcopy(status)
        if status['state'] not in ('pending', 'unknown'):
            raise Refused('terminal_operation')
        status.update(state='confirmed', effect='confirmed', receipt=deepcopy(receipt))
        status.pop('recovery', None)
        validate('OperationStatus', status)
        self.confirmed_events.append(operation_id)
        return deepcopy(status)

    def conflict(self, operation_id, item):
        record = self.operations[operation_id]
        if record['status']['state'] not in ('pending', 'unknown'):
            raise Refused('terminal_operation')
        request = strict_loads(record['request'])
        subjects = set()
        for command in request['commands']:
            subjects.update(command[k] for k in ('id', 'source', 'target', 'parent_id') if k in command)
            subjects.update(g['id'] for g in command.get('guards', []))
        if item['subject_id'] is not None and item['subject_id'] not in subjects:
            raise Refused('receipt_scope_denied')
        record['status'].update(state='conflict', effect='not_applied', conflicts=[item], retryable=False)
        record['status'].pop('recovery', None)
        return validate('OperationStatus', deepcopy(record['status']))

    def recover(self, principal, request):
        validate('RecoveryRequest', request)
        if principal['kind'] != 'node' or principal['node_id'] != request['node_id']:
            raise Refused('unauthorized')
        def owned(record):
            binding = record['status']['binding']
            return binding['node_id'] == request['node_id'] and all(binding[k] == request['attempt'][k] for k in request['attempt'])
        if request['mode'] == 'lookup':
            records = [self.operations.get(op) for op in request['operation_ids']]
            if any(record is None or not owned(record) for record in records):
                raise Refused('operation_unknown')
        else:
            records = [r for r in self.operations.values() if owned(r)]
            # The fixture has no cursor encoder. Production requires an opaque
            # cursor bound to the node+attempt+query; it may not be a raw offset.
            if request['cursor'] is not None:
                raise Refused('cursor_invalid')
            records = records[:request['limit']]
        return [deepcopy(r['status']) for r in records]


def query_digest(request):
    query = {k: v for k, v in request['query'].items() if k != 'cursor'}
    return digest({'binding': request['binding'], 'query': query})


def assert_read_response(request, response, context, graph):
    """Rejects leakage in a fixture response; production must filter BEFORE paging."""
    validate('ReadResponse', response, context['profile']['max_response_bytes'])
    binding_check(response['binding'], context['binding'])
    if response['request_id'] != request['request_id'] or response['query_digest'] != query_digest(request):
        raise Refused('response_binding_mismatch')
    result = response['result']
    query = request['query']
    if result['kind'] != query['kind']:
        raise Refused('response_binding_mismatch')

    def inspect(value):
        if isinstance(value, dict):
            if {'id', 'kind', 'content_revision', 'title', 'body'} <= value.keys():
                require_read(value['id'], context, graph)
            if {'source', 'target', 'edge_type', 'version'} <= value.keys():
                if not visible_edge({**value, 'workspace_id': context['binding']['workspace_id']}, context, graph):
                    raise Refused('scope_denied')
            if 'subject_id' in value:
                require_read(value['subject_id'], context, graph)
                if value['subject_id'] != query.get('node_id'):
                    raise Refused('response_binding_mismatch')
            for child in value.values():
                inspect(child)
        elif isinstance(value, list):
            for child in value:
                inspect(child)
    inspect(result)
    if query['kind'] in ('get', 'revision', 'context') and result['node']['id'] != query['node_id']:
        raise Refused('response_binding_mismatch')
    if query['kind'] == 'revision' and result['node']['content_revision'] != query['revision']:
        raise Refused('response_binding_mismatch')
    if query['kind'] == 'context':
        if len(result['nodes']) + 1 > query['max_nodes'] or len(result['edges']) > query['max_edges']:
            raise Refused('response_too_large')
        returned = {result['node']['id'], *(n['id'] for n in result['nodes'])}
        if any(e['source'] not in returned or e['target'] not in returned for e in result['edges']):
            raise Refused('scope_denied')
        if len(canonical(response)) > query['max_bytes']:
            raise Refused('response_too_large')
    if query['kind'] == 'search':
        if len(result['nodes']) > query['limit'] or any(query['kind_filter'] is not None and n['kind'] != query['kind_filter'] for n in result['nodes']):
            raise Refused('response_binding_mismatch')
    if query['kind'] in ('relationships', 'history'):
        values = result['edges'] if query['kind'] == 'relationships' else result['items']
        if len(values) > query['limit']:
            raise Refused('response_too_large')
        if query['kind'] == 'relationships':
            for edge in values:
                if edge['edge_type'] not in query['edge_types'] or query['node_id'] not in (edge['source'], edge['target']):
                    raise Refused('response_binding_mismatch')
                if query['direction'] == 'incoming' and edge['target'] != query['node_id']:
                    raise Refused('response_binding_mismatch')
                if query['direction'] == 'outgoing' and edge['source'] != query['node_id']:
                    raise Refused('response_binding_mismatch')
    return response


class GraphServiceFixture:
    """Receipt cache to inject commit-before-response-loss, not a graph engine."""
    def __init__(self, retained_receipt):
        self.retained_receipt = retained_receipt
        self.receipts = {}
        self.logical_commits = 0

    def dispatch(self, intent, lose_response=False, version_conflict=False):
        key = (intent['workspace_id'], intent['service_principal'], intent['operation_class'], intent['payload']['idempotency_key'])
        body = canonical(intent['payload'])
        if key in self.receipts:
            old_body, receipt = self.receipts[key]
            if old_body != body:
                raise Refused('id_conflict')
            return deepcopy(receipt)
        if version_conflict:
            raise Refused('version_conflict')
        self.logical_commits += 1
        self.receipts[key] = (body, deepcopy(self.retained_receipt))
        if lose_response:
            raise ConnectionError('fixture response lost AFTER graph commit')
        return deepcopy(self.retained_receipt)


def assert_receipt(request, receipt):
    validate('GraphReceipt', receipt)
    if receipt['workspace_id'] != request['binding']['workspace_id']:
        raise Refused('response_binding_mismatch')
    proposal = request['commands'][0]['kind'] == 'propose_decomposition'
    if proposal != (receipt['kind'] == 'graph_draft'):
        raise Refused('response_binding_mismatch')
    if proposal:
        return
    outputs, affected = set(), set()
    for command in request['commands']:
        kind = command['kind']
        outputs.add(('node' if kind in ('create_finding', 'correct_record') else 'edge', command['id']))
        affected.add(command.get('source', command['id']))
    if {(v['kind'], v['id']) for v in receipt['versions']} != outputs or set(receipt['affected_record_ids']) != affected:
        raise Refused('receipt_scope_denied')


def retry_action(status, attempts_used, elapsed_ms, profile, session_active):
    """Client decision vector only. A real client must persist counters/bytes."""
    validate('OperationStatus', status)
    if status['state'] == 'confirmed':
        return 'complete'
    if status['state'] in ('conflict', 'rejected'):
        return 'stop_and_review'
    budget = profile['retry_budget']
    if attempts_used >= budget['max_attempts'] or elapsed_ms >= budget['max_elapsed_ms']:
        return 'manual_reconciliation'
    if status['recovery']['action'] == 'manual_reconciliation':
        return 'manual_reconciliation'
    if not session_active:
        return 'node_lookup_existing_only'
    return 'poll_existing' if status['state'] == 'pending' else 'query_then_replay_identical'
