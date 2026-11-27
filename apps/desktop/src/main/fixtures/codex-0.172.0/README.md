# Codex 0.172.0 approval response schemas

Copied unchanged from Codex CLI 0.172.0's own generated schemas:

    npx -y @openai/codex@0.172.0 app-server generate-json-schema --out <dir>

The v2 replies (`item/commandExecution/requestApproval`, `item/fileChange/requestApproval`) and the legacy ones
(`execCommandApproval`, `applyPatchApproval`). `a-codex-deny-is-in-codexs-schema.test.ts` checks every decision Locust
can send Codex against them, so a Codex that changes its decisions fails a test instead of a person's turn. The Rust
port keeps the same copies in `apps/native/core/tests/fixtures/codex-0.172.0/`. When Codex moves, regenerate both into
folders named for that version and point the tests at them.
