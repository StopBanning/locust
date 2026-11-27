# Codex 0.172.0 approval response schemas

Copied unchanged from Codex CLI 0.172.0's own generated schemas:

    npx -y @openai/codex@0.172.0 app-server generate-json-schema --out <dir>

The v2 replies (`item/commandExecution/requestApproval`, `item/fileChange/requestApproval`) and the legacy ones
(`execCommandApproval`, `applyPatchApproval`). `codex_decision_schema_tests.rs` checks every decision Locust can send
Codex against them, so a Codex that changes its decisions fails a test instead of a person's turn. When Codex moves,
regenerate them into a new folder named for that version and point the test at it.
