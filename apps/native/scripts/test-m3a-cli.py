#!/usr/bin/env python3
"""Synthetic Claude CLI smoke. Usage: python3 scripts/test-m3a-cli.py <locust-native binary>"""
import json, os, pathlib, shlex, signal, subprocess, sys, tempfile, time
root = pathlib.Path(__file__).resolve().parent.parent
binary = pathlib.Path(sys.argv[1]).resolve()
fixture = root / 'scripts/fixtures/m3-claude.py'
with tempfile.TemporaryDirectory(prefix='.m3a-cli-', dir=root) as tmp:
    tmp = pathlib.Path(tmp)
    fake = tmp / 'claude'
    # Shim: pass Ask-mode argv through to the fixture as CLAUDE_ARGV, scenario from env.
    fake.write_text(
        '#!/bin/sh\n'
        'ARGS_FILE="${TMPDIR:-/tmp}/m3-claude-argv-$$"\n'
        ': > "$ARGS_FILE"\n'
        'for a in "$@"; do printf "%s\\n" "$a" >> "$ARGS_FILE"; done\n'
        'export CLAUDE_ARGV_FILE="$ARGS_FILE"\n'
        'exec python3 ' + shlex.quote(str(fixture)) + '\n'
    )
    fake.chmod(0o700)
    for scenario, expected in [
        ('success', 0),
        ('failure', 1),
        ('lost', 1),
        ('interrupt', 1),
        ('limit', 0),
        ('tools', 0),
        ('thinking', 0),
        ('approval', 0),
    ]:
        data = tmp / scenario
        env = dict(
            os.environ,
            PATH=str(tmp) + os.pathsep + os.environ['PATH'],
            LOCUST_NATIVE_DATA=str(data),
            M3_SCENARIO=scenario,
            M3_PID_FILE=str(tmp / 'server.pid'),
        )
        child = subprocess.Popen(
            [str(binary), '--run', 'claude', 'Synthetic prompt', '--folder', str(root), '--model', 'synthetic-model'],
            env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True,
        )
        prefix = ''
        if scenario == 'success':
            prefix = child.stdout.read(6)
            assert prefix == 'Hello ' and child.poll() is None, 'Reply did not stream before completion'
        if scenario == 'interrupt':
            deadline = time.monotonic() + 10
            while True:
                files = list(data.glob('mission-ledger/*.jsonl'))
                if files and 'run.started' in files[0].read_text():
                    break
                assert time.monotonic() < deadline, 'Turn did not start'
                time.sleep(.02)
            cancelled_at = time.monotonic()
            child.send_signal(signal.SIGINT)
        out, err = child.communicate(timeout=15)
        if scenario == 'interrupt':
            assert time.monotonic() - cancelled_at < 6, 'Cancellation exceeded five seconds plus margin'
        out = prefix + out
        assert child.returncode == expected, (scenario, child.returncode, err, out)
        records = [json.loads(line) for line in next(data.glob('mission-ledger/*.jsonl')).read_text().splitlines()]
        expected_type = {
            'success': 'run.completed', 'limit': 'run.completed', 'tools': 'run.completed',
            'thinking': 'run.completed', 'approval': 'run.completed',
            'interrupt': 'run.cancelled',
        }.get(scenario, 'run.failed')
        assert records[-1]['event']['type'] == expected_type, (scenario, records[-1]['event']['type'], err)
        assert records[0]['metadata']['runtime'] == 'claude'
        assert all(r.get('event', {}).get('sourceAdapter', 'claude') == 'claude' for r in records if r.get('recordType') == 'mission.event')
        assert all('RAW_REASONING_MUST_NOT_SURVIVE' not in json.dumps(r) for r in records)
        if scenario == 'success':
            assert out.strip().endswith('Hello world') or out == 'Hello world\n', repr(out)
        if scenario == 'approval':
            assert 'declines' in err.lower() or 'approve each' in err.lower(), err
        if scenario == 'limit':
            assert any(r.get('event', {}).get('type') == 'route.limit_detected' for r in records)
        print(f'{scenario}: exit {expected}, terminal {expected_type}')

# The actual executable, acting as Claude's stdio MCP permission server.
# An unreachable host must return a denial, never an implicit grant.
requests = [
    {'jsonrpc': '2.0', 'id': 1, 'method': 'initialize', 'params': {'protocolVersion': '2025-06-18'}},
    {'jsonrpc': '2.0', 'method': 'notifications/initialized'},
    {'jsonrpc': '2.0', 'id': 2, 'method': 'tools/list'},
    {'jsonrpc': '2.0', 'id': 3, 'method': 'tools/call', 'params': {'name': 'approve', 'arguments': {'tool_name': 'Write', 'input': {'file_path': 'hello.txt'}}}},
]
with tempfile.TemporaryDirectory(prefix='locust-mcp-smoke-') as folder:
    result = subprocess.run([str(binary), '--permission-bridge', str(pathlib.Path(folder) / 'absent')],
        input=''.join(json.dumps(r) + '\n' for r in requests), text=True, capture_output=True, timeout=5)
    assert result.returncode == 0, result.stderr
    replies = [json.loads(line) for line in result.stdout.splitlines()]
    assert [r['id'] for r in replies] == [1, 2, 3], replies
    assert replies[0]['result']['protocolVersion'] == '2025-06-18'
    assert replies[1]['result']['tools'][0]['name'] == 'approve'
    answer = json.loads(replies[2]['result']['content'][0]['text'])
    assert answer['behavior'] == 'deny', answer
print('permission-bridge: MCP initialize/list/call pass; unreachable host denied')
