#!/usr/bin/env python3
"""A Claude that says something, asks to write a file, and says more after the answer (approval order in a streamed run)."""
import json, os, socket, sys, time

def send(value):
    print(json.dumps(value), flush=True)

def said(text):
    send({'type': 'stream_event', 'event': {'type': 'content_block_delta', 'index': 0, 'delta': {'type': 'text_delta', 'text': text}}})

config = json.load(open(sys.argv[sys.argv.index('--mcp-config') + 1]))
host = config['mcpServers']['locust']['args'][1]
assert json.loads(sys.stdin.readline())['type'] == 'user'
send({'type': 'system', 'subtype': 'init', 'session_id': 'ordered'})
said('Before the write.')
with socket.socket(socket.AF_UNIX) as s:
    s.connect(host)
    s.sendall(json.dumps({'tool_name': 'Write', 'tool_use_id': 'write-1', 'input': {'file_path': os.path.join(os.getcwd(), 'hello.txt'), 'content': 'hello\n'}}).encode() + b'\n')
    answer = json.loads(s.makefile('r').readline())
if answer['behavior'] == 'allow':
    open('hello.txt', 'w').write('hello\n')
# A real agent takes a moment after the answer; within one text batch (50 ms) the order is not promised.
time.sleep(1.5)
said(' After the write.')
send({'type': 'result', 'subtype': 'success', 'result': 'Before the write. After the write.'})
sys.stdin.read()
