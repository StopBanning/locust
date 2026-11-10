#!/usr/bin/env python3
"""Synthetic JSON-RPC, based on runtime-adapters/test/codex-app-server-run.test.ts fakeServer.
No Codex invocation, config, credentials, or network. Assertions validate the client policy.
"""
import json, os, sys, time
if os.environ.get("M1_PID_FILE"):
    with open(os.environ["M1_PID_FILE"], "w") as f:
        f.write(str(os.getpid()))
scenario = sys.argv[1] if len(sys.argv) > 1 else 'success'
# M2: `resume` / `resume-interrupt` / `resume-failure` / `resume-refused` run the follow-up path (thread/resume, never
# thread/start) with the follow-up prompt; the rest are the M1 scenarios.
# M3b: `approve-allow` / `approve-deny` use Approve-each policy (untrusted + workspace-write).
resuming = scenario.startswith('resume')
approving = scenario.startswith('approve-')
base = scenario[len('resume-'):] if scenario.startswith('resume-') else ('success' if scenario == 'resume' else scenario)
if approving:
    base = 'success'
SANDBOX = 'workspace-write' if approving else 'read-only'
POLICY = 'untrusted' if approving else 'never'
APPROVE_DECISION = 'accept' if scenario == 'approve-allow' else ('reject' if scenario == 'approve-deny' else 'decline')
PROMPT = 'Synthetic follow-up' if resuming else 'Synthetic prompt'
def send(v):
    print(json.dumps(dict(jsonrpc='2.0', **v)), flush=True)
def notify(method, **params):
    send(dict(method=method, params=dict(threadId='thread_fake', **params)))
def item(done, **fields):
    notify('item/completed' if done else 'item/started', item=fields)
for line in sys.stdin:
    msg = json.loads(line)
    method, p = msg.get('method'), msg.get('params', {})
    if method == 'initialize':
        assert p['clientInfo']['name'] == 'locust'
        send(dict(id=msg['id'], result={}))
    elif method == 'initialized':
        pass
    elif method == 'config/read':
        send(dict(id=msg['id'], result={'config': {'mcp_servers': {'local': {'command':'node'}, 'remote': {'url':'https://example.invalid'}}}}))
    elif method == 'thread/resume':
        assert resuming, 'thread/resume outside a resume scenario'
        # codex-app-server-run.ts: reopen by the stored id, without the whole history, under the same policy.
        assert p['threadId'] == 'thread_fake' and p['excludeTurns'] is True, p
        assert p['sandbox'] == SANDBOX and p['approvalPolicy'] == POLICY
        assert p['config']['mcp_servers.local.enabled'] is False and p['config']['features.plugins'] is False
        assert p['model'] == 'synthetic-model'
        if base == 'refused':
            send(dict(id=msg['id'], error={'code': -32600, 'message': 'no rollout found for thread id thread_fake'}))
        else:
            send(dict(id=msg['id'], result={'thread':{'id':'thread_fake'}}))
    elif method == 'thread/start':
        assert not resuming, 'a follow-up must resume, not start a new thread'
        assert p['sandbox'] == SANDBOX and p['approvalPolicy'] == POLICY, p
        assert p['config']['mcp_servers.local.enabled'] is False
        assert p['config']['features.plugins'] is False
        assert 'mcp_servers.remote.enabled' not in p['config']
        send(dict(id=msg['id'], result={'thread':{'id':'thread_fake'}}))
    elif method == 'turn/start':
        assert p['summary'] == 'detailed' and p['approvalPolicy'] == POLICY, p
        assert p['input'] == [{'type':'text','text':PROMPT}]
        assert p['model'] == 'synthetic-model'
        send(dict(id=msg['id'], result={'turn':{'id':'turn_fake'}}))
        notify('turn/started', turn={'id':'turn_fake'})
        send(dict(method='turn/completed', params={'threadId':'other_thread'}))
        if base == 'lost':
            sys.exit(0)
        if base == 'failure':
            notify('error', error={'message':'Usage limit reached', 'codexErrorInfo':'usageLimitExceeded'})
            continue
        if base == 'flood':
            time.sleep(0.3)  # Allow the CLI to record run.started and receive SIGINT.
            while True:  # Never read stdin again, including interrupt/approval replies.
                notify('item/agentMessage/delta', itemId='answer', delta='x' * 4096)
        if base == 'interrupt':
            continue
        notify('item/agentMessage/delta', itemId='answer', delta='Hello ')
        time.sleep(0.4)  # CLI test must observe this delta before the turn can end.
        notify('item/agentMessage/delta', itemId='answer', delta='world')
        item(False, id='thinking', type='reasoning', content=['RAW_REASONING_MUST_NOT_SURVIVE'])
        item(True, id='thinking', type='reasoning', summary=['Checking the fixture'], content=['RAW_REASONING_MUST_NOT_SURVIVE'])
        notify('turn/plan/updated', turnId='turn_fake', plan=[{'step':'Check fixture','status':'completed'}])
        for kind, fields in [('commandExecution', {'command':'echo hello','aggregatedOutput':'hello','exitCode':0}), ('webSearch', {'query':'fixture'}), ('fileChange', {}), ('mcpToolCall', {'server':'docs','tool':'read'})]:
            item(False, id=kind, type=kind, **fields)
            item(True, id=kind, type=kind, status='completed', **fields)
        item(True, id='answer', type='agentMessage', text='Hello world')
        send(dict(id='approval', method='item/commandExecution/requestApproval',params={'command':'touch forbidden','cwd':'/tmp'}))
    elif msg.get('id') == 'approval':
        assert msg['result']['decision'] == APPROVE_DECISION, msg
        notify('warning', message='Synthetic warning')
        notify('thread/tokenUsage/updated', tokenUsage={'total':{'inputTokens':12,'outputTokens':4},'last':{'inputTokens':12,'outputTokens':4}})
        notify('turn/completed', turn={'id':'turn_fake','status':'completed'})
    elif method == 'turn/interrupt':
        assert p == {'threadId':'thread_fake','turnId':'turn_fake'}
        send(dict(id=msg['id'],result={}))
        notify('turn/completed', turn={'id':'turn_fake','status':'interrupted'})
    else:
        raise AssertionError(msg)
