/*
 * The page a phone opens (0.775, shared/phone.ts). One file, no scripts from anywhere else (the host's CSP allows
 * none), dark like Locust's own window. Every word from a run goes in with textContent, never as HTML. The key
 * arrives in the address's #fragment, is kept in the phone's storage, and is taken out of the address bar at once.
 *
 * TWO WAYS TO REACH LOCUST (0.826). On the home network the page comes from the PC itself and asks it over HTTP with
 * the key from the code (LAN, below). From anywhere it comes from Locust's relay (apps/relay) and asks the PC through
 * it, each request and answer sealed on one side and opened on the other (RELAY_LOCK and RELAY, below), so the relay
 * passes along what it cannot read. What the phone shows and does is the same either way: UI is one script, and the
 * two differ only in how they ask.
 */
const STYLE = `
:root { color-scheme: dark; --bg: #0b0d0e; --raised: #16191a; --raised-2: #1d2122; --line: rgba(255,255,255,0.08);
  --text: #f2efe9; --muted: #8b9190; --soft: #c7c3bc; --green: #7fc97f; --amber: #efc969; --red: #e4685d;
  --bubble: #23282a; --button: #f2efe9; --button-text: #0b0d0e; }
* { box-sizing: border-box; }
html, body { margin: 0; background: var(--bg); color: var(--text); }
body { font: 16px/1.45 system-ui, -apple-system, "Segoe UI", Roboto, sans-serif; -webkit-text-size-adjust: 100%;
  padding: env(safe-area-inset-top) 0 env(safe-area-inset-bottom); min-height: 100dvh; }
header { position: sticky; top: 0; z-index: 2; display: flex; align-items: center; gap: 10px; padding: 14px 16px;
  background: rgba(11,13,14,0.92); backdrop-filter: blur(10px); border-bottom: 1px solid var(--line); }
header h1 { font-size: 17px; font-weight: 600; margin: 0; flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.dot { width: 8px; height: 8px; border-radius: 50%; background: var(--muted); flex: none; }
.dot[data-on] { background: var(--green); }
.back { border: 0; background: none; color: var(--soft); font: inherit; font-size: 15px; padding: 4px 8px 4px 0; }
main { padding: 12px 16px 24px; display: grid; gap: 22px; }
h2 { font-size: 12px; font-weight: 600; letter-spacing: 0.08em; text-transform: uppercase; color: var(--muted); margin: 0 0 8px; }
.card { background: var(--raised); border: 1px solid var(--line); border-radius: 14px; padding: 14px; display: grid; gap: 10px; }
.card[data-waiting] { border-color: rgba(233,185,73,0.35); }
.who { font-size: 13px; color: var(--amber); }
.what { white-space: pre-wrap; word-break: break-word; font-size: 15px; }
.row { display: flex; gap: 10px; }
button.act { flex: 1; min-height: 44px; border-radius: 10px; font: inherit; font-weight: 600; border: 1px solid var(--line); }
button.yes { background: var(--button); color: var(--button-text); border-color: transparent; }
button.no { background: var(--raised-2); color: var(--text); }
button:disabled { opacity: 0.5; }
ul { list-style: none; margin: 0; padding: 0; display: grid; gap: 8px; }
li > button.convo { width: 100%; text-align: left; background: var(--raised); color: var(--text); border: 1px solid var(--line);
  border-radius: 14px; padding: 12px 14px; font: inherit; display: grid; gap: 3px; }
.title { font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.meta { font-size: 13px; color: var(--muted); display: flex; gap: 6px; align-items: center; }
.meta[data-running] { color: var(--green); }
.snippet { font-size: 14px; color: var(--soft); overflow: hidden; display: -webkit-box; -webkit-line-clamp: 2; -webkit-box-orient: vertical; }
.empty, .note { color: var(--muted); font-size: 15px; }
.turns { display: grid; gap: 14px; padding-bottom: 120px; }
.asked { justify-self: end; max-width: 85%; background: var(--bubble); border-radius: 16px 16px 4px 16px; padding: 10px 13px; white-space: pre-wrap; word-break: break-word; }
.answer { white-space: pre-wrap; word-break: break-word; color: var(--text); }
.by { font-size: 12px; color: var(--muted); margin-bottom: 2px; }
.working { color: var(--green); font-size: 14px; }
form.compose { position: fixed; left: 0; right: 0; bottom: 0; display: flex; gap: 8px; padding: 10px 12px calc(10px + env(safe-area-inset-bottom));
  background: rgba(11,13,14,0.96); border-top: 1px solid var(--line); }
form.compose textarea { flex: 1; resize: none; min-height: 44px; max-height: 140px; border-radius: 12px; border: 1px solid var(--line);
  background: var(--raised); color: var(--text); font: inherit; padding: 10px 12px; }
form.compose button { min-width: 72px; border-radius: 12px; border: 0; background: var(--button); color: var(--button-text); font: inherit; font-weight: 600; }
.pair { padding: 48px 24px; display: grid; gap: 12px; text-align: center; }
.pair strong { font-size: 18px; }
.pair .said { color: var(--soft); font-size: 15px; }
.toast { position: fixed; left: 16px; right: 16px; bottom: calc(80px + env(safe-area-inset-bottom)); background: var(--raised-2);
  border: 1px solid var(--line); border-radius: 12px; padding: 12px 14px; font-size: 14px; }
:focus-visible { outline: 2px solid var(--soft); outline-offset: 2px; }
@media (prefers-reduced-motion: no-preference) { button { transition: opacity 120ms; } }
`

/*
 * Each way of asking is a `transport`, with the same five parts:
 *   paired()          whether this phone holds what it needs to ask Locust anything
 *   api(path, body)   a promise of Locust's answer; it rejects with 'unpaired' once this phone is no longer paired
 *   pairNote()        words for the pairing screen, while pairing is under way or after it failed, or ''
 *   problem()         why Locust cannot be reached right now, or ''
 *   start(changed)    reads what the phone kept and a code in the address; calls changed() whenever the above may
 *                     have changed, changed(true) when what was on screen no longer applies
 */

/** On the home network: the key from the code goes with every request to the PC that served the page. */
const LAN = `
  var transport = (function () {
    var KEY = 'locust-phone-key';
    var key = null;
    var changed = function () {};
    function take(text) {
      if (!/^[A-Za-z0-9_-]{32}$/.test(text)) return false;
      key = text;
      try { localStorage.setItem(KEY, key); } catch (e) {}
      return true;
    }
    return {
      paired: function () { return !!key; },
      pairNote: function () { return ''; },
      problem: function () { return ''; },
      api: function (path, body) {
        return fetch(path, {
          method: body ? 'POST' : 'GET',
          headers: body ? { authorization: 'Bearer ' + key, 'content-type': 'application/json' } : { authorization: 'Bearer ' + key },
          body: body ? JSON.stringify(body) : undefined,
          cache: 'no-store'
        }).then(function (res) {
          if (res.status === 401) { key = null; try { localStorage.removeItem(KEY); } catch (e) {} changed(); throw new Error('unpaired'); }
          return res.json().then(function (data) { if (!res.ok) throw new Error(data && data.message || 'Locust did not answer.'); return data; });
        });
      },
      start: function (onChange) {
        changed = onChange;
        try { key = localStorage.getItem(KEY); } catch (e) {}
        take(location.hash.length > 1 ? location.hash.slice(1) : '');
        if (location.hash) history.replaceState(null, '', location.pathname);
        // A new code opened in this same tab changes only the #fragment, which reloads nothing: take its key here.
        window.addEventListener('hashchange', function () {
          if (!take(location.hash.slice(1))) return;
          history.replaceState(null, '', location.pathname);
          changed(true);
        });
        changed();
      }
    };
  })();
`

/**
 * THE LOCK, in the page: the same steps under the same names as relay-crypto.ts, which a test runs this code against
 * (the-lock-is-the-same-on-both-sides.test.ts). X25519 between the phone's key and the PC's, HKDF to one AES-GCM key
 * for each direction, a random nonce for every box, and the room, the device and the direction bound into each.
 */
export const RELAY_LOCK = `
  var lock = (function () {
    var VERSION = 'locust-relay-v1';
    var subtle = crypto.subtle;
    var enc = new TextEncoder(), dec = new TextDecoder();
    function b64(bytes) {
      var text = '';
      for (var i = 0; i < bytes.length; i += 1) text += String.fromCharCode(bytes[i]);
      return btoa(text).split('+').join('-').split('/').join('_').split('=').join('');
    }
    function unb64(text) {
      var padded = text.split('-').join('+').split('_').join('/');
      while (padded.length % 4) padded += '=';
      var raw = atob(padded), bytes = new Uint8Array(raw.length);
      for (var i = 0; i < raw.length; i += 1) bytes[i] = raw.charCodeAt(i);
      return bytes;
    }
    function sha256(text) {
      return subtle.digest('SHA-256', enc.encode(text)).then(function (digest) { return b64(new Uint8Array(digest)); });
    }
    return {
      b64: b64,
      unb64: unb64,
      sha256: sha256,
      pairingToken: function (secret) { return sha256(VERSION + '|pair-relay|' + secret); },
      pairingProof: function (secret) { return sha256(VERSION + '|pair-proof|' + secret); },
      deviceIdOf: function (publicKey) { return sha256(VERSION + '|device|' + publicKey).then(function (hash) { return hash.slice(0, 22); }); },
      aad: function (room, device, direction) { return VERSION + '|' + room + '|' + device + '|' + direction; },
      linkKeys: function (privateKey, peerPublicKey, room, side) {
        return subtle.importKey('raw', unb64(peerPublicKey), { name: 'X25519' }, false, []).then(function (peer) {
          return subtle.deriveBits({ name: 'X25519', public: peer }, privateKey, 256);
        }).then(function (shared) {
          if (new Uint8Array(shared).every(function (byte) { return byte === 0; })) throw new Error('That key locks nothing.');
          return subtle.importKey('raw', shared, 'HKDF', false, ['deriveKey']);
        }).then(function (base) {
          function key(direction, usage) {
            return subtle.deriveKey({ name: 'HKDF', hash: 'SHA-256', salt: enc.encode(VERSION + '|' + room), info: enc.encode(direction) },
              base, { name: 'AES-GCM', length: 256 }, false, [usage]);
          }
          var mine = side === 'pc' ? 'pc-to-device' : 'device-to-pc';
          var theirs = side === 'pc' ? 'device-to-pc' : 'pc-to-device';
          return Promise.all([key(mine, 'encrypt'), key(theirs, 'decrypt')]).then(function (keys) { return { seal: keys[0], open: keys[1] }; });
        });
      },
      seal: function (key, value, aad) {
        var iv = crypto.getRandomValues(new Uint8Array(12));
        return subtle.encrypt({ name: 'AES-GCM', iv: iv, additionalData: enc.encode(aad) }, key, enc.encode(JSON.stringify(value)))
          .then(function (sealed) { return { n: b64(iv), c: b64(new Uint8Array(sealed)) }; });
      },
      open: function (key, n, c, aad) {
        return subtle.decrypt({ name: 'AES-GCM', iv: unb64(n), additionalData: enc.encode(aad) }, key, unb64(c))
          .then(function (plain) { return JSON.parse(dec.decode(plain)); });
      }
    };
  })();
`

/**
 * From anywhere: the page came from Locust's relay, and asks the PC through it. A code scanned once pairs the phone:
 * it makes its own key, proves to the PC that it saw the code, and is given a token the relay lets it in with. The
 * keys it opens and seals with are kept where the browser keeps keys (IndexedDB) and cannot be read back out of it.
 */
const RELAY = `
  var transport = (function () {
    var RELAY = (location.protocol === 'https:' ? 'wss://' : 'ws://') + location.host + '/v1/room/';
    var OFFLINE = 'Your computer is not connected. Check that it is on and awake, and that Locust is open.';
    var DB = 'locust-relay', STORE = 'link';
    var link = null, loaded = false, note = '', trouble = '';
    var socket = null, ready = false, pcOn = false, retry = 0, retryTimer = null, pingTimer = null;
    var waiting = {}, nextId = 0, session = lock.b64(crypto.getRandomValues(new Uint8Array(6)));
    var changed = function () {};

    function keep(mode, work) {
      return new Promise(function (resolve, reject) {
        var request = indexedDB.open(DB, 1);
        request.onupgradeneeded = function () { request.result.createObjectStore(STORE); };
        request.onerror = function () { reject(request.error); };
        request.onsuccess = function () {
          var db = request.result, tx = db.transaction(STORE, mode), result;
          var step = work(tx.objectStore(STORE));
          step.onsuccess = function () { result = step.result; };
          tx.oncomplete = function () { db.close(); resolve(result); };
          tx.onerror = tx.onabort = function () { db.close(); reject(tx.error); };
        };
      });
    }
    function saved() { return keep('readonly', function (store) { return store.get('link'); }); }
    function save(value) { return keep('readwrite', function (store) { return store.put(value, 'link'); }); }
    function forget() { return keep('readwrite', function (store) { return store.delete('link'); }); }

    function deviceName() {
      var agent = navigator.userAgent || '';
      return /iPhone/.test(agent) ? 'iPhone' : /iPad/.test(agent) ? 'iPad' : /Android/.test(agent) ? 'Android phone' : 'Browser';
    }
    function readCode() {
      var hash = location.hash;
      if (hash.indexOf('#p=') !== 0) return null;
      history.replaceState(null, '', location.pathname);
      var parts = hash.slice(3).split('.');
      if (parts.length !== 3 || !/^[A-Za-z0-9_-]{22}$/.test(parts[0]) || !/^[A-Za-z0-9_-]{43}$/.test(parts[1]) || !/^[A-Za-z0-9_-]{22}$/.test(parts[2])) return null;
      return { room: parts[0], pc: parts[1], secret: parts[2] };
    }
    function failAll(message) {
      Object.keys(waiting).forEach(function (id) { var wait = waiting[id]; delete waiting[id]; clearTimeout(wait.timer); wait.reject(new Error(message)); });
    }
    function closeSocket() {
      var ws = socket;
      socket = null; ready = false;
      clearTimeout(retryTimer); clearInterval(pingTimer);
      if (ws) { ws.onclose = null; try { ws.close(); } catch (e) {} }
    }
    function unpair(message) {
      link = null; note = message; trouble = '';
      closeSocket(); failAll('unpaired');
      forget().catch(function () {});
      changed(true);
    }

    function pair(code) {
      closeSocket(); failAll('unpaired');
      link = null; note = 'Pairing with Locust…'; changed(true);
      var device, publicKey, keys;
      return crypto.subtle.generateKey({ name: 'X25519' }, false, ['deriveBits']).catch(function () {
        throw new Error('This browser cannot make the keys Locust needs. Update it, or open the code in Safari or Chrome.');
      }).then(function (made) {
        return crypto.subtle.exportKey('raw', made.publicKey).then(function (raw) {
          publicKey = lock.b64(new Uint8Array(raw));
          return Promise.all([lock.deviceIdOf(publicKey), lock.linkKeys(made.privateKey, code.pc, code.room, 'device'), lock.pairingToken(code.secret), lock.pairingProof(code.secret)]);
        });
      }).then(function (got) {
        device = got[0]; keys = got[1];
        var token = got[2], proof = got[3];
        return new Promise(function (resolve, reject) {
          var ws = new WebSocket(RELAY + code.room);
          var timer = setTimeout(function () { finish(new Error('Locust did not answer. Make a new code on your computer and scan it again.')); }, 30000);
          function finish(error, value) {
            clearTimeout(timer); ws.onclose = null;
            try { ws.close(); } catch (e) {}
            if (error) reject(error); else resolve(value);
          }
          ws.onopen = function () { ws.send(JSON.stringify({ t: 'auth', as: 'pairing', token: token })); };
          ws.onerror = function () { finish(new Error('Locust’s relay could not be reached. Check this phone’s connection, then scan the code again.')); };
          ws.onclose = function () { finish(new Error('The connection closed before pairing finished. Make a new code and scan it again.')); };
          ws.onmessage = function (event) {
            var frame;
            try { frame = JSON.parse(event.data); } catch (e) { return; }
            if (frame.t === 'err') {
              finish(new Error(frame.code === 'pairing-closed' || frame.code === 'auth' ? 'That code was already used, or it ran out. Make a new one in Locust and scan it again.'
                : frame.code === 'pc-offline' ? 'Locust is not connected on your computer. Open it, make a new code and scan it again.'
                : 'Pairing did not work. Make a new code and scan it again.'));
              return;
            }
            if (frame.t === 'ok') {
              if (!frame.pc) { finish(new Error('Locust is not connected on your computer. Open it, make a new code and scan it again.')); return; }
              lock.seal(keys.seal, { v: 1, kind: 'pair', proof: proof, name: deviceName() }, lock.aad(code.room, device, 'device-to-pc')).then(function (box) {
                ws.send(JSON.stringify({ t: 'send', to: 'pc', n: box.n, c: box.c, k: publicKey }));
              });
              return;
            }
            if (frame.t === 'msg' && frame.from === 'pc') {
              lock.open(keys.open, frame.n, frame.c, lock.aad(code.room, device, 'pc-to-device')).then(function (said) {
                if (said && said.v === 1 && said.kind === 'paired' && said.device === device && /^[A-Za-z0-9_-]{43}$/.test(said.token)) finish(null, said.token);
                else finish(new Error('Locust did not take this phone. Make a new code and scan it again.'));
              }, function () { finish(new Error('Locust’s answer could not be opened. Make a new code and scan it again.')); });
            }
          };
        });
      }).then(function (token) {
        var next = { room: code.room, device: device, token: token, seal: keys.seal, open: keys.open, seq: 0 };
        return save(next).then(function () { link = next; note = ''; connect(); changed(true); });
      }).catch(function (error) {
        link = null; note = error && error.message ? error.message : 'Pairing did not work. Make a new code and scan it again.';
        changed(true);
      });
    }

    function answer(frame) {
      var current = link;
      lock.open(current.open, frame.n, frame.c, lock.aad(current.room, current.device, 'pc-to-device')).then(function (said) {
        // Only an answer to a question this page asked: anything else, an old one included, is dropped.
        var wait = said && said.v === 1 && said.kind === 'res' && typeof said.id === 'string' ? waiting[said.id] : undefined;
        if (!wait) return;
        delete waiting[said.id]; clearTimeout(wait.timer);
        if (said.status === 401) { wait.reject(new Error('unpaired')); unpair('This phone is no longer paired. Scan a new code in Locust to pair it again.'); return; }
        if (said.status >= 200 && said.status < 300) wait.resolve(said.data);
        else wait.reject(new Error(said.data && said.data.message || 'Locust did not answer.'));
      }, function () { /* A box that does not open with this phone's key is not for it. */ });
    }

    function connect() {
      if (!link || socket) return;
      clearTimeout(retryTimer);
      var ws = socket = new WebSocket(RELAY + link.room);
      ready = false;
      ws.onopen = function () { ws.send(JSON.stringify({ t: 'auth', as: 'device', device: link.device, token: link.token })); };
      ws.onmessage = function (event) {
        var frame;
        try { frame = JSON.parse(event.data); } catch (e) { return; }
        if (frame.t === 'ok') { ready = true; retry = 0; pcOn = !!frame.pc; trouble = pcOn ? '' : OFFLINE; changed(); return; }
        if (frame.t === 'presence' && typeof frame.pc === 'boolean') { pcOn = frame.pc; trouble = pcOn ? '' : OFFLINE; changed(); return; }
        if (frame.t === 'removed') { unpair('Locust removed this phone. Scan a new code in Locust to pair it again.'); return; }
        if (frame.t === 'err' && (frame.code === 'auth' || frame.code === 'room')) { unpair('This phone is no longer paired. Scan a new code in Locust to pair it again.'); return; }
        if (frame.t === 'err') {
          if (frame.code === 'pc-offline') { pcOn = false; trouble = OFFLINE; failAll(OFFLINE); changed(); }
          else failAll('Locust’s relay did not pass that on. Try again.');
          return;
        }
        if (frame.t === 'msg' && frame.from === 'pc') answer(frame);
      };
      ws.onclose = function () {
        if (socket !== ws) return;
        socket = null; ready = false; clearInterval(pingTimer);
        trouble = 'Reaching Locust again…';
        failAll(trouble);
        changed();
        if (link) { retry = Math.min(retry + 1, 6); retryTimer = setTimeout(connect, 1000 * Math.pow(2, retry - 1)); }
      };
      clearInterval(pingTimer);
      // Answered by the relay itself ('pong'), so keeping the line open wakes nothing and costs nothing.
      pingTimer = setInterval(function () { if (socket === ws && ws.readyState === 1) ws.send('ping'); }, 25000);
    }

    return {
      paired: function () { return link !== null; },
      pairNote: function () { return loaded ? note : 'Opening…'; },
      problem: function () { return link ? trouble : ''; },
      api: function (path, body) {
        var current = link;
        if (!current) return Promise.reject(new Error('unpaired'));
        if (!ready || !socket) return Promise.reject(new Error(trouble || 'Reaching Locust…'));
        if (!pcOn) return Promise.reject(new Error(OFFLINE));
        nextId += 1;
        var id = session + '-' + nextId;
        current.seq += 1;
        var request = { v: 1, kind: 'req', id: id, seq: current.seq, t: Date.now(), path: path };
        if (body) request.body = body;
        // The count goes to storage first: a request is never sent with a number Locust may have seen already.
        return save(current).then(function () {
          return lock.seal(current.seal, request, lock.aad(current.room, current.device, 'device-to-pc'));
        }).then(function (box) {
          return new Promise(function (resolve, reject) {
            if (!socket || link !== current) { reject(new Error(trouble || 'Reaching Locust…')); return; }
            waiting[id] = { resolve: resolve, reject: reject, timer: setTimeout(function () { delete waiting[id]; reject(new Error('Locust took too long to answer. Try again.')); }, 20000) };
            socket.send(JSON.stringify({ t: 'send', to: 'pc', n: box.n, c: box.c }));
          });
        });
      },
      start: function (onChange) {
        changed = onChange;
        if (!window.crypto || !crypto.subtle || !window.indexedDB || !window.WebSocket) {
          loaded = true; note = 'This browser cannot lock messages the way Locust needs. Open the code in Safari or Chrome.'; changed(); return;
        }
        var code = readCode();
        saved().then(function (kept) {
          loaded = true;
          if (code) { pair(code); return; }
          if (kept && kept.room && kept.seal && kept.open) { link = kept; connect(); }
          changed();
        }, function () {
          loaded = true;
          note = 'This browser would not keep Locust’s keys. A private tab does not keep them: open the code in a normal one.';
          changed();
        });
        // A new code opened in this same tab changes only the #fragment, which reloads nothing.
        window.addEventListener('hashchange', function () { var next = readCode(); if (next) pair(next); });
        document.addEventListener('visibilitychange', function () { if (document.visibilityState === 'visible' && link && !socket) connect(); });
      }
    };
  })();
`

/** What the phone shows and does, whichever way it asks. */
const UI = `
  var root = document.getElementById('root');
  var open = null;       // the conversation shown, by its newest turn's id
  var openTitle = '';
  var state = null;
  var timer = null;
  var draft = '';

  function el(tag, attrs, children) {
    var node = document.createElement(tag);
    if (attrs) for (var name in attrs) {
      if (name === 'text') node.textContent = attrs[name];
      else if (name === 'on') for (var ev in attrs.on) node.addEventListener(ev, attrs.on[ev]);
      else if (attrs[name] !== false && attrs[name] != null) node.setAttribute(name, attrs[name] === true ? '' : attrs[name]);
    }
    (children || []).forEach(function (child) { if (child) node.appendChild(child); });
    return node;
  }
  function when(at) {
    var then = new Date(at), now = new Date();
    if (isNaN(then)) return '';
    var mins = Math.round((now - then) / 60000);
    if (mins < 1) return 'just now';
    if (mins < 60) return mins + ' min ago';
    if (mins < 60 * 24) return Math.round(mins / 60) + ' h ago';
    return then.toLocaleDateString();
  }
  function phaseWords(phase) {
    return phase === 'running' ? 'working' : phase === 'completed' ? 'done' : phase === 'failed' ? 'stopped with an error'
      : phase === 'cancelled' ? 'stopped' : 'stopped before it finished';
  }
  function toast(text) {
    var old = document.querySelector('.toast'); if (old) old.remove();
    var t = el('div', { class: 'toast', role: 'status', text: text });
    document.body.appendChild(t);
    setTimeout(function () { t.remove(); }, 4000);
  }
  function header(title, back) {
    return el('header', null, [
      back ? el('button', { class: 'back', type: 'button', 'aria-label': 'Back to conversations', text: '‹ Back', on: { click: back } }) : null,
      el('h1', { text: title }),
      el('span', { class: 'dot', 'data-on': state !== null, title: state !== null ? 'Connected to Locust' : 'Not reaching Locust' })
    ]);
  }
  function pairScreen() {
    var said = transport.pairNote();
    root.replaceChildren(el('div', { class: 'pair' }, [
      el('strong', { text: 'Pair this phone with Locust' }),
      el('div', { class: 'note', text: 'On your computer, open Locust, then Settings › Your phone, and scan the code with this phone’s camera.' }),
      said ? el('div', { class: 'said', role: 'status', text: said }) : null
    ]));
  }
  function waitingCard(item) {
    var card = el('div', { class: 'card', 'data-waiting': true });
    var buttons = [
      el('button', { class: 'act yes', type: 'button', text: 'Allow once' }),
      el('button', { class: 'act no', type: 'button', text: 'Deny' })
    ];
    buttons.forEach(function (button, index) {
      button.addEventListener('click', function () {
        buttons.forEach(function (b) { b.disabled = true; });
        transport.api('/api/decide', { approvalId: item.approvalId, approve: index === 0 }).then(function (r) {
          toast(r.ok ? (index === 0 ? 'Allowed once.' : 'Denied.') : 'That card was already answered.');
          refresh();
        }).catch(function (e) { buttons.forEach(function (b) { b.disabled = false; }); if (e.message !== 'unpaired') toast(e.message); });
      });
    });
    card.append(el('div', { class: 'who', text: (item.name ? item.name + ' asks' : 'An agent asks') + ' · ' + when(item.requestedAt) }),
      el('div', { class: 'what', text: item.summary }), el('div', { class: 'row' }, buttons));
    return card;
  }
  function listScreen() {
    var parts = [header('Locust')];
    var main = el('main');
    if (state === null) main.append(el('div', { class: 'note', text: transport.problem() || 'Reaching Locust…' }));
    else {
      if (state.waiting.length > 0) {
        var section = el('section', null, [el('h2', { text: 'Waiting on you' })]);
        var stack = el('div', { style: 'display:grid;gap:10px' });
        state.waiting.forEach(function (item) { stack.append(waitingCard(item)); });
        section.append(stack); main.append(section);
      }
      var convos = el('section', null, [el('h2', { text: 'Conversations' })]);
      if (state.conversations.length === 0) convos.append(el('div', { class: 'empty', text: 'No conversations yet. Start one in Locust on your computer.' }));
      else {
        var list = el('ul');
        state.conversations.forEach(function (c) {
          list.append(el('li', null, [el('button', { class: 'convo', type: 'button', on: { click: function () { open = c.missionId; openTitle = c.title; draft = ''; render(); refresh(); } } }, [
            el('span', { class: 'title', text: c.title }),
            el('span', { class: 'meta', 'data-running': c.phase === 'running', text: c.agent + ' · ' + phaseWords(c.phase) + ' · ' + when(c.at) }),
            c.snippet ? el('span', { class: 'snippet', text: c.snippet }) : null
          ])]));
        });
        convos.append(list);
      }
      main.append(convos);
    }
    parts.push(main);
    root.replaceChildren.apply(root, parts);
  }
  var turns = null;
  function threadScreen() {
    var main = el('main');
    var list = el('div', { class: 'turns' });
    if (turns === null) list.append(el('div', { class: 'note', text: transport.problem() || 'Opening…' }));
    else turns.forEach(function (turn) {
      if (turn.asked) list.append(el('div', { class: 'asked', text: turn.asked }));
      var said = el('div', null, [el('div', { class: 'by', text: turn.agent })]);
      if (turn.answer) said.append(el('div', { class: 'answer', text: turn.answer }));
      if (turn.phase === 'running') said.append(el('div', { class: 'working', text: 'Working…' }));
      else if (!turn.answer) said.append(el('div', { class: 'note', text: 'This turn ' + phaseWords(turn.phase) + ' without an answer.' }));
      list.append(said);
    });
    main.append(list);
    var box = el('textarea', { rows: '1', placeholder: 'Send the next message', 'aria-label': 'Message' });
    box.value = draft;
    box.addEventListener('input', function () { draft = box.value; box.style.height = 'auto'; box.style.height = box.scrollHeight + 'px'; });
    var send = el('button', { type: 'submit', text: 'Send' });
    var running = turns !== null && turns.length > 0 && turns[turns.length - 1].phase === 'running';
    send.disabled = running;
    if (running) box.placeholder = 'Wait for this turn to finish';
    var form = el('form', { class: 'compose', on: { submit: function (event) {
      event.preventDefault();
      var text = box.value.trim(); if (!text || !turns || turns.length === 0) return;
      send.disabled = true;
      transport.api('/api/send', { missionId: turns[turns.length - 1].missionId, text: text }).then(function (r) {
        if (!r.ok) { send.disabled = false; toast(r.message || 'Locust could not send that.'); return; }
        draft = ''; toast('Sent.'); refresh();
      }).catch(function (e) { send.disabled = false; if (e.message !== 'unpaired') toast(e.message); });
    } } }, [box, send]);
    var focused = document.activeElement && document.activeElement.tagName === 'TEXTAREA';
    root.replaceChildren(header(openTitle, function () { open = null; turns = null; render(); refresh(); }), main, form);
    if (focused) { box.focus(); box.setSelectionRange(box.value.length, box.value.length); }
  }
  function render() {
    if (!transport.paired()) { pairScreen(); return; }
    if (open) threadScreen(); else listScreen();
  }
  function refresh() {
    clearTimeout(timer);
    if (!transport.paired()) return;
    var asked = open;
    var work = asked
      ? transport.api('/api/turns?id=' + encodeURIComponent(asked)).then(function (r) {
          if (open !== asked) return;
          var before = turns === null ? 0 : turns.length;
          turns = r.turns;
          // A message sent from here starts a newer turn: follow it.
          if (turns.length > 0) open = turns[turns.length - 1].missionId;
          var atEnd = window.innerHeight + window.scrollY >= document.body.scrollHeight - 40;
          render();
          if (turns.length !== before || atEnd) window.scrollTo(0, document.body.scrollHeight);
        })
      : transport.api('/api/state').then(function (r) { state = r; render(); });
    work.catch(function (e) {
      if (e.message === 'unpaired') return;
      if (!asked) { state = null; render(); }
    }).then(function () {
      if (transport.paired() && document.visibilityState === 'visible') timer = setTimeout(refresh, 3000);
    });
  }
  document.addEventListener('visibilitychange', function () { if (document.visibilityState === 'visible') refresh(); });
  render();
  transport.start(function (reset) {
    if (reset) { open = null; turns = null; state = null; }
    render(); refresh();
  });
`

/** The page: on the home network (`lan`, served by the PC) or from Locust's relay (`relay`). */
export function phonePage(options: { readonly via?: 'lan' | 'relay' } = {}): string {
  const script = options.via === 'relay' ? `(function () {${RELAY_LOCK}${RELAY}${UI}})();` : `(function () {${LAN}${UI}})();`
  return `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover">
<meta name="theme-color" content="#0b0d0e">
<meta name="referrer" content="no-referrer">
<title>Locust</title>
<style>${STYLE}</style>
</head>
<body>
<div id="root"></div>
<script>${script}</script>
</body>
</html>`
}
