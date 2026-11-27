#!/usr/bin/env python3
"""Synthetic Codex app-server for the slash-command tests: it answers every request and writes each one it was sent to
`commands-requests.jsonl` beside its working folder, for a test to read. No Codex, credentials or network."""
import json, os, sys

# A test may choose what the turn says and how it ends: `commands-reply.json` in the working folder, {"text": ...,
# "status": "completed" or "failed", "deltas": [pieces streamed before the whole text], "config": what config/read
# answers, "goal": what thread/goal/get answers, "goal_notices": goal notifications the turn says before it ends, "ask": {"method": ..., "params": ...}, a request the turn sends as `srv-1` and waits on before it ends}.
# Without it the turn answers "Done." and completes, and the person's Codex has no settings. The answer to `srv-1` is
# written to `commands-replies.jsonl`.
def chosen():
    if os.path.exists('commands-reply.json'):
        with open('commands-reply.json') as f:
            return json.load(f)
    return {}

def chosen_reply():
    c = chosen()
    return c.get('text', 'Done.'), c.get('status', 'completed')

def send(v):
    print(json.dumps(dict(jsonrpc='2.0', **v)), flush=True)

def notify(method, **params):
    send(dict(method=method, params=dict(threadId='thread_fake', **params)))

def done(msg):
    send(dict(id=msg['id'], result={'turn': {'id': 'turn_fake'}}))
    notify('turn/started', turn={'id': 'turn_fake'})
    ask = chosen().get('ask')
    if ask:
        send(dict(id='srv-1', method=ask['method'], params=dict(threadId='thread_fake', turnId='turn_fake', **ask['params'])))
    else:
        end()

def end():
    reply, status = chosen_reply()
    # `goal_notices`: goal notifications ({"method", "params"}) the turn says before it ends.
    for said in chosen().get('goal_notices', []):
        notify(said['method'], **said['params'])
    for piece in chosen().get('deltas', []):
        notify('item/agentMessage/delta', itemId='answer', delta=piece)
    notify('item/agentMessage/delta', itemId='answer', operation='replace', delta=reply)
    notify('item/completed', item=dict(id='answer', type='agentMessage', text=reply))
    turn = {'id': 'turn_fake', 'status': status}
    if status == 'failed':
        turn['error'] = {'message': 'Synthetic failure.'}
    notify('turn/completed', turn=turn)

# The logs go one folder up, beside the working folder and not in it: a write-capable turn's disk observation reports
# every file that changed in the working folder, and these are the stand-in's own notes, not the agent's work.
LOG = lambda name: os.path.join(os.path.dirname(os.getcwd()), name)

for line in sys.stdin:
    msg = json.loads(line)
    method, p = msg.get('method'), msg.get('params', {})
    if method not in (None, 'initialized'):
        with open(LOG('commands-requests.jsonl'), 'a') as log:
            log.write(json.dumps({'method': method, 'params': p}) + '\n')
    if method is None and msg.get('id') == 'srv-1':
        with open(LOG('commands-replies.jsonl'), 'a') as log:
            log.write(json.dumps(msg) + '\n')
        end()
    elif method == 'initialize':
        send(dict(id=msg['id'], result={}))
    elif method == 'config/read':
        send(dict(id=msg['id'], result={'config': chosen().get('config', {})}))
    elif method in ('thread/start', 'thread/resume'):
        send(dict(id=msg['id'], result={'thread': {'id': 'thread_fake'}}))
    elif method in ('turn/start', 'review/start'):
        done(msg)
    elif method == 'thread/goal/get':
        # `goal`: the standing goal the thread holds; none without it.
        send(dict(id=msg['id'], result={'goal': chosen().get('goal')}))
    elif method in ('thread/goal/set', 'thread/goal/clear'):
        send(dict(id=msg['id'], result={}))
    elif method == 'thread/compact/start':
        send(dict(id=msg['id'], result={}))
        notify('turn/started', turn={'id': 'turn_compact'})
        notify('turn/completed', turn={'id': 'turn_compact', 'status': 'completed'})
    elif method == 'turn/interrupt':
        send(dict(id=msg['id'], result={}))
        notify('turn/completed', turn={'id': 'turn_fake', 'status': 'interrupted'})
