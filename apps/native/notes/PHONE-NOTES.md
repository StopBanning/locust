# Phone on the same Wi-Fi, part 1 (18 Nov 2026)

Branch `exec/native-phone`, from 19a93865. Area 12 of `parity/rows.tsv`. This round covers only the Wi-Fi half; the
"anywhere" relay is not part of it (see "Not this round").

## Section 1: pairing and the page's rules

`core/src/phone_host.rs` (from `main/phone-host.ts`, `main/phone-scout-cards.ts`, `main/ledger-writes.ts`) and
`core/src/phone_page.rs` (from `main/phone-page.ts`), with `phone_host_tests.rs` and `phone_page_tests.rs`.

- **The pairing key**: 32 random bytes as unpadded base64url (43 characters), sealed in the `SecretBox`
  (`own_models::SecretBox`, the same one `teammate_keys.rs` uses). `phone.json` (written `0o600`, temp file then
  rename) holds only `enabled` and the sealed key, never the key in plain text. `PairingKey` has no `Debug` that shows
  it, and `scrub_phone_error` takes the key out of any error before it is shown. Tests use a fake secret box; no test
  touches the keychain.
- **Who may do what**: anyone may load the page. Every other path needs the key (compared by bytes with
  `same_key`, so a key that has the right number of characters but the wrong number of bytes is refused and counted as
  wrong). Writes are JSON only. A `Host` header that names the computer by a name, as a DNS-rebinding page would, is
  refused (`valid_host_header`). After `MAX_WRONG` = 20 wrong keys within `WRONG_WINDOW_MS` = 10 minutes, the host
  stops answering that address. Request bodies are capped at 32 KiB and messages at 20,000 characters.
- **On and off**: the host is off until turned on. Off means no listener and no address at all, and it unpairs every
  phone. A new code unpairs every phone while staying on. The key survives a restart, so a paired phone stays paired.
  If the app refuses (`refusal`), a saved opt-in is ignored and no listener or key is made.
- **Addresses**: only IPv4 addresses that are not internal, leaving out virtual switches a phone cannot reach
  (vEthernet, VirtualBox, VMware, Hyper-V, WSL, Docker, loopback).
- **What the page shows**: conversations (one row per conversation, newest first, at most 20, named by the first
  thing asked and by the teammate who answered); turns (at most 8, answers capped at 8,000 characters, the person's
  words only where the person typed them, a streamed answer read whole or as still arriving); and Scout cards (in
  Locust's words, newest first, at most 10, a GitHub link only, and nothing the window keeps). Dismissing a card works
  only for an id that is a card's id.
- **Reading history once per change**: `LedgerWriteCounter` counts a write when it is asked for and again when it
  lands, and never counts a read. `KeyedOnce` builds once per key, shares a build that is still running (it holds its
  lock through the build), and tries again after a failed build.
- **The page**: `phone_page(via, ..)` returns Electron's page for `Lan` and `Relay`. The answer renderer
  (`PHONE_ANSWER`) draws Markdown with DOM nodes, never HTML. The five page tests that run the renderer use `node`, as
  `native/tests.rs` does, and **skip where `node` is missing**. They ran (and caught a break) here.
- The in-flight `KeyedOnce` test gives the second ask 50 ms to reach the lock while the first is building. On a very
  loaded machine it can arrive late and still pass, so the break can slip by; it can never fail falsely. Here the
  break was caught.
- `parity/rows.tsv` area 12 stays `missing` until a person can pair a phone from the window (Sections 3 and 4).
- The listener sits behind the `PhoneListenerFactory`/`PhoneBound` traits; tests use a fake and never bind a port. The
  real `std::net` listener is Section 3.

### Tests (all passed by name, `nextest -E 'test(/phone_/)'`, 18 Nov 2026)

`phone_host::tests::`: `is_off_until_turned_on_and_off_means_no_address_at_all`,
`serves_the_page_to_anyone_and_nothing_else_without_the_key_from_the_code`,
`refuses_a_request_that_names_this_computer_by_a_name_as_a_rebinding_page_would`,
`answers_a_card_and_sends_a_message_only_as_json_with_the_key`,
`stops_listening_to_an_address_after_too_many_wrong_keys`,
`answers_a_key_of_the_right_length_in_characters_but_not_in_bytes_and_counts_it_as_wrong`,
`keeps_its_key_across_a_restart_so_a_paired_phone_stays_paired`,
`off_unpairs_every_phone_and_a_new_code_does_it_while_staying_on`,
`offers_home_network_addresses_never_a_virtual_switch_a_phone_cannot_reach`,
`ignores_a_saved_phone_opt_in_and_refuses_forged_pairing_without_a_listener_or_key`,
`pairing_key_is_sealed_in_secret_box_never_plaintext_and_scrubbed_from_errors`,
`scout_cards_in_locusts_words_newest_first_with_github_link_only_and_nothing_the_window_keeps`,
`dismisses_a_scout_card_the_phone_names_and_nothing_that_is_not_a_cards_id`,
`conversations_are_one_row_per_conversation_newest_first_named_by_the_first_thing_asked`,
`conversations_name_the_teammate_who_answered_as_the_window_does`,
`turns_show_each_turns_answer_and_the_persons_words_only_where_the_person_typed_them`,
`reads_a_streamed_answer_whole_and_one_still_arriving`,
`ledger_write_counter_counts_writes_before_and_after_and_never_reads`,
`keyed_once_builds_once_per_key_shares_a_build_in_flight_and_retries_after_a_failure`.

`phone_page::tests::`: `draws_the_bullets_kit_sent_the_phone_as_a_list_with_bold_names`,
`draws_paragraphs_a_heading_a_numbered_list_code_and_emphasis`,
`carries_an_item_on_over_a_line_indented_under_it_and_shows_an_unclosed_fence_as_code`,
`keeps_every_word_from_a_run_as_text_never_as_html_and_a_link_as_its_words`,
`gives_the_list_of_conversations_the_words_without_the_marks`,
`is_in_the_page_both_ways_drawing_the_thread_and_the_snippets`.

### Breaks (`~/work/scratch/mut6.py`, logs in `~/work/scratch/phone/sec1_breaks.log`, `sec1b_breaks.log`)

| Break | Caught by |
| --- | --- |
| skip `valid_host_header` | `refuses_a_request_that_names_this_computer_by_a_name_as_a_rebinding_page_would` |
| never stop after `MAX_WRONG` | `stops_listening_to_an_address_after_too_many_wrong_keys` |
| every key is right (`authed = true`) | `serves_the_page_to_anyone_..`, `answers_a_key_of_the_right_length_..`, `off_unpairs_every_phone_..` |
| let virtual switches through | `offers_home_network_addresses_never_a_virtual_switch_a_phone_cannot_reach` |
| ignore `refusal` | `ignores_a_saved_phone_opt_in_and_refuses_forged_pairing_without_a_listener_or_key` |
| bold drawn as `em` in the page script | `draws_paragraphs_..`, `draws_the_bullets_..`, `keeps_every_word_..` |
| `KeyedOnce` lets go of its lock while building | `keyed_once_builds_once_per_key_shares_a_build_in_flight_and_retries_after_a_failure` |

## Not this round

- **The anywhere relay** (`main/phone-anywhere.ts`, `main/relay-crypto.ts`): needs https and AES-GCM. `ureq` is built
  without TLS and `Cargo.lock` has no AES crate, and the handoff allows no new crate without Colin's yes.
  `a-phone-pairs-and-asks-through-the-relay` and `your-locust-ask-reaches-your-phone-while-the-pc-is-off` stay
  `waiting`.
- **Web Push and the sealed offline queue** (`main/web-push.ts`): `an-alert-is-locked-for-its-phone-alone` is the RFC
  8291 encryption (aes128gcm, ECDH P-256), which needs the same missing crates, so it stays `waiting`.
- A live check from a real phone on the home Wi-Fi needs Colin.
