#!/usr/bin/env python3
"""Run one renderer regression against the workspace, restoring source even on failure.

Run from apps/native: python3 scripts/fixtures/renderer-mutation.py name
No filtered cargo runs. A compiler error does not count as catching a mutation.
"""
import os
from pathlib import Path
import subprocess
import sys

CONTROLS = {
    "anonymous-title": (
        "src/ui/decision_card.rs",
        'None => "A decision is needed to continue".into(),',
        'None => "The runtime needs a decision to continue".into(),',
        "says_a_decision_is_needed_and_no_word_for_the_machinery_when_nobody_asked_by_name",
    ),
    "free-name": (
        "core/src/people.rs",
        'EXAMPLE_NAMES.into_iter().find(|name| !taken.iter().any(|t| t.trim().to_lowercase() == name.to_lowercase())).unwrap_or("Wren")',
        'let _ = taken; "Wren"',
        "skips_every_name_already_on_the_team_whatever_its_case",
    ),
    "fresh-colour": (
        "core/src/people.rs",
        'hues.into_iter().find(|h| !taken.contains(h)).unwrap_or(hues[taken.len() % hues.len()])',
        'hues[taken.len() % hues.len()]',
        "is_the_first_one_nobody_wears_then_round_again",
    ),
}
path, before, after, named = CONTROLS[sys.argv[1]]
path = Path(path)
original = path.read_bytes()
source = original.decode()
assert source.count(before) == 1, "mutation must match exactly once"
log = Path("notes") / f"renderer-control-{sys.argv[1]}.log"
env = dict(os.environ, LOCUST_TEST_TS_READER=str(Path("dist/thread-7-ts-reader/index.js").resolve()))
try:
    path.write_text(source.replace(before, after, 1))
    with log.open("w") as out:
        result = subprocess.run(["timeout", "5400", "python3", "scripts/gentle-cargo.py", "--timeout", "5380", "test", "--release", "--workspace", "--no-fail-fast"], stdout=out, stderr=subprocess.STDOUT, env=env)
    output = log.read_text()
    assert result.returncode != 0, "mutation survived"
    assert any(line.startswith("test ") and named in line and line.endswith(" ... FAILED") for line in output.splitlines()), f"named assertion failure absent: {log}"
    print(f"CAUGHT {sys.argv[1]}: {named}", flush=True)
finally:
    path.write_bytes(original)
    assert path.read_bytes() == original
    print(f"RESTORED {path}", flush=True)
