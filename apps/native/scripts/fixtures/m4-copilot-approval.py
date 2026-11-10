"""Copilot ACP Agent mode/allow_all gate; no prompt before both settings are safe."""
import sys,json
scenario=sys.argv[1]
agent='https://agentclientprotocol.com/protocol/session-modes#agent'
mode='autopilot'
allow='on'
def answer(v,result): print(json.dumps(dict(jsonrpc='2.0',id=v['id'],result=result)),flush=True)
for line in sys.stdin:
    v=json.loads(line);method=v.get('method')
    if method=='initialize': answer(v,dict(protocolVersion=1,agentCapabilities=dict(loadSession=True)))
    elif method in ('session/new','session/load'):
        result=dict(sessionId='s1',modes=dict(currentModeId=mode,availableModes=[dict(id=agent)]))
        if scenario!='missing':result['configOptions']=[dict(id='allow_all',currentValue=allow)]
        answer(v,result)
    elif method=='session/set_mode':
        assert v['params']['modeId']==agent;mode=agent;answer(v,{})
    elif method=='session/set_config_option':
        assert mode==agent and v['params']['configId']=='allow_all' and v['params']['value']=='off'
        allow='on' if scenario=='refuses' else 'off';answer(v,dict(configOptions=[dict(id='allow_all',currentValue=allow)]))
    elif method=='session/prompt':
        assert scenario!='refuses' and mode==agent and (allow=='off' or scenario=='missing')
        prompt=v
        print(json.dumps(dict(jsonrpc='2.0',id='permission',method='session/request_permission',params=dict(toolCall=dict(toolCallId='t1',kind='execute',title='Run echo',rawInput=dict(command='echo hi')),options=[dict(kind='reject_once',optionId='no'),dict(kind='allow_once',optionId='yes')]))),flush=True)
    elif v.get('id')=='permission':
        assert v['result']['outcome']['optionId']==('yes' if scenario=='allow' else 'no')
        answer(prompt,dict(stopReason='end_turn'))
