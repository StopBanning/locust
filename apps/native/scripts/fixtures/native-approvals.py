#!/usr/bin/env python3
"""A provider that performs exactly one write after its host grants permission."""
import json, os, socket, sys

def send(value):
    print(json.dumps(value), flush=True)

runtime = sys.argv[1]
if runtime == 'claude':
    config = json.load(open(sys.argv[sys.argv.index('--mcp-config') + 1]))
    host = config['mcpServers']['locust']['args'][1]
    assert json.loads(sys.stdin.readline())['type'] == 'user'
    with socket.socket(socket.AF_UNIX) as s:
        s.connect(host)
        s.sendall(json.dumps({'tool_name': 'Write', 'tool_use_id': 'write-1', 'input': {'file_path': os.path.join(os.getcwd(), 'hello.txt'), 'content': 'hello\n'}}).encode() + b'\n')
        answer = json.loads(s.makefile('r').readline())
    if answer['behavior'] == 'allow':
        open('hello.txt', 'w').write('hello\n')
    send({'type': 'result', 'subtype': 'success', 'result': 'DONE'})
    sys.stdin.read()
else:
    for line in sys.stdin:
        msg = json.loads(line)
        method = msg.get('method')
        if method == 'initialize':
            send({'id': msg['id'], 'result': {}})
        elif method == 'config/read':
            send({'id': msg['id'], 'result': {'config': {}}})
        elif method == 'thread/start':
            assert msg['params']['approvalPolicy'] == 'untrusted'
            assert msg['params']['sandbox'] == 'workspace-write'
            send({'id': msg['id'], 'result': {'thread': {'id': 'synthetic'}}})
        elif method == 'turn/start':
            send({'id': msg['id'], 'result': {'turn': {'id': 'turn'}}})
            send({'id': 'write', 'method': 'item/commandExecution/requestApproval', 'params': {'threadId': 'synthetic', 'command': 'touch hello.txt', 'cwd': os.getcwd()}})
        elif msg.get('id') == 'write':
            if msg['result']['decision'] == 'accept':
                open('hello.txt', 'w').write('hello\n')
            send({'method': 'turn/completed', 'params': {'threadId': 'synthetic', 'turn': {'id': 'turn', 'status': 'completed'}}})
