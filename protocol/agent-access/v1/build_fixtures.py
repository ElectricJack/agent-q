#!/usr/bin/env python3
"""Derive offline wire examples from retained qualified graph HTTP responses."""
from copy import deepcopy
import json
from pathlib import Path
import uuid

from conformance_model import project_context, query_digest, scoped_edge, scoped_node, translate_fixture
from validate import digest, validate

ROOT = Path(__file__).resolve().parent


def uid(n):
    return str(uuid.UUID(int=n))


def build():
    graph = json.loads((ROOT / 'fixtures/qualified-graph.json').read_text())
    profile = {'profile_id': 'coding-pilot', 'revision': '1', 'role': 'coding',
               'model': {'provider': 'fixture-provider', 'model': 'deterministic', 'configuration_digest': '1'*64},
               'instructions_digest': '2'*64, 'tool_policy_digest': '3'*64,
               'capabilities': ['read.get', 'read.revision', 'read.history', 'read.search', 'read.context', 'read.relationships',
                                'write.finding', 'write.correct', 'write.proposal', 'write.relationships'],
               'read_scope': {'mode': 'records', 'record_ids': [uid(1), uid(2), uid(3)], 'include_created_by_attempt': True,
                              'edge_types': ['blocks', 'parent-child', 'references', 'motivated_by', 'produces', 'supports', 'contradicts', 'supersedes']},
               'write_scope': {'record_ids': [uid(1), uid(2), uid(3)], 'relationship_types': ['references', 'motivated_by', 'produces', 'supports', 'contradicts', 'supersedes'],
                               'correct_fields': ['title', 'body', 'labels'], 'max_created_records': 32, 'max_proposal_tasks': 8},
               'retry_budget': {'max_attempts': 5, 'max_elapsed_ms': 30000, 'initial_delay_ms': 250, 'max_delay_ms': 2000},
               'max_response_bytes': 1048576}
    binding = {'task_id': uid(1), 'attempt_id': uid(102), 'session_id': uid(103), 'generation': 1, 'node_id': uid(100),
               'boot_id': uid(101), 'project_id': uid(104), 'workspace_id': uid(105), 'resolved_spec_digest': '4'*64,
               'profile_digest': digest(profile), 'context_digest': '0'*64}
    context = {'api_version': 1, 'binding': binding, 'profile': profile,
               'credential': {'kind': 'agent_session', 'version': '1', 'authority_revision': '1', 'expires_at': '2099-01-01T00:00:00Z'}}
    queries = {'get': {'kind': 'get', 'node_id': uid(2)},
               'revision': {'kind': 'revision', 'node_id': uid(1), 'revision': '1'},
               'history': {'kind': 'history', 'node_id': uid(1), 'limit': 64, 'cursor': None},
               'search': {'kind': 'search', 'text': 'receipts', 'kind_filter': 'knowledge', 'limit': 64, 'cursor': None},
               'context': {'kind': 'context', 'node_id': uid(1), 'depth': 1, 'direction': 'both', 'max_nodes': 64, 'max_edges': 256, 'max_bytes': 262144},
               'relationships': {'kind': 'relationships', 'node_id': uid(1), 'direction': 'both', 'edge_types': profile['read_scope']['edge_types'], 'limit': 64, 'cursor': None}}
    temporary = {'api_version': 1, 'request_id': uid(200), 'binding': binding, 'query': queries['context']}
    projected = project_context(graph['context'], temporary, context, graph)
    context['context'] = {'observed_at': graph['origin']['captured_at'], 'projection': 'authorized_scope', 'root_ids': [uid(1)],
                          'nodes': [projected['node'], *projected['nodes']], 'edges': projected['edges'],
                          'pinned_revisions': projected['pinned_revisions'], 'coverage': projected['coverage']}
    binding['context_digest'] = digest(context['context'])
    validate('ContextResponse', context)
    agent = {'kind': 'agent_session', 'binding': deepcopy(binding), 'active': True, 'graph_actor_id': 'api01-writer'}
    node_principal = {'kind': 'node', 'node_id': uid(100)}
    coverage = {'status': 'complete', 'complete_for': 'authorized_projection', 'reasons': []}
    revision = graph['revision']
    visible_links = [scoped_edge(e) for e in (revision['links'] or []) if e['source'] in profile['read_scope']['record_ids'] and e['target'] in profile['read_scope']['record_ids']]
    history = next(x for x in graph['history']['items'] if x['category'] == 'lifecycle' and x['operation'] == 'created')
    results = {'get': {'kind': 'get', 'node': scoped_node(graph['nodes'][uid(2)])},
               'revision': {'kind': 'revision', 'node': scoped_node(revision['node']), 'links': visible_links if revision['relationships_available'] else None,
                            'relationships_available': revision['relationships_available'], 'coverage': coverage},
               'history': {'kind': 'history', 'items': [{'entry_id': history['entry_id'], 'sequence': str(history['sequence']), 'category': history['category'],
                              'subject_id': uid(1), 'occurred_at': history['created_at'], 'node': None, 'edge': None}], 'next_cursor': None, 'coverage': coverage},
               'search': {'kind': 'search', 'nodes': [scoped_node(graph['nodes'][uid(2)])], 'next_cursor': None, 'coverage': coverage},
               'context': projected,
               'relationships': {'kind': 'relationships', 'edges': projected['edges'], 'next_cursor': None, 'coverage': coverage}}
    requests, responses = {}, {}
    for n, (kind, query) in enumerate(queries.items()):
        request = {'api_version': 1, 'request_id': uid(210+n), 'binding': deepcopy(binding), 'query': query}
        response = {'api_version': 1, 'request_id': request['request_id'], 'binding': deepcopy(binding), 'query_digest': query_digest(request),
                    'observed_at': graph['origin']['captured_at'], 'projection': 'authorized_scope', 'result': results[kind]}
        validate('ReadRequest', request); validate('ReadResponse', response)
        requests[kind], responses[kind] = request, response
    commands = {'create_finding': {'kind': 'create_finding', 'id': uid(10), 'title': 'A scoped finding', 'body': 'Synthetic evidence; no secret or automatic approval.', 'labels': ['conformance'], 'content_type': 'note'},
                'correct_record': {'kind': 'correct_record', 'id': uid(2), 'expected_version': str(graph['nodes'][uid(2)]['version']),
                                   'patch': {'body': graph['nodes'][uid(2)]['body']}},
                'add_relationship': {'kind': 'add_relationship', 'id': uid(29), 'source': uid(2), 'target': uid(3), 'expected_source_version': str(graph['nodes'][uid(2)]['version']),
                                     'edge_type': 'references', 'source_revision': None, 'target_revision': '1', 'description': 'Pinned authorized evidence'},
                'remove_relationship': {'kind': 'remove_relationship', 'id': uid(21), 'source': uid(2), 'target': uid(1), 'expected_version': '1',
                                        'expected_source_version': str(graph['nodes'][uid(2)]['version'])},
                'propose_decomposition': {'kind': 'propose_decomposition', 'name': 'Proposed follow-up work', 'parent_id': uid(1),
                    'expected_parent_version': str(graph['nodes'][uid(1)]['version']),
                    'tasks': [{'id': uid(11), 'title': 'Review scoped writes', 'body': 'Draft only.', 'acceptance_criteria': 'No stale overwrite.'}],
                    'relationships': [{'id': uid(31), 'source': uid(1), 'target': uid(11), 'edge_type': 'parent-child'},
                                      {'id': uid(32), 'source': uid(11), 'target': uid(3), 'edge_type': 'blocks'}],
                    'guards': [{'id': uid(3), 'expected_version': str(graph['nodes'][uid(3)]['version'])}]}}
    command_requests = {kind: {'api_version': 1, 'operation_id': uid(300+n), 'binding': deepcopy(binding), 'reason': 'Synthetic contract conformance', 'commands': [command]}
                        for n, (kind, command) in enumerate(commands.items())}
    request = command_requests['correct_record']
    common = {'api_version': 1, 'operation_id': request['operation_id'], 'binding': deepcopy(binding), 'payload_digest': digest(request)}
    hint = {'action': 'poll_existing', 'after_ms': 250, 'remaining_attempts': 4, 'remaining_elapsed_ms': 30000}
    receipt = {'kind': 'graph_mutation', 'workspace_id': uid(105), 'change_set_id': uid(400),
               'versions': [{'kind': 'node', 'id': uid(2), 'version': str(graph['nodes'][uid(2)]['version']), 'content_revision': str(graph['nodes'][uid(2)]['content_revision'])}],
               'affected_record_ids': [uid(2)], 'task_completion': False}
    statuses = {'pending': {**common, 'state': 'pending', 'admission': 'recorded', 'effect': 'not_dispatched', 'recovery': hint},
                'confirmed': {**common, 'state': 'confirmed', 'admission': 'recorded', 'effect': 'confirmed', 'receipt': receipt},
                'unknown': {**common, 'state': 'unknown', 'admission': 'recorded', 'effect': 'possible', 'recovery': {**hint, 'action': 'query_or_replay_identical'}},
                'conflict': {**common, 'state': 'conflict', 'admission': 'recorded', 'effect': 'not_applied', 'retryable': False,
                             'conflicts': [{'kind': 'version', 'subject_id': uid(2), 'expected': '2', 'current': '3'}]},
                'rejected': {**common, 'state': 'rejected', 'admission': 'recorded', 'effect': 'not_applied', 'retryable': False, 'code': 'graph_rejected'}}
    recovery = {'api_version': 1, 'request_id': uid(500), 'node_id': uid(100),
                'attempt': {k: binding[k] for k in ['attempt_id', 'session_id', 'generation']}, 'mode': 'lookup', 'operation_ids': [request['operation_id']]}
    listing = {k: v for k, v in recovery.items() if k not in ['operation_ids', 'mode']}
    listing.update(mode='list', limit=64, cursor=None)
    recovery_response = {'api_version': 1, 'request_id': uid(500), 'attempt': recovery['attempt'], 'operations': [statuses['unknown']],
                         'next_cursor': None, 'coverage': coverage}
    messages = [{'name': 'immutable-launch-context', 'type': 'ContextResponse', 'value': context, 'valid': True}]
    messages += [{'name': 'read-'+k, 'type': 'ReadRequest', 'value': v, 'valid': True} for k, v in requests.items()]
    messages += [{'name': 'read-result-'+k, 'type': 'ReadResponse', 'value': v, 'valid': True} for k, v in responses.items()]
    messages += [{'name': 'command-'+k, 'type': 'CommandRequest', 'value': v, 'valid': True} for k, v in command_requests.items()]
    messages += [{'name': 'operation-'+k, 'type': 'OperationStatus', 'value': v, 'valid': True} for k, v in statuses.items()]
    messages += [{'name': 'own-node-operation-lookup', 'type': 'RecoveryRequest', 'value': recovery, 'valid': True},
                 {'name': 'own-node-operation-list', 'type': 'RecoveryRequest', 'value': listing, 'valid': True},
                 {'name': 'own-node-recovery-result', 'type': 'RecoveryResponse', 'value': recovery_response, 'valid': True}]
    for field in ['task_state', 'verification', 'retired', 'metadata', 'created_by', 'scope', 'credential']:
        bad = deepcopy(command_requests['correct_record']);bad['commands'][0]['patch'][field] = 'forbidden'
        messages.append({'name': 'reject-protected-field-'+field, 'type': 'CommandRequest', 'value': bad, 'valid': False})
    for kind in ['complete_task', 'cancel_task', 'approve_evidence', 'work.ensure', 'candidate.publish', 'request_url']:
        bad = deepcopy(command_requests['correct_record']);bad['commands'][0] = {'kind': kind}
        messages.append({'name': 'reject-foreign-command-'+kind, 'type': 'CommandRequest', 'value': bad, 'valid': False})
    for k, v in command_requests.items():
        validate('CommandRequest', v);translate_fixture(v, context)
    for v in statuses.values():validate('OperationStatus', v)
    base = {'context': context, 'agent_principal': agent, 'node_principal': node_principal,
            'read_requests': requests, 'read_responses': responses, 'command_requests': command_requests,
            'operation_statuses': statuses, 'node_recovery_request': recovery, 'node_list_request': listing}
    scope_cases = []
    def case(name, action, request, expected='allowed', principal_kind='agent_session', active=True, response=None):
        value = {'name': name, 'action': action, 'principal_kind': principal_kind, 'active': active,
                 'request': deepcopy(request), 'expected': expected}
        if response is not None:
            value['response'] = deepcopy(response)
        scope_cases.append(value)
    for kind, request in requests.items():
        case('authorized-read-' + kind, 'read', request)
        if 'node_id' in request['query']:
            hidden = deepcopy(request);hidden['query']['node_id'] = uid(4)
            case('hidden-root-' + kind, 'read', hidden, 'not_found')
    for kind, request in command_requests.items():
        case('authorized-command-' + kind, 'command', request, 'scope_denied' if kind == 'propose_decomposition' else 'allowed')
    for principal_kind in ['node', 'graph_service']:
        case('wrong-credential-read-' + principal_kind, 'read', requests['get'], 'unauthorized', principal_kind)
        case('wrong-credential-write-' + principal_kind, 'command', command_requests['correct_record'], 'unauthorized', principal_kind)
    case('expired-agent-no-new-write', 'command', command_requests['correct_record'], 'session_expired_or_revoked', active=False)
    for field in ['source', 'target']:
        bad = deepcopy(command_requests['add_relationship']);bad['commands'][0][field] = uid(4)
        case('hidden-write-endpoint-' + field, 'command', bad, 'not_found')
    bad = deepcopy(command_requests['create_finding'])
    extra = deepcopy(command_requests['add_relationship']['commands'][0]);extra['target'] = uid(4);bad['commands'].append(extra)
    case('second-command-fails-whole-batch', 'command', bad, 'not_found')
    for edge_type in profile['read_scope']['edge_types']:
        bad = deepcopy(responses['relationships'])
        edge = deepcopy(next(iter(graph['edges'].values())));edge.update(source=uid(1), target=uid(4), edge_type=edge_type)
        bad['result']['edges'] = [scoped_edge(edge)]
        case('hidden-response-edge-' + edge_type, 'response', requests['relationships'], 'scope_denied', response=bad)
    return {'base.json': base, 'messages.json': messages, 'scope-cases.json': scope_cases}


def main():
    for name, value in build().items():
        (ROOT / 'fixtures' / name).write_text(json.dumps(value, indent=2) + '\n')
    print('Built offline fixture messages from retained qualified graph records.')


if __name__ == '__main__':
    main()
