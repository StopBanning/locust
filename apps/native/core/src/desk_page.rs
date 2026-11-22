//! LOCUST IN A BROWSER: THE WAY IN (Electron `main/desk-page.ts`, 0.830 / 0.838).
//!
//! Serves `/desk/` (the browser entry page with GitHub sign-in where offered, pairing code
//! guidance, and download link) and scriptless escaped note pages for GitHub sign-in outcomes.

use crate::phone_page::RELAY_LOCK;

const STYLE: &str = r####"
:root { color-scheme: dark; --bg: #0b0d0e; --text: #f2efe9; --muted: #8b9190; --button: #f2efe9; --button-text: #0b0d0e; }
* { box-sizing: border-box; }
html, body { margin: 0; background: var(--bg); color: var(--text); }
body { font: 16px/1.5 system-ui, -apple-system, "Segoe UI", Roboto, sans-serif; min-height: 100dvh; display: grid; place-items: center; padding: 24px; }
main { max-width: 460px; display: grid; gap: 14px; text-align: center; }
h1 { margin: 0; font-size: 20px; font-weight: 600; }
p { margin: 0; color: var(--muted); }
button, a.go { justify-self: center; display: inline-grid; place-items: center; min-height: 44px; padding: 0 18px; border-radius: 10px; border: 0; background: var(--button); color: var(--button-text); font: 600 15px system-ui, sans-serif; cursor: pointer; text-decoration: none; }
a.go.quiet { background: transparent; color: var(--text); box-shadow: inset 0 0 0 1px #2c3133; }
button:focus-visible, a:focus-visible { outline: 2px solid var(--text); outline-offset: 3px; }
.ways { display: grid; gap: 10px; justify-items: center; }
.or { font-size: 14px; }
.new { margin-top: 18px; padding-top: 18px; border-top: 1px solid #22272a; display: grid; gap: 8px; font-size: 14px; }
.new a { color: var(--text); }
"####;

const SCRIPT: &str = r####"
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
    return main;
  }
  function line(parent, tag, className, text) {
    var el = document.createElement(tag); if (className) el.className = className; if (text) el.textContent = text;
    parent.appendChild(el); return el;
  }
  function pairFirst() {
    var main = show('Open Locust in this browser', 'Locust runs on your computer. This page opens its whole window from anywhere, while your computer is on and Locust is open there.');
    var ways = line(main, 'div', 'ways');
    if (GITHUB) {
      var go = line(ways, 'a', 'go', 'Sign in with GitHub'); go.href = '/v1/github/start';
      line(ways, 'p', 'or', 'Works once Locust on your computer is signed in with GitHub too: Settings › General › Your phone › Sign in with GitHub.');
    }
    line(ways, 'p', 'or', (GITHUB ? 'Or pair it with a code. ' : '') + 'In Locust on your computer, open Settings › General › Your phone. Open in your browser opens it there; on another computer, open the code’s link here.');
    var fresh = line(main, 'div', 'new');
    line(fresh, 'p', '', 'New to Locust? It runs AI teammates on your own computer, with the AI tools you already use.');
    var get = line(fresh, 'a', '', 'Download Locust'); get.href = 'https://locust.lol';
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
      if (!info || typeof info.version !== 'string' || !/^\d+\.\d+\.\d+$/.test(info.version)) { show('Locust cannot open here', 'Locust on your computer gave no version this page knows.'); return; }
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
"####;

/// The way in, as the relay serves it at `/desk/`; `github` when this relay offers signing in with GitHub (0.838).
pub fn desk_page(github: bool) -> String {
    let flag = if github { "true" } else { "false" };
    format!(
        "<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n<meta name=\"theme-color\" content=\"#0b0d0e\">\n<meta name=\"referrer\" content=\"no-referrer\">\n<title>Locust</title>\n<style>{STYLE}</style>\n</head>\n<body>\n<div id=\"root\"></div>\n<script>(function () {{var GITHUB = {flag};{RELAY_LOCK}{SCRIPT}}})();</script>\n</body>\n</html>"
    )
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeskNoteLink {
    pub label: String,
    pub href: String,
}

fn escape_html(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '&' => out.push_str("&#38;"),
            '<' => out.push_str("&#60;"),
            '>' => out.push_str("&#62;"),
            '"' => out.push_str("&#34;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(ch),
        }
    }
    out
}

/// A short page in the way in's own look, with no script: what GitHub's sign-in ended in (0.838).
pub fn desk_note_page(title: &str, words: &str, links: &[DeskNoteLink]) -> String {
    let rendered_links: String = links
        .iter()
        .enumerate()
        .map(|(at, link)| {
            let extra = if at == 0 { "" } else { " quiet" };
            format!(
                "<a class=\"go{extra}\" href=\"{}\">{}</a>",
                escape_html(&link.href),
                escape_html(&link.label)
            )
        })
        .collect();
    let ways = if rendered_links.is_empty() {
        String::new()
    } else {
        format!("<div class=\"ways\">{rendered_links}</div>")
    };
    format!(
        "<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n<meta name=\"theme-color\" content=\"#0b0d0e\">\n<meta name=\"referrer\" content=\"no-referrer\">\n<title>Locust</title>\n<style>{STYLE}</style>\n</head>\n<body>\n<main><h1>{}</h1><p>{}</p>{ways}</main>\n</body>\n</html>",
        escape_html(title),
        escape_html(words)
    )
}
