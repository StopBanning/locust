#!/usr/bin/env python3
"""A scripted OpenCode 1 loopback server for the soak's burst row: once prompted, SOAK_RECORDS (default 50,000) numbered
text deltas on the event stream as fast as it can, then the finished part and the session going idle. Shapes from
scripts/fixtures/m4-opencode-serve.py. It listens on 127.0.0.1 only."""
import base64, json, os, sys, threading, time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

SID = "ses_soak"
password = os.environ.get("OPENCODE_SERVER_PASSWORD", "")
auth = "Basic " + base64.b64encode(("opencode:" + password).encode()).decode()
prompted = threading.Event()

def ev(t, **props):
    return ("data: " + json.dumps({"type": t, "properties": dict(sessionID=SID, **props)}) + "\n\n").encode()

class H(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"
    def log_message(self, *a):
        pass
    def answer(self, value, code=200):
        data = b"" if value is None else json.dumps(value).encode()
        self.send_response(code)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)
    def do_GET(self):
        path = self.path.split("?")[0]
        if self.headers.get("Authorization") != auth:
            return self.answer({"error": "unauthorized"}, 401)
        if path == "/event":
            self.send_response(200)
            self.send_header("Content-Type", "text/event-stream")
            self.send_header("Connection", "close")
            self.end_headers()
            self.wfile.flush()
            prompted.wait(30)
            w = self.wfile
            w.write(ev("message.updated", info={"id": "msg_u", "role": "user", "sessionID": SID}))
            w.write(ev("message.updated", info={"id": "msg_a", "role": "assistant", "sessionID": SID, "parentID": "msg_u"}))
            w.write(ev("message.part.updated", part={"id": "prt_1", "type": "step-start", "messageID": "msg_a", "sessionID": SID}))
            n = int(os.environ.get("SOAK_RECORDS", "50000"))
            for i in range(n):
                w.write(('data: {"type":"message.part.delta","properties":{"sessionID":"%s","partID":"prt_2",'
                         '"messageID":"msg_a","field":"text","delta":"<%d>"}}\n\n' % (SID, i)).encode())
            # As OpenCode does, the finished part repeats the whole text its deltas made.
            w.write(ev("message.part.updated", part={"id": "prt_2", "type": "text", "text": "".join("<%d>" % i for i in range(n)),
                                                       "messageID": "msg_a", "sessionID": SID, "time": {"end": 1}}))
            w.write(ev("message.part.updated", part={"id": "prt_3", "type": "step-finish", "messageID": "msg_a",
                       "sessionID": SID, "reason": "stop", "tokens": {"input": 3, "output": 2, "reasoning": 0,
                       "cache": {"read": 0, "write": 0}}, "cost": 0}))
            w.write(ev("session.idle"))
            w.flush()
            time.sleep(600)
            return
        if path == "/session/status":
            return self.answer({SID: {"type": "idle"}})
        if path.startswith("/session/") and path.endswith("/message"):
            return self.answer([])
        self.answer(None, 404)
    def do_POST(self):
        path = self.path.split("?")[0]
        n = int(self.headers.get("Content-Length", "0"))
        if n:
            self.rfile.read(n)
        if self.headers.get("Authorization") != auth:
            return self.answer({"error": "unauthorized"}, 401)
        if path == "/session":
            return self.answer({"id": SID})
        if path.endswith("/prompt_async"):
            prompted.set()
            return self.answer(None, 204)
        if path.endswith("/abort"):
            return self.answer(True)
        self.answer(None, 404)

class Server(ThreadingHTTPServer):
    daemon_threads = True
    def handle_error(self, *a):
        pass

httpd = Server(("127.0.0.1", 0), H)
print("opencode server listening on http://127.0.0.1:%d" % httpd.server_address[1], flush=True)
httpd.serve_forever()
