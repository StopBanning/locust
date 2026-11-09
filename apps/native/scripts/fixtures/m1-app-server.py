#!/usr/bin/env python3
"""Synthetic JSON-RPC, based on runtime-adapters/test/codex-app-server-run.test.ts fakeServer.
No Codex invocation, config, credentials, or network. Assertions validate the client policy.
"""
import json, os, sys, time
if os.environ.get("M1_PID_FILE"):
    with open(os.environ["M1_PID_FILE"], "w") as f:
        f.write(str(os.getpid()))
scenario = sys.argv[1] if len(sys.argv) > 1 else 'success'
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
    elif method == 'thread/start':
        assert p['sandbox'] == 'read-only' and p['approvalPolicy'] == 'never'
        assert p['config']['mcp_servers.local.enabled'] is False
        assert p['config']['features.plugins'] is False
        assert 'mcp_servers.remote.enabled' not in p['config']
        send(dict(id=msg['id'], result={'thread':{'id':'thread_fake'}}))
    elif method == 'turn/start':
        assert p['summary'] == 'detailed' and p['approvalPolicy'] == 'never'
        assert p['input'] == [{'type':'text','text':'Synthetic prompt'}]
        assert p['model'] == 'synthetic-model'
        send(dict(id=msg['id'], result={'turn':{'id':'turn_fake'}}))
        notify('turn/started', turn={'id':'turn_fake'})
        send(dict(method='turn/completed', params={'threadId':'other_thread'}))
        if scenario == 'lost':
            sys.exit(0)
        if scenario == 'failure':
            notify('error', error={'message':'Usage limit reached', 'codexErrorInfo':'usageLimitExceeded'})
            continue
        if scenario == 'flood':
            time.sleep(0.3)  # Allow the CLI to record run.started and receive SIGINT.
            while True:  # Never read stdin again, including interrupt/approval replies.
                notify('item/agentMessage/delta', itemId='answer', delta='x' * 4096)
        if scenario == 'interrupt':
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
        send(dict(id='approval', method='item/commandExecution/requestApproval',params={'command':'touch forbidden'}))
    elif msg.get('id') == 'approval':
        assert msg['result']['decision'] == 'decline'
        notify('warning', message='Synthetic warning')
        notify('thread/tokenUsage/updated', tokenUsage={'total':{'inputTokens':12,'outputTokens':4},'last':{'inputTokens':12,'outputTokens':4}})
        notify('turn/completed', turn={'id':'turn_fake','status':'completed'})
    elif method == 'turn/interrupt':
        assert p == {'threadId':'thread_fake','turnId':'turn_fake'}
        send(dict(id=msg['id'],result={}))
        notify('turn/completed', turn={'id':'turn_fake','status':'interrupted'})
    else:
        raise AssertionError(msg)
