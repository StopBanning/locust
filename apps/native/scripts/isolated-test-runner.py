#!/usr/bin/env python3
"""Run Cargo executables with disposable data paths before any test executes.

Also usable for synthetic CLI checks and headless snapshots. No real turns.
"""
import os
import pathlib
import subprocess
import sys
import tempfile

with tempfile.TemporaryDirectory(prefix='locust-native-', dir='/tmp') as directory:
    root = pathlib.Path(directory).resolve()
    env = dict(os.environ)
    # A real-agent test (LOCUST_REAL_*, run only when asked with --ignored) needs the agents' own logins, which
    # live under the real HOME / CODEX_HOME: those stay. Locust's own data still goes to the disposable folder,
    # and those tests build their stores under it, so neither the real native nor Electron folder is touched.
    real = any(name.startswith('LOCUST_REAL_') for name in env)
    paths = [('LOCUST_NATIVE_DATA', 'native'), ('APPDATA', 'appdata'), ('TMPDIR', 'tmp')]
    if not real:
        paths += [('HOME', 'home'), ('XDG_CONFIG_HOME', 'config'), ('CODEX_HOME', 'codex')]
    for name, suffix in paths:
        path = root / suffix
        path.mkdir()
        env[name] = str(path)
    env['LOCUST_TEST_ROOT'] = str(root)
    env.pop('LOCUST_MCP_CONNECTION', None)
    command = sys.argv[1:]
    result = subprocess.run(command, env=env)
    raise SystemExit(result.returncode)
