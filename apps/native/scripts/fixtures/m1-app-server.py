#!/usr/bin/env python3
"""Synthetic JSON-RPC, based on runtime-adapters/test/codex-app-server-run.test.ts fakeServer.
No Codex invocation, config, credentials, or network. Assertions validate the client policy.
"""
import json, os, sys, time
if os.environ.get("M1_PID_FILE"):
    with open(os.environ["M1_PID_FILE"], "w") as f:
        f.write(str(os.getpid()))
scenario = sys.argv[1] if len(sys.argv) > 1 else 'success'
# M2: `resume` / `resume-interrupt` / `resume-failure` / `resume-refused` run the follow-up path (thread/resume, never
# thread/start) with the follow-up prompt; the rest are the M1 scenarios.
# M3b: `approve-allow` / `approve-deny` use Approve-each policy (untrusted + workspace-write).
resuming = scenario.startswith('resume')
cards_mode = scenario.startswith('cards-')
relay_mode = scenario.startswith('relay-')
room_mode = scenario.startswith('room-')
approving = scenario.startswith('approve-')
is_resumed_turn = False
is_forked_turn = False
base = scenario[len('resume-'):] if scenario.startswith('resume-') else ('success' if scenario == 'resume' else scenario)
# `rewind`: an edited earlier message starts again COLD from the turn before it (thread/start, never thread/resume),
# told what was said up to there.
rewinding = scenario == 'rewind'
sealed_mode = scenario in ('sealed', 'sealed-asking')
if approving or scenario == 'resume-undone' or rewinding or sealed_mode:
    base = 'success'
SANDBOX = 'workspace-write' if (approving or scenario == 'cards-edit-check') else 'read-only'
thread_sandbox = SANDBOX
POLICY = 'untrusted' if approving else 'never'
APPROVE_DECISION = 'accept' if scenario == 'approve-allow' else ('reject' if scenario == 'approve-deny' else 'decline')
PROMPT = 'Synthetic follow-up' if (resuming or rewinding) else 'Synthetic prompt'
def send(v):
    print(json.dumps(dict(jsonrpc='2.0', **v)), flush=True)
def notify(method, **params):
    send(dict(method=method, params=dict(threadId='thread_fake', **params)))
def item(done, **fields):
    notify('item/completed' if done else 'item/started', item=fields)
def check_config(config, sandbox=SANDBOX, sealed=sealed_mode):
    # codex-read-only.ts: only a read-only thread switches off the person's MCP programs (and Codex's plugins); a
    # server reached at an address stays. Any other sandbox keeps their Codex as it is.
    # A sealed run switches off shell, ChatGPT apps, web search, and remote connectors too (codexSealedThreadConfig).
    if sealed:
        assert config['mcp_servers.local.enabled'] is False and config['mcp_servers.remote.enabled'] is False, config
        assert config['features.plugins'] is False and config['features.apps'] is False and config['features.shell_tool'] is False, config
        assert config['web_search'] == 'disabled', config
    elif sandbox == 'read-only':
        assert config['mcp_servers.local.enabled'] is False and config['features.plugins'] is False, config
        assert 'mcp_servers.remote.enabled' not in config, config
    else:
        assert not any(k.startswith('mcp_servers.') or k.startswith('features.') for k in config), config
for line in sys.stdin:
    msg = json.loads(line)
    method, p = msg.get('method'), msg.get('params', {})
    if method == 'initialize':
        assert p['clientInfo']['name'] == 'locust'
        send(dict(id=msg['id'], result={}))
    elif method == 'initialized':
        pass
    elif method == 'config/read':
        send(dict(id=msg['id'], result={'config': {'mcp_servers': {'local': {'command':'node'}, 'remote': {'url':'https://example.invalid'}}}}))
    elif method == 'thread/resume':
        assert resuming or cards_mode or relay_mode, 'thread/resume outside a resume scenario'
        is_resumed_turn = True
        # codex-app-server-run.ts: reopen by the stored id, without the whole history, under the same policy.
        assert p['threadId'] == 'thread_fake' and p['excludeTurns'] is True, p
        assert p['sandbox'] == SANDBOX and p['approvalPolicy'] == POLICY
        check_config(p['config'])
        assert p['model'] == 'synthetic-model'
        if base == 'refused':
            send(dict(id=msg['id'], error={'code': -32600, 'message': 'no rollout found for thread id thread_fake'}))
        else:
            send(dict(id=msg['id'], result={'thread':{'id':'thread_fake'}}))
    elif method == 'thread/fork':
        is_forked_turn = True
        send(dict(id=msg['id'], result={'thread':{'id':'thread_fake'}}))
    elif method == 'thread/start':
        assert not resuming, 'a follow-up must resume, not start a new thread'
        expected_sandbox = p.get('sandbox') if scenario == 'cards-scout-fix' else SANDBOX
        if scenario == 'cards-scout-fix':
            assert expected_sandbox in ('read-only', 'workspace-write'), p
        thread_sandbox = expected_sandbox
        assert p['sandbox'] == expected_sandbox and p['approvalPolicy'] == POLICY, p
        is_sealed = (expected_sandbox == 'read-only') if scenario == 'cards-scout-fix' else sealed_mode
        check_config(p['config'], expected_sandbox, is_sealed)
        send(dict(id=msg['id'], result={'thread':{'id':'thread_fake'}}))
    elif method == 'turn/start':
        assert p['summary'] == 'detailed' and p['approvalPolicy'] == POLICY, p
        # Native briefs folder / instructions / About you / memory before the person's words
        # (Electron workspace-brief + about-you + memory); the ledger still keeps the bare prompt.
        assert len(p.get('input') or []) == 1 and p['input'][0].get('type') == 'text', p
        text = p['input'][0].get('text') or ''
        # Every turn ends with the reference-folders line, after the person's words (codex-mission.ts referencesSection).
        words = text.split('\n\nReference folders — for reading, not changing:')[0]
        if is_forked_turn:
            assert 'This is a side question from the person, asked on a copy of this conversation' in text, text
            send(dict(id=msg['id'], result={'turn':{'id':'turn_fake'}}))
            notify('turn/started', turn={'id':'turn_fake'})
            reply = 'Side answer from copy.'
            notify('item/agentMessage/delta', itemId='answer', operation='replace', delta=reply)
            item(True, id='answer', type='agentMessage', text=reply)
            notify('turn/completed', turn={'id':'turn_fake','status':'completed'})
            continue
        if not cards_mode and not relay_mode and not room_mode:
            assert words == PROMPT or words.endswith('\n\n' + PROMPT), text
        # A follow-up to an undone turn starts with what Undo put back (undone-note.ts); any other follow-up says nothing of it.
        if scenario == 'resume-undone':
            assert text.startswith('Before this message, the person pressed Undo on your previous turn in Locust.') and 'laptop-setup.md' in text, text
        elif resuming:
            assert 'pressed Undo' not in text, text
        # Asking Scout (0.828): What Scout saw is in front of the person's words, as the turn's instructions.
        if scenario == 'sealed-asking':
            assert 'PR #9 minor lint' in text and text.index('PR #9 minor lint') < text.index(PROMPT), text
        if rewinding:
            assert 'The person went back to an earlier point in this conversation and changed their message there.' in text and 'Asked: "Synthetic prompt"' in text and 'Hello world' in text, text
        assert p['model'] == 'synthetic-model'
        send(dict(id=msg['id'], result={'turn':{'id':'turn_fake'}}))
        notify('turn/started', turn={'id':'turn_fake'})
        send(dict(method='turn/completed', params={'threadId':'other_thread'}))
        if room_mode:
            with open('room-prompts.jsonl', 'a') as f:
                f.write(json.dumps(words) + '\n')
            if scenario == 'room-queue' and ('Hold slot' in words or 'You are Codec, answering' in words):
                while os.path.exists('slot-hold.gate') and not os.path.exists('slot-release.gate'):
                    time.sleep(0.02)
            if 'You are Codec, answering' in words:
                reply = 'Codec room answer.'
            elif 'You are Pip, answering' in words:
                reply = 'Pip room answer.'
            else:
                reply = 'Room answer.'
            notify('item/agentMessage/delta', itemId='answer', operation='replace', delta=reply)
            item(True, id='answer', type='agentMessage', text=reply)
            notify('turn/completed', turn={'id':'turn_fake','status':'completed'})
            continue
        if cards_mode:
            if scenario == 'cards-decision':
                if is_resumed_turn:
                    assert 'Keep notes.md (the newer one). Continue with that.' in words, words
                    reply = 'Got it, keeping notes.md.'
                else:
                    reply = 'I checked both.\n\n<locust-ask>\nWhich file is right?\n- Keep notes.md :: the newer one\n- Keep NOTES.txt\n</locust-ask>'
            elif scenario == 'cards-next':
                if is_resumed_turn:
                    assert 'Clean the Linux cache too: Do the same for the AppImage cache.' in words, words
                    reply = 'Cleaned the Linux cache.'
                else:
                    reply = "Done.\n\n<locust-next>\nClean the Linux cache too :: Do the same for the AppImage cache.\nTell people in What's new :: Add a line to the changelog.\n</locust-next>"
            elif scenario == 'cards-question':
                # Codex's requestUserInput: a questions array; answered by id, an option by its label.
                send(dict(id='question', method='item/tool/requestUserInput', params={'threadId':'thread_fake','turnId':'turn_fake','itemId':'q1','questions':[
                    {'id':'database','header':'Storage','question':'Which database should the fixture use?','isOther':False,'isSecret':False,
                     'options':[{'label':'SQLite','description':'No server to run'},{'label':'Postgres','description':None}]},
                    {'id':'notes','header':None,'question':'Anything to keep?','isOther':False,'isSecret':False,'options':None}]}))
                continue
            elif scenario == 'cards-share':
                # The brief as the teammate got it, for the test to read; then a share, or the reply to one.
                with open('last-prompt.txt', 'w') as f:
                    f.write(text)
                if 'Messages from teammates, delivered before you started.' in text:
                    reply = 'Got the note about the port.'
                else:
                    reply = 'Checked the server.\n\n<locust-share to="Pip">The dev server listens on port 3001, set in vite.config.ts.</locust-share>'
            elif scenario == 'cards-file':
                reply = 'Here is the rollup.\n\n<locust-file>\ndocs/report.md :: the rollup you asked for\n\\\\server\\share\\bad.md\n</locust-file>'
            elif scenario == 'cards-streaming':
                notify('item/agentMessage/delta', itemId='answer', delta='Thinking...\n<locust-ask>\nWhich?\n- A :: one\n<locu')
                time.sleep(0.35)
                reply = 'Thinking...\n<locust-ask>\nWhich?\n- A :: one\n- B :: two\n</locust-ask>'
            elif scenario == 'cards-edit-check':
                if is_resumed_turn:
                    assert 'fix 1 of 2' in words, words
                    with open('notes.txt', 'w') as f:
                        f.write('FIXED\n')
                    reply = 'Fixed notes.txt.'
                else:
                    with open('notes.txt', 'w') as f:
                        f.write('BROKEN\n')
                    reply = 'Wrote notes.txt.'
            elif scenario == 'cards-scout-fix':
                if thread_sandbox == 'read-only':
                    reply = 'ITEM 1\nTAKE: Check failed on answer.txt.\nHAND TO: Wren\nDRAFT: Fix answer.txt so grep passes.'
                else:
                    parent = os.path.dirname(os.getcwd())
                    hold = os.path.join(parent, 'hold.gate')
                    release = os.path.join(parent, 'release.gate')
                    while os.path.exists(hold) and not os.path.exists(release):
                        time.sleep(0.02)
                    with open('answer.txt', 'w') as f:
                        f.write('42\n')
                    reply = 'Fixed answer.txt to 42.'
            else:
                reply = 'Unknown cards scenario'
            notify('item/agentMessage/delta', itemId='answer', operation='replace', delta=reply)
            item(True, id='answer', type='agentMessage', text=reply)
            notify('turn/completed', turn={'id':'turn_fake','status':'completed'})
            continue
        if scenario.startswith('relay-'):
            with open('relay-prompts.jsonl', 'a') as f:
                f.write(json.dumps({'prompt': text, 'words': words, 'resumed': is_resumed_turn}) + '\n')
            if scenario == 'relay-roundtrip':
                if 'What port is Vite on?' in text and not is_resumed_turn:
                    reply = '<locust-share to="Codec">Vite listens on port 5173. Please confirm.</locust-share>'
                elif 'Vite listens on port 5173.' in text:
                    reply = 'Got it, thanks!'
                else:
                    reply = 'Asking Pip now.\n\n<locust-share to="Pip">What port is Vite on?</locust-share>'
            elif scenario == 'relay-prose-return':
                if 'Which port should docs use?' in text and not is_resumed_turn:
                    reply = 'Use port 5173 for docs.'
                elif 'Use port 5173 for docs.' in text:
                    reply = 'Recorded 5173.'
                else:
                    reply = '<locust-share to="Pip">Which port should docs use?</locust-share>'
            elif scenario == 'relay-busy':
                if 'Check the lock first' in text:
                    while os.path.exists('pip-busy.gate') and not os.path.exists('pip-release.gate'):
                        time.sleep(0.02)
                    reply = 'Lock checked.'
                elif 'Please review migration 4.' in text and not is_resumed_turn:
                    reply = '<locust-share to="Codec">Reviewed migration 4. Thanks!</locust-share>'
                elif 'Reviewed migration 4.' in text:
                    reply = 'Received migration review.'
                else:
                    reply = '<locust-share to="Pip">Please review migration 4.</locust-share>'
            elif scenario == 'relay-cap':
                if 'Step from Codec' in text:
                    reply = '<locust-share to="Codec">Step from Pip?</locust-share>'
                elif 'Step from Pip?' in text:
                    reply = '<locust-share to="Pip">Step from Codec?</locust-share>'
                else:
                    reply = '<locust-share to="Pip">Step from Codec?</locust-share>'
            elif scenario == 'relay-stop':
                if 'Long audit task' in text and not is_resumed_turn:
                    notify('item/agentMessage/delta', itemId='answer', operation='replace', delta='Working on audit...')
                    continue
                reply = '<locust-share to="Pip">Long audit task</locust-share>'
            else:
                reply = 'Unknown relay scenario'
            notify('item/agentMessage/delta', itemId='answer', operation='replace', delta=reply)
            item(True, id='answer', type='agentMessage', text=reply)
            notify('turn/completed', turn={'id':'turn_fake','status':'completed'})
            continue
        if base == 'lost':
            sys.exit(0)
        if base == 'failure':
            notify('error', error={'message':'Usage limit reached', 'codexErrorInfo':'usageLimitExceeded'})
            continue
        if base == 'flood':
            time.sleep(0.3)  # Allow the CLI to record run.started and receive SIGINT.
            while True:  # Never read stdin again, including interrupt/approval replies.
                notify('item/agentMessage/delta', itemId='answer', delta='x' * 4096)
        if base == 'interrupt':
            continue
        notify('item/agentMessage/delta', itemId='answer', delta='Hello ')
        time.sleep(0.4)  # CLI test must observe this delta before the turn can end.
        notify('item/agentMessage/delta', itemId='answer', delta='world')
        item(False, id='thinking', type='reasoning', content=['RAW_REASONING_MUST_NOT_SURVIVE'])
        item(True, id='thinking', type='reasoning', summary=['Checking the fixture'], content=['RAW_REASONING_MUST_NOT_SURVIVE'])
        notify('turn/plan/updated', turnId='turn_fake', plan=[{'step':'Check fixture','status':'completed'}])
        for kind, fields in [('commandExecution', {'command':'echo hello','aggregatedOutput':'hello','exitCode':0}), ('webSearch', {'query':'fixture'}), ('fileChange', {}), ('mcpToolCall', {'server':'docs','tool':'read'})]:
            item(False, id=kind, type=kind, **fields)
            item(True, id=kind, type=kind, status='completed', **fields)
        item(True, id='answer', type='agentMessage', text='Hello world')
        send(dict(id='approval', method='item/commandExecution/requestApproval',params={'command':'touch forbidden','cwd':'/tmp'}))
    elif msg.get('id') == 'question':
        assert msg.get('result') == {'answers': {'database': {'answers': ['SQLite']}, 'notes': {'answers': ['Keep the migration names.']}}}, msg
        reply = 'Using SQLite, keeping the migration names.'
        notify('item/agentMessage/delta', itemId='answer', operation='replace', delta=reply)
        item(True, id='answer', type='agentMessage', text=reply)
        notify('turn/completed', turn={'id':'turn_fake','status':'completed'})
    elif msg.get('id') == 'approval':
        assert msg['result']['decision'] == APPROVE_DECISION, msg
        notify('warning', message='Synthetic warning')
        notify('thread/tokenUsage/updated', tokenUsage={'total':{'inputTokens':12,'outputTokens':4},'last':{'inputTokens':12,'outputTokens':4}})
        notify('turn/completed', turn={'id':'turn_fake','status':'completed'})
    elif method == 'turn/interrupt':
        assert p == {'threadId':'thread_fake','turnId':'turn_fake'}
        send(dict(id=msg['id'],result={}))
        notify('turn/completed', turn={'id':'turn_fake','status':'interrupted'})
    else:
        raise AssertionError(msg)
