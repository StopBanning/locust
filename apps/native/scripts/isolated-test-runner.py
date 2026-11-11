#!/usr/bin/env python3
"""Run Cargo executables with disposable data paths before any test executes.

Also usable for synthetic CLI checks and headless snapshots. No real turns.
"""
import os
import pathlib
import subprocess
import sys
import tempfile

with tempfile.TemporaryDirectory(prefix='p11-', dir='/tmp') as directory:
    root = pathlib.Path(directory).resolve()
    env = dict(os.environ)
    for name, suffix in [('HOME', 'home'), ('LOCUST_NATIVE_DATA', 'native'),
                         ('XDG_CONFIG_HOME', 'config'), ('APPDATA', 'appdata'),
                         ('CODEX_HOME', 'codex'), ('TMPDIR', 'tmp')]:
        path = root / suffix
        path.mkdir()
        env[name] = str(path)
    env['LOCUST_TEST_ROOT'] = str(root)
    env.pop('LOCUST_MCP_CONNECTION', None)
    for name in list(env):
        if name.startswith('LOCUST_REAL_'):
            raise SystemExit('Isolation guard refuses real-data test opt-ins: ' + name)
    command = sys.argv[1:]
    result = subprocess.run(command, env=env)
    raise SystemExit(result.returncode)
