import { RELAY_LOCK } from './phone-page.js'

/*
 * LOCUST IN A BROWSER: THE WAY IN (0.830, shared/desk.ts). app.locust.lol/desk, served by the relay. It finds this
 * browser's pairing -- the phone page's own (phone-page.ts), so one pairing does both -- asks the computer, sealed,
 * which Locust it runs and whether this browser may open its window, and then opens that version's own window, which
 * the relay serves from that release (apps/relay/src/worker.ts). No code from the computer ever runs here.
 */
const STYLE = `
:root { color-scheme: dark; --bg: #0b0d0e; --text: #f2efe9; --muted: #8b9190; --button: #f2efe9; --button-text: #0b0d0e; }
* { box-sizing: border-box; }
html, body { margin: 0; background: var(--bg); color: var(--text); }
body { font: 16px/1.5 system-ui, -apple-system, "Segoe UI", Roboto, sans-serif; min-height: 100dvh; display: grid; place-items: center; padding: 24px; }
main { max-width: 460px; display: grid; gap: 14px; text-align: center; }
h1 { margin: 0; font-size: 20px; font-weight: 600; }
p { margin: 0; color: var(--muted); }
button { justify-self: center; min-height: 44px; padding: 0 18px; border-radius: 10px; border: 0; background: var(--button); color: var(--button-text); font: 600 15px system-ui, sans-serif; cursor: pointer; }
`

const SCRIPT = `
  var root = document.getElementById('root');
  function show(title, words, action) {
    var main = document.createElement('main');
    var h = document.createElement('h1'); h.textContent = title; main.appendChild(h);
    var p = document.createElement('p'); p.textContent = words; main.appendChild(p);
    if (action) {
      var b = document.createElement('button'); b.type = 'button'; b.textContent = action.label;
      b.addEventListener('click', action.run); main.appendChild(b);
    }
    root.replaceChildren(main);
  }
  function pairFirst() {
    show('Pair this browser first', 'On your computer, open Locust, then Settings › General › Your phone, and open its code in this browser. Then turn on Full window for it there.', { label: 'Pair this browser', run: function () { location.href = '/'; } });
  }
  function saved() {
    return new Promise(function (resolve) {
      var request;
      try { request = indexedDB.open('locust-relay', 1); } catch (e) { resolve(null); return; }
      request.onupgradeneeded = function () { request.result.createObjectStore('link'); };
      request.onerror = function () { resolve(null); };
      request.onsuccess = function () {
        var db = request.result, read = db.transaction('link', 'readonly').objectStore('link').get('link');
        read.onsuccess = function () { db.close(); resolve(read.result || null); };
        read.onerror = function () { db.close(); resolve(null); };
      };
    });
  }
  var session = lock.b64(crypto.getRandomValues(new Uint8Array(9)));
  var asked = 0, socket = null;
  function hello(link) {
    asked += 1;
    var frame = { v: 1, kind: 'desk', s: session, q: asked, t: Date.now(), items: [{ kind: 'hello', id: 'hello-' + asked }] };
    lock.seal(link.seal, frame, lock.aad(link.room, link.device, 'device-to-pc')).then(function (box) {
      if (socket) socket.send(JSON.stringify({ t: 'send', to: 'pc', n: box.n, c: box.c }));
    });
  }
  function answered(link, frame) {
    lock.open(link.open, frame.n, frame.c, lock.aad(link.room, link.device, 'pc-to-device')).then(function (said) {
      if (!said || said.v !== 1 || said.kind !== 'desk' || !Array.isArray(said.items)) return;
      var res = said.items.find(function (item) { return item && item.kind === 'res' && item.id === 'hello-' + asked; });
      if (!res) return;
      if (!res.ok) { show('Locust cannot open here', res.message || 'Locust on your computer said no.', { label: 'Try again', run: function () { hello(link); } }); return; }
      var info;
      try { info = JSON.parse(res.value); } catch (e) { return; }
      if (!info || typeof info.version !== 'string' || !/^\\d+\\.\\d+\\.\\d+$/.test(info.version)) { show('Locust cannot open here', 'Locust on your computer gave no version this page knows.'); return; }
      try { sessionStorage.setItem('locust-desk', JSON.stringify({ computer: String(info.computer || '').slice(0, 60), theme: info.theme === 'light' ? 'light' : 'dark' })); } catch (e) {}
      location.replace('/desk/v/' + info.version + '/desk.html');
    }, function () {});
  }
  function connect(link) {
    var ws = socket = new WebSocket((location.protocol === 'https:' ? 'wss://' : 'ws://') + location.host + '/v1/room/' + link.room);
    ws.onopen = function () { ws.send(JSON.stringify({ t: 'auth', as: 'device', device: link.device, token: link.token })); };
    ws.onmessage = function (event) {
      var frame; try { frame = JSON.parse(event.data); } catch (e) { return; }
      if ((frame.t === 'ok' && frame.pc) || (frame.t === 'presence' && frame.pc === true)) { show('Opening Locust…', 'Asking your computer which Locust it runs.'); hello(link); return; }
      if ((frame.t === 'ok' && !frame.pc) || (frame.t === 'presence' && frame.pc === false)) { show('Your computer is not connected', 'Check that it is on and awake, and that Locust is open there. This page opens Locust as soon as it is.'); return; }
      if (frame.t === 'removed' || (frame.t === 'err' && (frame.code === 'auth' || frame.code === 'room'))) { pairFirst(); return; }
      if (frame.t === 'msg' && frame.from === 'pc') answered(link, frame);
    };
    ws.onclose = function () { if (socket === ws) { socket = null; setTimeout(function () { connect(link); }, 3000); } };
  }
  if (!window.crypto || !crypto.subtle || !window.indexedDB || !window.WebSocket) {
    show('This browser cannot open Locust', 'Open this page in a current Chrome, Edge, Firefox or Safari.');
  } else {
    show('Opening Locust…', 'Finding this browser’s pairing.');
    saved().then(function (link) { if (link && link.room && link.seal && link.open) connect(link); else pairFirst(); }, pairFirst);
  }
`

/** The way in, as the relay serves it at /desk/. */
export function deskPage(): string {
  return `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta name="theme-color" content="#0b0d0e">
<meta name="referrer" content="no-referrer">
<title>Locust</title>
<style>${STYLE}</style>
</head>
<body>
<div id="root"></div>
<script>(function () {${RELAY_LOCK}${SCRIPT}})();</script>
</body>
</html>`
}
