"""Scripted OpenCode 1 authenticated loopback server for native synthetic tests."""
import base64, json, os, sys, threading, time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

args = [a for a in sys.argv[1:] if a != 'serve']
scenario = args[0] if args else 'success'
password = os.environ.get('OPENCODE_SERVER_PASSWORD', '')
auth = 'Basic ' + base64.b64encode(f'opencode:{password}'.encode()).decode()
busy_path = os.environ.get('LOCUST_OPENCODE_BUSY_FLAG', '')
if scenario == 'busy' and busy_path and not os.path.exists(busy_path):
    open(busy_path, 'w').close()
    print('database is locked', file=sys.stderr, flush=True)
    sys.exit(1)

state = {
    'session': None,
    'prompted': False,
    'subscribers': [],
    'lock': threading.Lock(),
}

def publish(event):
    raw = f"data: {json.dumps(event)}\n\n"
    with state['lock']:
        dead = []
        for q in state['subscribers']:
            try:
                q.append(raw)
            except Exception:
                dead.append(q)
        for q in dead:
            state['subscribers'].remove(q)

class Handler(BaseHTTPRequestHandler):
    protocol_version = 'HTTP/1.1'
    def log_message(self, *args):
        pass
    def _auth(self):
        return self.headers.get('Authorization') == auth
    def _read(self):
        n = int(self.headers.get('Content-Length', '0'))
        body = self.rfile.read(n) if n else b''
        return json.loads(body) if body else None
    def _json(self, code, body):
        data = b'' if body is None else json.dumps(body).encode()
        self.send_response(code)
        self.send_header('Content-Type', 'application/json')
        self.send_header('Content-Length', str(len(data)))
        self.end_headers()
        if data:
            self.wfile.write(data)
    def do_GET(self):
        if not self._auth():
            return self._json(401, {'error': 'unauthorized'})
        if self.path.startswith('/event'):
            self.send_response(200)
            self.send_header('Content-Type', 'text/event-stream')
            self.send_header('Cache-Control', 'no-cache')
            self.send_header('Connection', 'close')
            self.end_headers()
            q = []
            with state['lock']:
                state['subscribers'].append(q)
            try:
                while True:
                    while q:
                        chunk = q.pop(0)
                        self.wfile.write(chunk.encode())
                        self.wfile.flush()
                    time.sleep(0.02)
            except Exception:
                with state['lock']:
                    if q in state['subscribers']:
                        state['subscribers'].remove(q)
            return
        if self.path.startswith('/session/status'):
            sid = state['session']
            return self._json(200, {sid: {'type': 'idle'}} if sid else {})
        if self.path.startswith('/session/') and self.path.endswith('/message'):
            sid = self.path.split('/')[2]
            return self._json(200, [{
                'info': {'id': 'msg_a', 'role': 'assistant', 'sessionID': sid, 'parentID': 'msg_u', 'finish': 'stop', 'error': None, 'time': {'created': 1, 'completed': 2}},
                'parts': [
                    {'id': 'prt_1', 'type': 'step-start', 'messageID': 'msg_a', 'sessionID': sid},
                    {'id': 'prt_2', 'type': 'text', 'text': 'Served hello.', 'messageID': 'msg_a', 'sessionID': sid, 'time': {'end': 1}},
                    {'id': 'prt_3', 'type': 'step-finish', 'messageID': 'msg_a', 'sessionID': sid, 'reason': 'stop', 'tokens': {'input': 3, 'output': 2, 'reasoning': 0, 'cache': {'read': 0, 'write': 0}}, 'cost': 0},
                ],
            }])
        return self._json(404, {'error': 'missing'})
    def do_POST(self):
        if not self._auth():
            return self._json(401, {'error': 'unauthorized'})
        body = self._read()
        if self.path == '/session':
            state['session'] = 'sess1'
            return self._json(200, {'id': 'sess1'})
        # Every message and command it is sent, for a test to read (cwd is the run's folder).
        if '/prompt_async' in self.path or self.path.endswith('/command'):
            with open('opencode-requests.jsonl', 'a') as log:
                log.write(json.dumps({'path': self.path.rsplit('/', 1)[-1], 'body': body}) + '\n')
        if self.path.endswith('/command') and scenario == 'command-refused':
            return self._json(400, {'error': 'no such command'})
        if '/prompt_async' in self.path or self.path.endswith('/command'):
            state['prompted'] = True
            sid = 'sess1'
            def work():
                time.sleep(0.05)
                publish({'type': 'message.updated', 'properties': {'sessionID': sid, 'info': {'id': 'msg_u', 'role': 'user', 'sessionID': sid}}})
                publish({'type': 'message.updated', 'properties': {'sessionID': sid, 'info': {'id': 'msg_a', 'role': 'assistant', 'sessionID': sid, 'parentID': 'msg_u'}}})
                if scenario == 'approve':
                    publish({'type': 'permission.asked', 'properties': {'sessionID': sid, 'id': 'perm1', 'permission': 'bash', 'patterns': ['echo hi'], 'metadata': {'command': 'echo hi'}}})
                    return
                publish({'type': 'message.part.updated', 'properties': {'sessionID': sid, 'part': {'id': 'prt_1', 'type': 'step-start', 'messageID': 'msg_a', 'sessionID': sid}}})
                publish({'type': 'message.part.delta', 'properties': {'sessionID': sid, 'partID': 'prt_2', 'messageID': 'msg_a', 'field': 'text', 'delta': 'Served '}})
                publish({'type': 'message.part.delta', 'properties': {'sessionID': sid, 'partID': 'prt_2', 'messageID': 'msg_a', 'field': 'text', 'delta': 'hello.'}})
                publish({'type': 'message.part.updated', 'properties': {'sessionID': sid, 'part': {'id': 'prt_2', 'type': 'text', 'text': 'Served hello.', 'messageID': 'msg_a', 'sessionID': sid, 'time': {'end': 1}}}})
                publish({'type': 'message.part.updated', 'properties': {'sessionID': sid, 'part': {'id': 'prt_3', 'type': 'step-finish', 'messageID': 'msg_a', 'sessionID': sid, 'reason': 'stop', 'tokens': {'input': 3, 'output': 2, 'reasoning': 0, 'cache': {'read': 0, 'write': 0}}, 'cost': 0}}})
                publish({'type': 'session.idle', 'properties': {'sessionID': sid}})
            threading.Thread(target=work, daemon=True).start()
            return self._json(200, None)
        if '/permission/' in self.path and self.path.endswith('/reply'):
            sid = 'sess1'
            allowed = (body or {}).get('reply') == 'once'
            def after():
                time.sleep(0.05)
                publish({'type': 'message.part.updated', 'properties': {'sessionID': sid, 'part': {'id': 'prt_1', 'type': 'step-start', 'messageID': 'msg_a', 'sessionID': sid}}})
                publish({'type': 'message.part.updated', 'properties': {'sessionID': sid, 'part': {'id': 'prt_2', 'type': 'text', 'text': 'Allowed.' if allowed else 'Refused.', 'messageID': 'msg_a', 'sessionID': sid, 'time': {'end': 1}}}})
                publish({'type': 'message.part.updated', 'properties': {'sessionID': sid, 'part': {'id': 'prt_3', 'type': 'step-finish', 'messageID': 'msg_a', 'sessionID': sid, 'reason': 'stop', 'tokens': {'input': 1, 'output': 1, 'reasoning': 0, 'cache': {'read': 0, 'write': 0}}, 'cost': 0}}})
                publish({'type': 'session.idle', 'properties': {'sessionID': sid}})
            threading.Thread(target=after, daemon=True).start()
            return self._json(200, None)
        if self.path.endswith('/abort'):
            publish({'type': 'session.idle', 'properties': {'sessionID': state['session'] or 'sess1'}})
            return self._json(200, None)
        return self._json(404, {'error': 'missing'})

httpd = ThreadingHTTPServer(('127.0.0.1', 0), Handler)
host, port = httpd.server_address
print(f'listening on http://127.0.0.1:{port}', flush=True)
try:
    httpd.serve_forever()
except KeyboardInterrupt:
    pass
