"""Scripted ACP peer, based on acp-run.test.ts and measured ACP tool frames."""
import json, sys, time
scenario = sys.argv[1] if len(sys.argv) > 1 else 'success'
prompt = None
def send(v):
    print(json.dumps(dict(jsonrpc='2.0', **v)), flush=True)
def answer(v, result):
    send(dict(id=v['id'], result=result))
def update(**u):
    send(dict(method='session/update', params=dict(sessionId='s1', update=u)))
for line in sys.stdin:
    v = json.loads(line)
    method = v.get('method')
    if method == 'initialize':
        assert v['params'] == dict(protocolVersion=1, clientCapabilities=dict(fs=dict(readTextFile=False,writeTextFile=False),terminal=False))
        if scenario == 'version':
            answer(v, dict(protocolVersion=99))
        else:
            answer(v, dict(protocolVersion=1,agentCapabilities=dict(loadSession=scenario != 'no-resume')))
    elif method in ('session/new','session/load'):
        assert v['params']['mcpServers'] == []
        update(sessionUpdate='agent_message_chunk', content=dict(type='text',text='REPLAY_MUST_NOT_SURVIVE'))
        answer(v, dict(sessionId='s1',configOptions=[dict(id='model',currentValue='old')]))
    elif method == 'session/set_config_option':
        answer(v, dict(configOptions=[dict(id=v['params']['configId'],currentValue=v['params']['value'])]))
    elif method == 'session/prompt':
        prompt = v
        if scenario == 'stderr-quota':
            print('Usage limit exceeded',file=sys.stderr,flush=True)
            time.sleep(.1)
            sys.exit(1)
        if scenario == 'oversize':
            print('x' * (8*1024*1024+1),flush=True)
            continue
        if scenario == 'flood':
            for _ in range(10000):
                update(sessionUpdate='agent_message_chunk',content=dict(type='text',text='x'))
            continue
        if scenario == 'lost':
            sys.exit(0)
        if scenario == 'malformed':
            print('this is not JSON', flush=True)
        if scenario in ('stop','hang'):
            continue
        if scenario == 'quota':
            send(dict(id=v['id'], error=dict(code=-1,message='Usage limit exceeded')))
            continue
        if scenario == 'refusal':
            answer(v,dict(stopReason='refusal'))
            continue
        # A foreign session's text must not enter this turn.
        send(dict(method='session/update',params=dict(sessionId='other',update=dict(sessionUpdate='agent_message_chunk',content=dict(type='text',text='FOREIGN_MUST_NOT_SURVIVE')))))
        update(sessionUpdate='agent_thought_chunk',content=dict(type='text',text='RAW_REASONING_MUST_NOT_SURVIVE'))
        update(sessionUpdate='agent_message_chunk',content=dict(type='text',text='Before.'))
        update(sessionUpdate='available_commands_update',availableCommands=[dict(name='PRIVATE_COMMAND_MUST_NOT_SURVIVE')])
        update(sessionUpdate='tool_call',toolCallId='tool1',kind='execute',title='shell',rawInput=dict(cwd='/tmp'),locations=[dict(path='/tmp')])
        update(sessionUpdate='tool_call_update',toolCallId='tool1',kind='execute',rawInput=dict(command='echo hi'))
        send(dict(id='files',method='fs/read_text_file',params=dict(path='/etc/passwd')))
    elif v.get('id') == 'files':
        assert v['error']['code'] == -32601
        send(dict(id='permission',method='session/request_permission',params=dict(toolCall=dict(toolCallId='tool1',kind='execute',title='Run echo',rawInput=dict(command='echo hi')),options=[dict(optionId='yes',kind='allow_once'),dict(optionId='forever',kind='allow_always'),dict(optionId='no',kind='reject_once')])))
        if scenario == 'pending-exit':
            sys.exit(0)
        if scenario == 'terminal-pending':
            answer(prompt,dict(stopReason='end_turn'))
    elif v.get('id') == 'permission':
        if scenario == 'terminal-pending':
            assert v['result']['outcome']['outcome'] == 'cancelled'
            continue
        option = v['result']['outcome'].get('optionId')
        assert option == ('yes' if scenario in ('approve','always') else 'no'), v
        update(sessionUpdate='tool_call_update',toolCallId='tool1',status='completed' if option=='yes' else 'failed',locations=[dict(path='/tmp')],content=[dict(type='content',content=dict(type='text',text='hi'))])
        update(sessionUpdate='tool_call',toolCallId='edit',kind='edit',status='completed',content=[dict(type='diff',path='a.txt',oldText='same\nold\n',newText='same\nnew\n')])
        update(sessionUpdate='agent_message_chunk',content=dict(type='text',text='After.'))
        answer(prompt, dict(stopReason='max_tokens' if scenario=='limit' else 'cancelled' if scenario=='declined' else 'end_turn',usage=dict(inputTokens=10,outputTokens=2)))
    elif method == 'session/cancel':
        answer(prompt, dict(stopReason='cancelled'))
