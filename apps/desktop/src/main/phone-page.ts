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
.desk { padding-top: 4px; }
.more { color: var(--soft); font-size: 14px; }
.back { border: 0; background: none; color: var(--soft); font: inherit; font-size: 15px; padding: 4px 8px 4px 0; }
main { padding: 12px 16px 24px; display: grid; gap: 22px; }
h2 { font-size: 12px; font-weight: 600; letter-spacing: 0.08em; text-transform: uppercase; color: var(--muted); margin: 0 0 8px; }
.card { background: var(--raised); border: 1px solid var(--line); border-radius: 14px; padding: 14px; display: grid; gap: 10px; }
.card[data-waiting] { border-color: rgba(233,185,73,0.35); }
.who { font-size: 13px; color: var(--amber); }
/* A message left for the computer waits on nobody; one it held waits on the person (0.837). */
.card[data-note="waiting"] .who { color: var(--muted); }
.card[data-note="old"], .card[data-note="gone"], .card[data-note="failed"] { border-color: rgba(233,185,73,0.35); }
.what { white-space: pre-wrap; word-break: break-word; font-size: 15px; }
.row { display: flex; gap: 10px; }
button.act, a.act { flex: 1; min-height: 44px; border-radius: 10px; font: inherit; font-weight: 600; border: 1px solid var(--line); }
a.act { display: flex; align-items: center; justify-content: center; text-decoration: none; box-sizing: border-box; }
button.yes, a.yes { background: var(--button); color: var(--button-text); border-color: transparent; }
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
.alerts { display: grid; gap: 10px; }
.alerts h2 { margin: 0; }
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
    // Scout's away looks (0.829): what they found, opened here with the key Locust gave this phone.
    var away = null, awayAsked = false;

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
    // Unpaired, what it last saw and its waiting notes go with the link (0.837).
    function forget() {
      keptView = null; myNotes = []; keptText = '';
      return keep('readwrite', function (store) { store.delete('seen'); store.delete('seen-key'); return store.delete('link'); });
    }

    /*
     * WHAT IT LAST SAW, AND WHAT WAITS (0.837). The conversations this phone last read are kept here, sealed with a key
     * the browser keeps and will not hand out, so with the computer off it still shows them. A message written then is
     * left at the relay as a note, sealed as a request is, and Locust sends it when the computer is back. This phone
     * keeps its own words for each; the relay has only the sealed box, and says which still wait, and why one was held.
     */
    var keptView = null, keptText = '', myNotes = [], noteWaits = {};
    var SEEN_AAD = new TextEncoder().encode('locust-phone-seen-v1');
    var NOTE_ERRORS = {
      full: 'Twenty messages are already waiting. Take one back, or wait for your computer.',
      'too-fast': 'That was a lot at once. Wait a moment, then send it.',
      'bad-frame': 'That message is too long to wait. Keep it under 8,000 characters.',
      'not-owner': 'This phone cannot leave messages. Scan a new code in Locust to pair it again.'
    };
    function seenKey() {
      return keep('readonly', function (store) { return store.get('seen-key'); }).then(function (key) {
        if (key) return key;
        // Not extractable: the browser seals and opens with it, and gives it to no one.
        return crypto.subtle.generateKey({ name: 'AES-GCM', length: 256 }, false, ['encrypt', 'decrypt']).then(function (made) {
          return keep('readwrite', function (store) { return store.put(made, 'seen-key'); }).then(function () { return made; });
        });
      });
    }
    function writeKept() {
      if (!link) return Promise.resolve();
      var text = JSON.stringify({ view: keptView, notes: myNotes });
      if (text === keptText) return Promise.resolve();
      keptText = text;
      return seenKey().then(function (key) {
        var iv = crypto.getRandomValues(new Uint8Array(12));
        return crypto.subtle.encrypt({ name: 'AES-GCM', iv: iv, additionalData: SEEN_AAD }, key, new TextEncoder().encode(text)).then(function (box) {
          return keep('readwrite', function (store) { return store.put({ iv: iv, box: box }, 'seen'); });
        });
      }).catch(function () { keptText = ''; });
    }
    function readKept() {
      return Promise.all([seenKey(), keep('readonly', function (store) { return store.get('seen'); })]).then(function (got) {
        var sealed = got[1];
        if (!sealed || !sealed.iv || !sealed.box) return;
        return crypto.subtle.decrypt({ name: 'AES-GCM', iv: sealed.iv, additionalData: SEEN_AAD }, got[0], sealed.box).then(function (plain) {
          var text = new TextDecoder().decode(plain), value = JSON.parse(text);
          keptText = text;
          keptView = value && value.view && typeof value.view.at === 'number' ? value.view : null;
          myNotes = value && Array.isArray(value.notes) ? value.notes.filter(function (one) { return one && typeof one.note === 'string' && typeof one.text === 'string'; }) : [];
        });
      }).catch(function () {});
    }
    function cut(text, most) { return typeof text === 'string' && text.length > most ? text.slice(0, most) + '…' : text; }
    function noteAsk(frame) {
      return new Promise(function (resolve, reject) {
        if (!socket || !ready) { reject(new Error(trouble || 'Reaching Locust’s relay…')); return; }
        nextId += 1;
        var id = 'n-' + nextId;
        frame.id = id;
        noteWaits[id] = { resolve: resolve, reject: reject, timer: setTimeout(function () { delete noteWaits[id]; reject(new Error('Locust’s relay did not answer. Try again.')); }, 15000) };
        socket.send(JSON.stringify(frame));
      });
    }
    function noteAnswer(frame) {
      var wait = typeof frame.id === 'string' ? noteWaits[frame.id] : undefined;
      if (!wait) return false;
      delete noteWaits[frame.id]; clearTimeout(wait.timer);
      if (frame.t !== 'err') { wait.resolve(frame); return true; }
      var error = new Error(NOTE_ERRORS[frame.code] || 'Locust’s relay did not keep that. Try again.');
      error.code = frame.code;
      wait.reject(error);
      return true;
    }
    // Which of this phone's notes still wait: one the relay no longer has, inside its week, was sent by Locust.
    function syncNotes() {
      if (!link || !socket || !ready) return;
      noteAsk({ t: 'notes' }).then(function (answer) {
        var there = {};
        (Array.isArray(answer.items) ? answer.items : []).forEach(function (item) { if (item && typeof item.id === 'string') there[item.id] = item; });
        var sent = 0;
        myNotes = myNotes.filter(function (one) {
          var item = there[one.note];
          if (item) { one.held = item.held === 'old' || item.held === 'gone' || item.held === 'failed' ? item.held : null; return true; }
          if (Date.now() - one.t < 7 * 24 * 3600 * 1000) sent += 1;
          return false;
        });
        writeKept();
        if (sent > 0 && self.onSent) self.onSent(sent);
        changed();
      }, function () {});
    }

    // A phone by its kind; a computer's browser by its name and system (0.830), as Settings lists it.
    function deviceName() {
      var agent = navigator.userAgent || '';
      if (/iPhone/.test(agent)) return 'iPhone';
      if (/iPad/.test(agent)) return 'iPad';
      if (/Android/.test(agent)) return /Mobile/.test(agent) ? 'Android phone' : 'Android tablet';
      // Plain words, not patterns: a slash in a pattern here is lost on its way into the page.
      function has(word) { return agent.indexOf(word) >= 0; }
      var browser = has('Edg/') ? 'Edge' : has('OPR/') ? 'Opera' : has('Firefox/') ? 'Firefox' : has('Chrome/') ? 'Chrome' : has('Safari/') ? 'Safari' : 'Browser';
      var system = /Windows/.test(agent) ? 'Windows' : /Macintosh|Mac OS X/.test(agent) ? 'Mac' : /CrOS/.test(agent) ? 'ChromeOS' : /Linux/.test(agent) ? 'Linux' : '';
      return system ? browser + ' on ' + system : browser;
    }
    /*
     * A CODE, OR A GITHUB TICKET (0.838). #p= is a code from Settings: room, the PC's key, the secret, and a 'd' when
     * Open in your browser made it, to go on to the full window. #g= is what signing in with GitHub hands this browser
     * (apps/relay/src/github.ts): room, the PC's key, a ticket the relay lets in once, and GitHub's token, which only
     * the PC reads, to check the account with GitHub itself. Neither stays in the address.
     */
    function readCode() {
      var hash = location.hash;
      if (hash.indexOf('#p=') !== 0 && hash.indexOf('#g=') !== 0) return null;
      history.replaceState(null, '', location.pathname);
      var parts = hash.slice(3).split('.');
      if (!/^[A-Za-z0-9_-]{22}$/.test(parts[0] || '') || !/^[A-Za-z0-9_-]{43}$/.test(parts[1] || '')) return null;
      if (hash.indexOf('#g=') === 0) {
        if (parts.length !== 4 || !/^[A-Za-z0-9_-]{43}$/.test(parts[2]) || !/^[A-Za-z0-9_]{20,255}$/.test(parts[3])) return null;
        return { room: parts[0], pc: parts[1], ticket: parts[2], github: parts[3], desk: true };
      }
      if ((parts.length !== 3 && !(parts.length === 4 && parts[3] === 'd')) || !/^[A-Za-z0-9_-]{22}$/.test(parts[2])) return null;
      return { room: parts[0], pc: parts[1], secret: parts[2], desk: parts.length === 4 };
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

    var signinFailed = false;
    function pair(code) {
      signinFailed = false;
      closeSocket(); failAll('unpaired');
      link = null; note = 'Pairing with Locust…'; changed(true);
      var device, publicKey, keys;
      return crypto.subtle.generateKey({ name: 'X25519' }, false, ['deriveBits']).catch(function () {
        throw new Error('This browser cannot make the keys Locust needs. Update it, or open the code in Safari or Chrome.');
      }).then(function (made) {
        return crypto.subtle.exportKey('raw', made.publicKey).then(function (raw) {
          publicKey = lock.b64(new Uint8Array(raw));
          return Promise.all([lock.deviceIdOf(publicKey), lock.linkKeys(made.privateKey, code.pc, code.room, 'device'),
            code.ticket ? code.ticket : lock.pairingToken(code.secret), code.ticket ? '' : lock.pairingProof(code.secret)]);
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
              finish(new Error(code.ticket && (frame.code === 'pairing-closed' || frame.code === 'auth') ? 'That sign-in was already used, or it ran out. Sign in with GitHub again.'
                : frame.code === 'pairing-closed' || frame.code === 'auth' ? 'That code was already used, or it ran out. Make a new one in Locust and scan it again.'
                : frame.code === 'pc-offline' ? 'Locust is not connected on your computer. Open it, make a new code and scan it again.'
                : 'Pairing did not work. Make a new code and scan it again.'));
              return;
            }
            if (frame.t === 'ok') {
              if (!frame.pc) { finish(new Error(code.ticket ? 'Locust is not connected on your computer. Check that it is on and Locust is open there, then sign in with GitHub again.' : 'Locust is not connected on your computer. Open it, make a new code and scan it again.')); return; }
              var hello = code.ticket ? { v: 1, kind: 'signin', github: code.github, name: deviceName() } : { v: 1, kind: 'pair', proof: proof, name: deviceName() };
              lock.seal(keys.seal, hello, lock.aad(code.room, device, 'device-to-pc')).then(function (box) {
                ws.send(JSON.stringify({ t: 'send', to: 'pc', n: box.n, c: box.c, k: publicKey }));
              });
              return;
            }
            if (frame.t === 'msg' && frame.from === 'pc') {
              lock.open(keys.open, frame.n, frame.c, lock.aad(code.room, device, 'pc-to-device')).then(function (said) {
                if (said && said.v === 1 && said.kind === 'paired' && said.device === device && /^[A-Za-z0-9_-]{43}$/.test(said.token)) finish(null, said.token);
                else if (code.ticket && said && said.kind === 'refused') finish(new Error(said.message || 'Locust on your computer did not take this sign-in.'));
                else finish(new Error('Locust did not take this phone. Make a new code and scan it again.'));
              }, function () { finish(new Error('Locust’s answer could not be opened. Make a new code and scan it again.')); });
            }
          };
        });
      }).then(function (token) {
        var next = { room: code.room, device: device, token: token, seal: keys.seal, open: keys.open, seq: 0 };
        return save(next).then(function () {
          // Opened for the full window: on to it, with the pairing it shares.
          if (code.desk) { location.replace('/desk/'); return; }
          link = next; note = ''; connect(); changed(true);
        });
      }).catch(function (error) {
        signinFailed = !!code.ticket;
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
      // Out of sight, no line (see rest): a page opened in the background opens it when it is seen.
      if (!link || socket || document.visibilityState === 'hidden') return;
      clearTimeout(retryTimer);
      var ws = socket = new WebSocket(RELAY + link.room);
      ready = false;
      ws.onopen = function () { ws.send(JSON.stringify({ t: 'auth', as: 'device', device: link.device, token: link.token })); };
      ws.onmessage = function (event) {
        var frame;
        try { frame = JSON.parse(event.data); } catch (e) { return; }
        if (frame.t === 'ok') { ready = true; retry = 0; pcOn = !!frame.pc; trouble = pcOn ? '' : OFFLINE; changed(); syncNotes(); if (pcOn) { refreshAlerts(); awayAsked = false; refreshAway(); } else readLetters(); return; }
        if (frame.t === 'presence' && typeof frame.pc === 'boolean') { pcOn = frame.pc; trouble = pcOn ? '' : OFFLINE; changed(); syncNotes(); if (pcOn) { refreshAlerts(); awayAsked = false; refreshAway(); } else readLetters(); return; }
        if ((frame.t === 'noted' || frame.t === 'unnoted' || frame.t === 'my-notes' || frame.t === 'err') && noteAnswer(frame)) return;
        if (frame.t === 'notes-changed') { syncNotes(); return; }
        if (frame.t === 'letters' && frame.id === 'away' && Array.isArray(frame.items)) { openLetters(frame.items); return; }
        if (frame.t === 'letter') { if (!pcOn) readLetters(); return; }
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

    /*
     * OUT OF SIGHT, THE LINE IS LET GO (0.827). Locust sends an alert only to a phone whose page is not open, since an
     * open page shows everything itself. A locked phone or a page in the background kept its line, and a line a phone
     * drops without closing stays open at the relay, so Locust took the phone for open and sent nothing (the alerts
     * drive, 10/23). Hidden, the page closes its line; seen again, it opens it again.
     */
    function rest() {
      if (!socket) return;
      var ws = socket;
      socket = null; ready = false; clearInterval(pingTimer); clearTimeout(retryTimer);
      failAll('Reaching Locust…');
      try { ws.close(1000); } catch (e) {}
    }

    /*
     * ALERTS (0.827). The browser's push service brings an alert this page's PC sealed for it, through the relay; the
     * service worker (PHONE_SERVICE_WORKER) shows it. An iPhone shows web alerts only from the Home Screen.
     */
    var alertsOn = false, alertsBusy = false, alertsChecked = false;
    function pushable() { return 'serviceWorker' in navigator && 'PushManager' in window && 'Notification' in window; }
    function onHomeScreen() { return navigator.standalone === true || (!!window.matchMedia && window.matchMedia('(display-mode: standalone)').matches); }
    function rememberAlerts(on) { try { if (on) localStorage.setItem('locust-alerts', '1'); else localStorage.removeItem('locust-alerts'); } catch (e) {} }
    function wantedAlerts() { try { return localStorage.getItem('locust-alerts') === '1'; } catch (e) { return false; } }
    function signUp(sub) {
      var json = sub.toJSON();
      return self.api('/api/alerts', { endpoint: json.endpoint, p256dh: json.keys.p256dh, auth: json.keys.auth });
    }
    // Each time the page opens and reaches Locust: a sign-up the browser dropped (an iPhone's can lapse) is made again.
    function refreshAlerts() {
      if (alertsChecked || !ALERTS_KEY || !pushable() || Notification.permission !== 'granted') return;
      alertsChecked = true;
      navigator.serviceWorker.ready.then(function (reg) {
        return reg.pushManager.getSubscription().then(function (sub) {
          if (sub) return sub;
          return wantedAlerts() ? reg.pushManager.subscribe({ userVisibleOnly: true, applicationServerKey: lock.unb64(ALERTS_KEY) }) : null;
        });
      }).then(function (sub) {
        if (!sub) return;
        return signUp(sub).then(function () { alertsOn = true; rememberAlerts(true); changed(); });
      }).catch(function () { alertsChecked = false; });
    }

    /*
     * SCOUT WHILE YOUR COMPUTER IS OFF (0.829). With the PC on, this phone asks it once for the away key, kept here as
     * a key that cannot be read back out. With the PC off, it reads the newest letters from the relay and shows what
     * the away looks found: Scout's sentences, and links to the repository on GitHub, nowhere else.
     */
    function refreshAway() {
      if (awayAsked || !link) return;
      awayAsked = true;
      var current = link;
      self.api('/api/away').then(function (r) {
        if (link !== current) return;
        if (!r || r.off) { if (current.away) { delete current.away; away = null; changed(); return save(current); } return; }
        if (typeof r.key !== 'string' || !/^[A-Za-z0-9_-]{43}$/.test(r.key) || typeof r.repo !== 'string' || !/^[\\w.-]+\\/[\\w.-]+$/.test(r.repo)) return;
        return crypto.subtle.importKey('raw', lock.unb64(r.key), 'AES-GCM', false, ['decrypt']).then(function (key) {
          current.away = { key: key, repo: r.repo };
          return save(current);
        });
      }).catch(function () { awayAsked = false; });
    }
    function readLetters() {
      if (!link || !link.away) { away = null; return; }
      if (!socket || !ready) return;
      if (!away) away = { repo: link.away.repo, at: null, read: true, found: [] };
      socket.send(JSON.stringify({ t: 'letters', newest: 24, id: 'away' }));
    }
    function openLetters(items) {
      var current = link;
      if (!current || !current.away) return;
      var repo = current.away.repo, prefix = 'https://github.com/' + repo.toLowerCase() + '/';
      Promise.all(items.filter(function (item) { return item && item.from === 'away' && typeof item.n === 'string' && typeof item.c === 'string'; }).map(function (item) {
        return lock.open(current.away.key, item.n, item.c, 'locust-away/v1|' + current.room + '|away').then(function (letter) { return letter; }, function () { return null; });
      })).then(function (letters) {
        if (link !== current || pcOn) return;
        var looks = letters.filter(function (letter) { return letter && letter.v === 1 && letter.kind === 'look' && letter.repo === repo && typeof letter.at === 'string'; });
        looks.sort(function (a, b) { return a.at < b.at ? -1 : a.at > b.at ? 1 : 0; });
        var byKey = {}, order = [];
        looks.forEach(function (letter) {
          (Array.isArray(letter.found) ? letter.found : []).forEach(function (one) {
            if (!one || typeof one.key !== 'string' || typeof one.said !== 'string') return;
            var url = typeof one.url === 'string' && one.url.toLowerCase().indexOf(prefix) === 0 && /^https:\\/\\/github\\.com\\/[\\w./#?=&%-]+$/.test(one.url) ? one.url : null;
            if (!byKey[one.key]) order.push(one.key);
            byKey[one.key] = { said: String(one.said).slice(0, 400), lines: Array.isArray(one.lines) ? one.lines.filter(function (line) { return typeof line === 'string'; }).slice(0, 8) : [], url: url, at: letter.at, now: one.urgency === 'now' };
          });
        });
        var newest = looks[looks.length - 1];
        away = {
          repo: repo,
          at: newest ? newest.at : null,
          read: newest ? newest.read === true : true,
          found: order.map(function (key) { return byKey[key]; }).sort(function (a, b) { return a.at < b.at ? 1 : a.at > b.at ? -1 : 0; }).slice(0, 20)
        };
        changed();
      });
    }

    var self = {
      onSent: null,
      /** The relay answers and the computer does not: what is written now waits for it. */
      offline: function () { return !!link && ready && !!socket && !pcOn; },
      kept: function () { return link ? keptView : null; },
      keepState: function (state) {
        if (!link || !state || !Array.isArray(state.conversations)) return;
        keptView = { at: Date.now(), conversations: state.conversations.slice(0, 40).map(function (c) {
          return { missionId: c.missionId, title: cut(c.title, 200), agent: c.agent, at: c.at, snippet: cut(c.snippet, 300) };
        }), turns: keptView && keptView.turns ? keptView.turns : {} };
        writeKept();
      },
      keepTurns: function (ids, turns) {
        if (!link || !keptView || !Array.isArray(turns)) return;
        var shown = turns.slice(-20).map(function (turn) { return { missionId: turn.missionId, asked: cut(turn.asked, 2000), agent: turn.agent, answer: cut(turn.answer, 4000), at: turn.at, phase: turn.phase }; });
        // Newest last, so the oldest are the first let go.
        ids.forEach(function (id) { if (id) { delete keptView.turns[id]; keptView.turns[id] = shown; } });
        // The conversations read last, each kept under the ids it was opened by.
        var names = Object.keys(keptView.turns);
        if (names.length > 20) names.slice(0, names.length - 20).forEach(function (name) { delete keptView.turns[name]; });
        writeKept();
      },
      notes: function () { return link ? myNotes.slice() : []; },
      leave: function (missionId, title, text) {
        var current = link;
        if (!current) return Promise.reject(new Error('unpaired'));
        var nid = lock.b64(crypto.getRandomValues(new Uint8Array(16))), t = Date.now();
        return lock.seal(current.seal, { v: 1, kind: 'note', nid: nid, missionId: missionId, text: text, t: t }, lock.aad(current.room, current.device, 'device-to-pc')).then(function (box) {
          return noteAsk({ t: 'note', n: box.n, c: box.c });
        }).then(function (answer) {
          if (link !== current) return;
          myNotes.push({ note: answer.note, missionId: missionId, title: cut(title, 200), text: text, t: t, held: null });
          writeKept();
          changed();
        });
      },
      /** Taken back from the relay: 'gone' when Locust had already taken it. */
      takeBack: function (entry) {
        return noteAsk({ t: 'unnote', note: entry.note }).then(function () { return 'taken'; }, function (error) {
          if (error.code === 'note-gone') return 'gone';
          throw error;
        }).then(function (how) {
          myNotes = myNotes.filter(function (one) { return one.note !== entry.note; });
          writeKept();
          changed();
          return how;
        });
      },
      paired: function () { return link !== null; },
      away: function () { return link && link.away && !pcOn ? away : null; },
      pairNote: function () { return loaded ? note : 'Opening…'; },
      // A GitHub sign-in that did not let this browser in (0.838): its screen points back to the way in, not a camera.
      signedInFailed: function () { return signinFailed; },
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
          if (kept && kept.room && kept.seal && kept.open) { link = kept; connect(); readKept().then(function () { changed(); }); }
          changed();
        }, function () {
          loaded = true;
          note = 'This browser would not keep Locust’s keys. A private tab does not keep them: open the code in a normal one.';
          changed();
        });
        // A new code opened in this same tab changes only the #fragment, which reloads nothing.
        window.addEventListener('hashchange', function () { var next = readCode(); if (next) pair(next); });
        document.addEventListener('visibilitychange', function () {
          if (document.visibilityState === 'visible') { if (link && !socket) connect(); } else rest();
        });
        window.addEventListener('pagehide', rest);
        window.addEventListener('pageshow', function () { if (document.visibilityState === 'visible' && link && !socket) connect(); });
        if (ALERTS_KEY && pushable()) navigator.serviceWorker.register('/sw.js').catch(function () {});
      },
      alerts: function () {
        if (!ALERTS_KEY || !link) return 'none';
        if (!pushable()) return /iPhone|iPad/.test(navigator.userAgent || '') && !onHomeScreen() ? 'home-screen' : 'none';
        if (Notification.permission === 'denied') return 'denied';
        return alertsBusy ? 'busy' : alertsOn ? 'on' : 'off';
      },
      setAlerts: function (on) {
        if (alertsBusy) return Promise.resolve();
        alertsBusy = true; changed();
        var work = on
          ? Notification.requestPermission().then(function (permission) {
              if (permission !== 'granted') throw new Error(permission === 'denied' ? 'Alerts are blocked for this page in your browser’s settings.' : 'Alerts stay off until you allow them.');
              return navigator.serviceWorker.ready;
            }).then(function (reg) {
              return reg.pushManager.getSubscription().then(function (sub) { return sub || reg.pushManager.subscribe({ userVisibleOnly: true, applicationServerKey: lock.unb64(ALERTS_KEY) }); });
            }).then(signUp).then(function () { alertsOn = true; rememberAlerts(true); })
          : navigator.serviceWorker.ready.then(function (reg) { return reg.pushManager.getSubscription(); }).then(function (sub) {
              return sub ? sub.unsubscribe() : true;
            }).then(function () { return self.api('/api/alerts', { off: true }); }).then(function () { alertsOn = false; rememberAlerts(false); });
        return work.then(function () { alertsBusy = false; changed(); }, function (error) { alertsBusy = false; changed(); throw error; });
      }
    };
    return self;
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
    if (transport.signedInFailed && transport.signedInFailed()) {
      root.replaceChildren(el('div', { class: 'pair' }, [
        el('strong', { text: 'Locust did not open here' }),
        said ? el('div', { class: 'said', role: 'status', text: said }) : null,
        el('a', { class: 'note', href: '/desk/', text: 'Back to signing in' })
      ]));
      return;
    }
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
    if (state === null) {
      // The computer off (0.837): what this phone last saw, and what waits for it.
      var kept = transport.kept ? transport.kept() : null;
      var off = transport.offline && transport.offline();
      main.append(el('div', { class: 'note', 'data-off': off, text: off && kept
        ? 'Your computer is off. This is what your phone last saw, at ' + clock(kept.at) + '. A message you write now waits, and Locust sends it when your computer is back.'
        : transport.problem() || 'Reaching Locust…' }));
      var waits = notesSection(null);
      if (waits) main.append(waits);
      if (kept && kept.conversations.length > 0) main.append(conversationsSection(kept.conversations, true));
      var seen = awaySection();
      if (seen) main.append(seen);
    } else {
      var held = notesSection(null);
      if (held) main.append(held);
      if (state.waiting.length > 0) {
        var section = el('section', null, [el('h2', { text: 'Waiting on you' })]);
        var stack = el('div', { style: 'display:grid;gap:10px' });
        state.waiting.forEach(function (item) { stack.append(waitingCard(item)); });
        section.append(stack); main.append(section);
      }
      var scouted = scoutSection(state.scout || []);
      if (scouted) main.append(scouted);
      main.append(conversationsSection(state.conversations, false));
      var alerts = alertsSection();
      if (alerts) main.append(alerts);
      // From the relay (0.830): the whole window, for a device Settings lets have it.
      if (transport.away) main.append(el('section', { class: 'desk' }, [el('a', { class: 'more', href: '/desk/', text: 'Open Locust’s full window' })]));
    }
    parts.push(main);
    root.replaceChildren.apply(root, parts);
  }
  function clock(at) {
    var then = new Date(at), now = new Date();
    var time = then.toLocaleTimeString([], { hour: 'numeric', minute: '2-digit' });
    return then.toDateString() === now.toDateString() ? time : then.toLocaleDateString([], { weekday: 'short', month: 'short', day: 'numeric' }) + ', ' + time;
  }
  // Kept, a conversation's state is as it was then: no "working" it may no longer be doing.
  function conversationsSection(conversations, kept) {
    var convos = el('section', null, [el('h2', { text: 'Conversations' })]);
    if (conversations.length === 0) { convos.append(el('div', { class: 'empty', text: 'No conversations yet. Start one in Locust on your computer.' })); return convos; }
    var list = el('ul');
    conversations.forEach(function (c) {
      list.append(el('li', null, [el('button', { class: 'convo', type: 'button', on: { click: function () { open = c.missionId; openTitle = c.title; draft = ''; turns = null; render(); refresh(); } } }, [
        el('span', { class: 'title', text: c.title }),
        el('span', { class: 'meta', 'data-running': !kept && c.phase === 'running', text: kept ? c.agent + ' · ' + when(c.at) : c.agent + ' · ' + phaseWords(c.phase) + ' · ' + when(c.at) }),
        c.snippet ? el('span', { class: 'snippet', text: c.snippet }) : null
      ])]));
    });
    convos.append(list);
    return convos;
  }
  /*
   * WHAT WAITS FOR YOUR COMPUTER (0.837). Each message this phone left while the computer was off, until Locust sends
   * it: Take back while it waits; and once the computer has held one, the person decides here.
   */
  function noteCard(one, inThread) {
    var card = el('div', { class: 'card', 'data-note': one.held || 'waiting' }, [
      el('div', { class: 'who', text: 'You · ' + when(new Date(one.t).toISOString()) + (inThread || !one.title ? '' : ' · ' + one.title) }),
      el('div', { class: 'what', text: inThread ? one.text : (one.text.length > 300 ? one.text.slice(0, 300) + '…' : one.text) })
    ]);
    var words = one.held === 'old' ? 'Your computer was off for a long time, so Locust did not send this by itself. Send it now, or discard it.'
      : one.held === 'gone' ? 'Locust could not send this: that conversation is not in Locust any more.'
      : one.held === 'failed' ? 'Locust could not send this. Send it again, or discard it.'
      : 'Waiting for your computer. Locust sends it when your computer is back.';
    card.append(el('div', { class: 'snippet', text: words }));
    var row = el('div', { class: 'row' });
    var busy = function (on) { Array.prototype.forEach.call(row.querySelectorAll('button'), function (b) { b.disabled = on; }); };
    var discard = function (said) {
      busy(true);
      transport.takeBack(one).then(function (how) { toast(how === 'gone' ? 'Your computer already took that one.' : said); }, function (e) { busy(false); toast(e.message); });
    };
    if (one.held === 'old' || one.held === 'failed') {
      row.append(el('button', { class: 'act yes', type: 'button', text: 'Send now', on: { click: function () {
        busy(true);
        transport.api('/api/send', { missionId: one.missionId, text: one.text }).then(function (r) {
          if (!r.ok) { busy(false); toast(r.message || 'Locust could not send that.'); return; }
          return transport.takeBack(one).then(function () { toast('Sent.'); refresh(); });
        }).catch(function (e) { busy(false); if (e.message !== 'unpaired') toast(e.message); });
      } } }));
    }
    if (one.held === 'gone' && navigator.clipboard) {
      row.append(el('button', { class: 'act yes', type: 'button', text: 'Copy', on: { click: function () {
        navigator.clipboard.writeText(one.text).then(function () { toast('Copied.'); }, function () { toast('This browser would not copy it.'); });
      } } }));
    }
    row.append(el('button', { class: 'act no', type: 'button', text: one.held ? 'Discard' : 'Take back', on: { click: function () { discard(one.held ? 'Discarded.' : 'Taken back. Nothing was sent.'); } } }));
    card.append(row);
    return card;
  }
  // In a conversation, its notes are those left on any of its turns: it moves on to a newer turn once one is sent.
  function notesSection(missionIds) {
    var mine = transport.notes ? transport.notes() : [];
    var shown = mine.filter(function (one) { return missionIds ? missionIds.indexOf(one.missionId) >= 0 : true; });
    // With the computer on, only one it held needs the person; one still waiting is about to go.
    if (state !== null && !missionIds) shown = shown.filter(function (one) { return !!one.held; });
    if (shown.length === 0) return null;
    var stack = el('div', { style: 'display:grid;gap:10px' });
    shown.forEach(function (one) { stack.append(noteCard(one, !!missionIds)); });
    return el('section', { class: 'waits' }, [el('h2', { text: 'Waiting to send' }), stack]);
  }
  function scoutSection(cards) {
    if (cards.length === 0) return null;
    var stack = el('div', { style: 'display:grid;gap:10px' });
    cards.forEach(function (one) {
      var card = el('div', { class: 'card', 'data-waiting': one.now, 'data-scout': true }, [
        el('div', { class: 'who', text: 'Scout · ' + when(one.at) }),
        el('div', { class: 'what', text: one.said })
      ]);
      (one.lines || []).forEach(function (line) { card.append(el('div', { class: 'snippet', text: line })); });
      var row = el('div', { class: 'row' });
      if (one.url) row.append(el('a', { class: 'act yes', href: one.url, target: '_blank', rel: 'noopener noreferrer', text: 'Open on GitHub' }));
      var dismiss = el('button', { class: 'act no', type: 'button', text: 'Dismiss' });
      dismiss.addEventListener('click', function () {
        dismiss.disabled = true;
        transport.api('/api/scout/dismiss', { cardId: one.cardId }).then(function (r) {
          toast(r.ok ? 'Dismissed.' : 'That card was already gone.');
          refresh();
        }).catch(function (e) { dismiss.disabled = false; if (e.message !== 'unpaired') toast(e.message); });
      });
      row.append(dismiss);
      card.append(row);
      stack.append(card);
    });
    return el('section', null, [el('h2', { text: 'Scout found' }), stack]);
  }
  function awaySection() {
    var seen = transport.away ? transport.away() : null;
    if (!seen) return null;
    var parts = [el('h2', { text: 'While your computer is off' })];
    if (!seen.at) parts.push(el('div', { class: 'note', text: 'Scout looks at ' + seen.repo + ' on GitHub while your computer is off. Nothing has come from it yet.' }));
    else {
      parts.push(el('div', { class: 'note', text: 'Scout looked at ' + seen.repo + ' on GitHub ' + when(seen.at) + (seen.read ? '.' : ', but GitHub could not be read.') }));
      if (seen.found.length === 0) parts.push(el('div', { class: 'empty', text: 'Nothing new in its last looks.' }));
      else {
        var stack = el('div', { style: 'display:grid;gap:10px' });
        seen.found.forEach(function (one) {
          var card = el('div', { class: 'card', 'data-waiting': one.now }, [
            el('div', { class: 'who', text: 'Scout · ' + when(one.at) }),
            el('div', { class: 'what', text: one.said })
          ]);
          one.lines.forEach(function (line) { card.append(el('div', { class: 'snippet', text: line })); });
          if (one.url) card.append(el('div', { class: 'row' }, [el('a', { class: 'act yes', href: one.url, target: '_blank', rel: 'noopener noreferrer', text: 'Open on GitHub' })]));
          stack.append(card);
        });
        parts.push(stack);
      }
    }
    return el('section', { class: 'away' }, parts);
  }
  function alertsSection() {
    var state = transport.alerts ? transport.alerts() : 'none';
    if (state === 'none') return null;
    var words = state === 'on' ? 'On. This phone buzzes when something needs you while you are away from your computer.'
      : state === 'off' ? 'Get a buzz when something needs you while you are away from your computer.'
      : state === 'busy' ? 'One moment…'
      : state === 'denied' ? 'Alerts are blocked for this page in your browser’s settings.'
      : 'To get alerts on this phone, add Locust to your Home Screen: tap Share, then Add to Home Screen, and open it from there.';
    var parts = [el('h2', { text: 'Alerts' }), el('div', { class: 'note', text: words })];
    if (state === 'on' || state === 'off') {
      parts.push(el('div', { class: 'row' }, [el('button', { class: 'act ' + (state === 'off' ? 'yes' : 'no'), type: 'button', text: state === 'off' ? 'Turn on alerts' : 'Turn off alerts', on: { click: function () {
        transport.setAlerts(state === 'off').then(function () { toast(state === 'off' ? 'Alerts are on.' : 'Alerts are off.'); }, function (e) { toast(e.message); });
      } } })]));
    }
    return el('section', { class: 'alerts' }, parts);
  }
  var turns = null;
  function threadScreen() {
    var main = el('main');
    var list = el('div', { class: 'turns' });
    var off = !!(transport.offline && transport.offline());
    var kept = turns === null && transport.kept ? transport.kept() : null;
    var shown = turns !== null ? turns : kept && kept.turns && kept.turns[open] ? kept.turns[open] : null;
    if (turns === null && shown !== null) list.append(el('div', { class: 'note', 'data-off': off, text: (off ? 'Your computer is off. ' : '') + 'This is the conversation as your phone last saw it, at ' + clock(kept.at) + '.' }));
    if (shown === null) list.append(el('div', { class: 'note', text: transport.problem() || 'Opening…' }));
    else shown.forEach(function (turn) {
      if (turn.asked) list.append(el('div', { class: 'asked', text: turn.asked }));
      var said = el('div', null, [el('div', { class: 'by', text: turn.agent })]);
      if (turn.answer) said.append(el('div', { class: 'answer', text: turn.answer }));
      if (turn.phase === 'running' && turns !== null) said.append(el('div', { class: 'working', text: 'Working…' }));
      else if (!turn.answer) said.append(el('div', { class: 'note', text: turns === null ? 'No answer had come when your phone last saw it.' : 'This turn ' + phaseWords(turn.phase) + ' without an answer.' }));
      list.append(said);
    });
    var waiting = notesSection([open].concat((shown || []).map(function (turn) { return turn.missionId; })));
    if (waiting) list.append(waiting);
    main.append(list);
    var box = el('textarea', { rows: '1', placeholder: off ? 'Leave a message for later' : 'Send the next message', 'aria-label': 'Message' });
    box.value = draft;
    box.addEventListener('input', function () { draft = box.value; box.style.height = 'auto'; box.style.height = box.scrollHeight + 'px'; });
    var send = el('button', { type: 'submit', text: off ? 'Leave' : 'Send' });
    var running = !off && turns !== null && turns.length > 0 && turns[turns.length - 1].phase === 'running';
    send.disabled = running;
    if (running) box.placeholder = 'Wait for this turn to finish';
    var form = el('form', { class: 'compose', on: { submit: function (event) {
      event.preventDefault();
      var text = box.value.trim(); if (!text) return;
      // The computer off (0.837): the message waits at the relay, sealed, and Locust sends it when it is back.
      if (off) {
        if (text.length > 8000) { toast('That message is too long to wait. Keep it under 8,000 characters.'); return; }
        send.disabled = true;
        transport.leave(open, openTitle, text).then(function () { draft = ''; toast('Waiting for your computer.'); render(); }, function (e) { send.disabled = false; if (e.message !== 'unpaired') toast(e.message); });
        return;
      }
      if (!turns || turns.length === 0) return;
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
          if (transport.keepTurns) transport.keepTurns([asked, open], turns);
          var atEnd = window.innerHeight + window.scrollY >= document.body.scrollHeight - 40;
          render();
          if (turns.length !== before || atEnd) window.scrollTo(0, document.body.scrollHeight);
        })
      : transport.api('/api/state').then(function (r) { state = r; if (transport.keepState) transport.keepState(r); render(); });
    work.catch(function (e) {
      if (e.message === 'unpaired') return;
      // Seen again, the line is still opening: what was on screen stays until it answers (0.827).
      if (e.message === 'Reaching Locust…') return;
      if (!asked) { state = null; render(); }
      // The computer went off with a conversation open: it shows as the phone last saw it, with a way to leave a message.
      else if (open === asked && turns !== null && transport.offline && transport.offline()) { turns = null; render(); }
    }).then(function () {
      if (transport.paired() && document.visibilityState === 'visible') timer = setTimeout(refresh, 3000);
    });
  }
  document.addEventListener('visibilitychange', function () { if (document.visibilityState === 'visible') refresh(); });
  // A tapped alert (PHONE_SERVICE_WORKER) opens the list, where what it was about waits.
  if ('serviceWorker' in navigator) navigator.serviceWorker.addEventListener('message', function (event) {
    if (event.data && event.data.locust === 'alert') { open = null; turns = null; render(); refresh(); }
  });
  render();
  transport.onSent = function (count) {
    toast(count === 1 ? 'Your computer is back, and Locust sent the message you left.' : 'Your computer is back, and Locust sent the ' + count + ' messages you left.');
  };
  transport.start(function (reset) {
    if (reset) { open = null; turns = null; state = null; }
    render(); refresh();
  });
`

/**
 * The page: on the home network (`lan`, served by the PC) or from Locust's relay (`relay`). From the relay it can be a
 * home-screen app with alerts (0.827), given the public half of the key the relay signs pushes with.
 */
export function phonePage(options: { readonly via?: 'lan' | 'relay'; readonly alertsKey?: string } = {}): string {
  const relay = options.via === 'relay'
  const alertsKey = relay && options.alertsKey !== undefined && /^[A-Za-z0-9_-]{87}$/.test(options.alertsKey) ? options.alertsKey : ''
  const script = relay ? `(function () {var ALERTS_KEY = '${alertsKey}';${RELAY_LOCK}${RELAY}${UI}})();` : `(function () {${LAN}${UI}})();`
  const app = relay ? '\n<link rel="manifest" href="/manifest.webmanifest">\n<link rel="apple-touch-icon" href="/icon-256.png">\n<meta name="apple-mobile-web-app-capable" content="yes">\n<meta name="apple-mobile-web-app-title" content="Locust">' : ''
  return `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover">
<meta name="theme-color" content="#0b0d0e">
<meta name="referrer" content="no-referrer">
<title>Locust</title>${app}
<style>${STYLE}</style>
</head>
<body>
<div id="root"></div>
<script>${script}</script>
</body>
</html>`
}

/**
 * The phone's service worker (0.827): it shows an alert the browser's push service brought, and a tap opens the page.
 * What it shows is what the PC sealed: who and what kind, never a command or a message's words (phone-alerts.ts).
 */
export const PHONE_SERVICE_WORKER = `self.addEventListener('push', function (event) {
  var said = {};
  try { said = event.data ? event.data.json() : {}; } catch (e) {}
  var title = typeof said.title === 'string' && said.title ? said.title.slice(0, 120) : 'Locust';
  var body = typeof said.body === 'string' ? said.body.slice(0, 300) : 'Something needs you.';
  var tag = typeof said.tag === 'string' && said.tag ? said.tag.slice(0, 60) : 'locust';
  event.waitUntil(self.registration.showNotification(title, { body: body, tag: tag, renotify: true, icon: '/icon-256.png', badge: '/icon-256.png' }));
});
self.addEventListener('notificationclick', function (event) {
  event.notification.close();
  event.waitUntil(self.clients.matchAll({ type: 'window', includeUncontrolled: true }).then(function (open) {
    // An alert is about something waiting on you: the page goes to the list, where it waits (0.827).
    for (var i = 0; i < open.length; i += 1) if ('focus' in open[i]) { open[i].postMessage({ locust: 'alert' }); return open[i].focus(); }
    return self.clients.openWindow('/');
  }));
});
`

/** What makes the page a home-screen app (0.827). */
export function phoneManifest(): string {
  return JSON.stringify({
    name: 'Locust',
    short_name: 'Locust',
    start_url: '/',
    scope: '/',
    display: 'standalone',
    background_color: '#0b0d0e',
    theme_color: '#0b0d0e',
    icons: [{ src: '/icon-256.png', sizes: '256x256', type: 'image/png' }, { src: '/icon-512.png', sizes: '512x512', type: 'image/png', purpose: 'any' }]
  })
}
