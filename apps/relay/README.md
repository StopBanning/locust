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
