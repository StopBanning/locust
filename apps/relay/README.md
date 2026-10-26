# Locust's relay

Lets a person's phone reach Locust on their computer from anywhere. The computer keeps one WebSocket open to the
relay, and a phone paired by its code connects there too; the relay passes messages between them. Every message is
sealed on one of the person's devices and opened on the other (an X25519 exchange, then AES-256-GCM, a key for each
direction), so the relay passes along what it cannot read. It keeps only hashes of the tokens it lets in.

It also serves the page the phone opens: the same page Locust serves on the home network
(`apps/desktop/src/main/phone-page.ts`), asking through the relay instead.

- `src/room-core.ts` holds the rules: who may connect to a computer's room, and what each connection may send. Its
  tests are in `test/`, and Locust's own end-to-end test runs them too
  (`apps/desktop/src/main/a-phone-pairs-and-asks-through-the-relay.test.ts`).
- `src/room.ts` is the Durable Object that is one computer's room, using Cloudflare's WebSocket Hibernation API, so a
  quiet room costs nothing.
- `src/worker.ts` serves the page at `/` and the rooms at `/v1/room/<id>`.

## Scout's away looks

While the computer is off, a scheduled workflow in the repository Scout watches looks for it
(`apps/desktop/src/main/scout-away-look.ts`) and posts to `/v1/room/<id>/away` with a token the computer opened
for it. It leaves a letter sealed with a key the relay never has; the computer and its phones read the letters when
they come, and the relay keeps them for a week at most. A letter may ring the phones that are not open, with alerts
their computer sealed for each browser ahead of time, at most five times a day. The computer turns this off with
`away-close`, which deletes its sealed letters and leaves called-link letters alone.

## Called links

A routine set to **When a link is called** uses the same room. Its computer makes a password and sends only its
SHA-256 hash with `hook-open`; `hook-close` retires it. `POST /v1/room/<room>/hook/<password>` accepts a readable body
and content type and returns `202` without waiting for the computer. Unlike phone messages, these bodies are **not
end-to-end encrypted**. A link is a password: anyone who has it can start the routine. Do not send secrets.

Only the PC receives these letters, live or through `hook-letters` on reconnection. It acknowledges a letter with
`hook-collected` after saving it locally; then the relay deletes it. The existing 200-letter cap is shared with
Scout's letters, and nothing is kept for more than a week. Expired or displaced calls leave a count for the routine's
history. A hook accepts at most 60 calls an hour, across password changes. Other methods get `405`, oversized bodies
`413`, excess calls `429`, and wrong, unknown or retired passwords the same `404`.

The PC folds waiting calls into one Ask run, with their count and the newest body's bounded data block. Hook-triggered
steps cannot use a routine's writing mode, run its folder check or start a writable copy. No new listener runs on the
PC. A relay upgrade is needed before this desktop feature can confirm a working link; existing rooms need no storage
migration. Keep HTTP access logs and tracing from recording full hook URLs: their path holds a password.

## Run your own

```
npx wrangler deploy
```

from this folder, with the route in `wrangler.jsonc` changed to a domain of yours. Then start Locust with
`LOCUST_RELAY` set to its address (`https://` and the host). To try it on one computer:

```
npx wrangler dev --port 8790 --ip 127.0.0.1 --local
```

and `LOCUST_RELAY=http://127.0.0.1:8790`.
