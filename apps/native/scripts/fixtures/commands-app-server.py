#!/usr/bin/env python3
"""Synthetic Codex app-server for the slash-command tests: it answers every request and writes each one it was sent to
`commands-requests.jsonl` in its working folder, for a test to read. No Codex, credentials or network."""
import json, os, sys

# A test may choose what the turn says and how it ends: `commands-reply.json` in the working folder, {"text": ...,
# "status": "completed" or "failed", "deltas": [pieces streamed before the whole text], "config": what config/read
# answers}. Without it the turn answers "Done." and completes, and the person's Codex has no settings.
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
    reply, status = chosen_reply()
    for piece in chosen().get('deltas', []):
        notify('item/agentMessage/delta', itemId='answer', delta=piece)
    notify('item/agentMessage/delta', itemId='answer', operation='replace', delta=reply)
    notify('item/completed', item=dict(id='answer', type='agentMessage', text=reply))
    turn = {'id': 'turn_fake', 'status': status}
    if status == 'failed':
        turn['error'] = {'message': 'Synthetic failure.'}
    notify('turn/completed', turn=turn)

for line in sys.stdin:
    msg = json.loads(line)
    method, p = msg.get('method'), msg.get('params', {})
    if method not in (None, 'initialized'):
        with open('commands-requests.jsonl', 'a') as log:
            log.write(json.dumps({'method': method, 'params': p}) + '\n')
    if method == 'initialize':
        send(dict(id=msg['id'], result={}))
    elif method == 'config/read':
        send(dict(id=msg['id'], result={'config': chosen().get('config', {})}))
    elif method in ('thread/start', 'thread/resume'):
        send(dict(id=msg['id'], result={'thread': {'id': 'thread_fake'}}))
    elif method in ('turn/start', 'review/start'):
        done(msg)
    elif method == 'thread/compact/start':
        send(dict(id=msg['id'], result={}))
        notify('turn/started', turn={'id': 'turn_compact'})
        notify('turn/completed', turn={'id': 'turn_compact', 'status': 'completed'})
