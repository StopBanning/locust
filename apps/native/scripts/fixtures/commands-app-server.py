#!/usr/bin/env python3
"""Synthetic Codex app-server for the slash-command tests: it answers every request and writes each one it was sent to
`commands-requests.jsonl` in its working folder, for a test to read. No Codex, credentials or network."""
import json, sys

def send(v):
    print(json.dumps(dict(jsonrpc='2.0', **v)), flush=True)

def notify(method, **params):
    send(dict(method=method, params=dict(threadId='thread_fake', **params)))

def done(msg):
    send(dict(id=msg['id'], result={'turn': {'id': 'turn_fake'}}))
    notify('turn/started', turn={'id': 'turn_fake'})
    reply = 'Done.'
    notify('item/agentMessage/delta', itemId='answer', operation='replace', delta=reply)
    notify('item/completed', item=dict(id='answer', type='agentMessage', text=reply))
    notify('turn/completed', turn={'id': 'turn_fake', 'status': 'completed'})

for line in sys.stdin:
    msg = json.loads(line)
    method, p = msg.get('method'), msg.get('params', {})
    if method not in (None, 'initialized'):
        with open('commands-requests.jsonl', 'a') as log:
            log.write(json.dumps({'method': method, 'params': p}) + '\n')
    if method == 'initialize':
        send(dict(id=msg['id'], result={}))
    elif method == 'config/read':
        send(dict(id=msg['id'], result={'config': {}}))
    elif method in ('thread/start', 'thread/resume'):
        send(dict(id=msg['id'], result={'thread': {'id': 'thread_fake'}}))
    elif method in ('turn/start', 'review/start'):
        done(msg)
    elif method == 'thread/compact/start':
        send(dict(id=msg['id'], result={}))
        notify('turn/started', turn={'id': 'turn_compact'})
        notify('turn/completed', turn={'id': 'turn_compact', 'status': 'completed'})
