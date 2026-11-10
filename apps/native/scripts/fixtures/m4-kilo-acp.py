"""Scripted Kilo ACP peer: offers model then mode, based on m4-opencode-acp.py."""
import json, sys
args = [a for a in sys.argv[1:] if a != 'acp']
scenario = args[0] if args else 'success'
prompt = None
seen = []
def send(v):
    print(json.dumps(dict(jsonrpc='2.0', **v)), flush=True)
def answer(v, result):
    send(dict(id=v['id'], result=result))
def update(**u):
    send(dict(method='session/update', params=dict(sessionId='s1', update=u)))
options = [
    dict(id='model', currentValue='kilo/google/gemini-3-pro-image'),
    dict(id='mode', currentValue='code'),
]
for line in sys.stdin:
    v = json.loads(line)
    method = v.get('method')
    if method == 'initialize':
        flag = __import__('os').environ.get('LOCUST_KILO_CRASH_FLAG', '')
        if scenario == 'crash-once' and flag and not __import__('os').path.exists(flag):
            open(flag, 'w').close()
            sys.exit(0)
        answer(v, dict(protocolVersion=1, agentCapabilities=dict(loadSession=True)))
    elif method in ('session/new', 'session/load'):
        update(sessionUpdate='agent_message_chunk', content=dict(type='text', text='REPLAY_MUST_NOT_SURVIVE'))
        answer(v, dict(sessionId='s1', configOptions=list(options)))
    elif method == 'session/set_config_option':
        cid, value = v['params']['configId'], v['params']['value']
        seen.append(cid)
        for row in options:
            if row['id'] == cid:
                row['currentValue'] = value
        # Model first, then mode: prove order for the success path.
        if scenario == 'order' and cid == 'mode' and seen != ['model', 'mode']:
            send(dict(id=v['id'], error=dict(code=-1, message=f'order was {seen}')))
            continue
        answer(v, dict(configOptions=list(options)))
    elif method == 'session/prompt':
        prompt = v
        if scenario == 'lost':
            sys.exit(0)
        update(sessionUpdate='agent_thought_chunk', content=dict(type='text', text='RAW_REASONING_MUST_NOT_SURVIVE'))
        update(sessionUpdate='agent_message_chunk', content=dict(type='text', text='Kilo says hi.'))
        update(sessionUpdate='tool_call', toolCallId='tool1', kind='execute', title='shell', rawInput=dict(command='echo hi'))
        send(dict(id='permission', method='session/request_permission', params=dict(toolCall=dict(toolCallId='tool1', kind='execute', title='Run echo', rawInput=dict(command='echo hi')), options=[dict(optionId='yes', kind='allow_once'), dict(optionId='forever', kind='allow_always'), dict(optionId='no', kind='reject_once')])))
    elif v.get('id') == 'permission':
        option = v['result']['outcome'].get('optionId')
        # Kilo never raises Locust cards; Ask rejects, Auto allows.
        assert option == ('yes' if scenario == 'auto' else 'no'), v
        update(sessionUpdate='tool_call_update', toolCallId='tool1', status='completed' if option == 'yes' else 'failed')
        update(sessionUpdate='agent_message_chunk', content=dict(type='text', text='Done.'))
        answer(prompt, dict(stopReason='end_turn', usage=dict(inputTokens=3, outputTokens=1)))
    elif method == 'session/cancel':
        answer(prompt, dict(stopReason='cancelled'))
