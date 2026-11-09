#!/usr/bin/env python3
"""Only synthetic subprocesses. Usage: python3 scripts/test-m1-cli.py <locust-native binary>"""
import json, os, pathlib, shlex, signal, subprocess, sys, tempfile, time
root = pathlib.Path(__file__).resolve().parent.parent
binary = pathlib.Path(sys.argv[1]).resolve()
with tempfile.TemporaryDirectory(prefix='.m1-cli-', dir=root) as tmp:
    tmp = pathlib.Path(tmp)
    fake = tmp / 'codex'
    fake.write_text('#!/bin/sh\nexec python3 ' + shlex.quote(str(root / 'scripts/fixtures/m1-app-server.py')) + ' "$M1_SCENARIO"\n')
    fake.chmod(0o700)
    for scenario, expected in [('success', 0), ('failure', 1), ('lost', 1), ('interrupt', 1), ('flood', 1)]:
        data = tmp / scenario
        env = dict(os.environ, PATH=str(tmp) + os.pathsep + os.environ['PATH'], LOCUST_NATIVE_DATA=str(data), M1_SCENARIO=scenario, M1_PID_FILE=str(tmp / 'server.pid'))
        child = subprocess.Popen([str(binary), '--run','codex','Synthetic prompt','--folder',str(root),'--model','synthetic-model'], env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        prefix = ''
        if scenario == 'success':
            prefix = child.stdout.read(6)
            assert prefix == 'Hello ' and child.poll() is None, 'Reply did not stream before completion'
        if scenario in ('interrupt', 'flood'):
            deadline = time.monotonic() + 10
            while True:
                files = list(data.glob('mission-ledger/*.jsonl'))
                if files and 'run.started' in files[0].read_text():
                    break
                assert time.monotonic() < deadline, 'Turn did not start'
                time.sleep(.02)
            if scenario == 'flood':
                prefix = child.stdout.read(4096)
                assert prefix == 'x' * 4096, 'No flood delta observed before Ctrl-C'
            cancelled_at = time.monotonic()
            child.send_signal(signal.SIGINT)
        out, err = child.communicate(timeout=10)
        if scenario in ('interrupt', 'flood'):
            assert time.monotonic() - cancelled_at < 6, 'Cancellation exceeded five seconds plus scheduling margin'
        if scenario == 'flood':
            pid = int((tmp / 'server.pid').read_text())
            try:
                os.kill(pid, 0)
            except ProcessLookupError:
                pass
            else:
                raise AssertionError('Flooding fake server survived CLI exit')
        out = prefix + out
        assert child.returncode == expected, (scenario, child.returncode, err)
        if scenario == 'success':
            assert out == 'Hello world\n', repr(out)
            assert 'declines approvals' in err
        records = [json.loads(line) for line in next(data.glob('mission-ledger/*.jsonl')).read_text().splitlines()]
        expected_type = {'success':'run.completed','interrupt':'run.cancelled'}.get(scenario,'run.failed')
        if scenario == 'flood':
            expected_type = records[-1]['event']['type']
            assert expected_type in ('run.cancelled', 'run.failed')
        else:
            assert records[-1]['event']['type'] == expected_type
        assert all('RAW_REASONING_MUST_NOT_SURVIVE' not in json.dumps(r) for r in records)
        timing = f', cancellation {time.monotonic() - cancelled_at:.3f}s' if scenario in ('interrupt', 'flood') else ''
        print(f'{scenario}: exit {expected}, terminal {expected_type}, stdout {out[:80]!r}{timing}')
