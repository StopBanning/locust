(() => {
  const post = (entry) => {
    try { window.webkit.messageHandlers.locustLog.postMessage(JSON.stringify({ ...entry, at: new Date().toISOString() }).slice(0, 32000)); } catch {}
  };
  for (const level of ['log', 'info', 'warn', 'error', 'debug']) {
    const original = console[level];
    console[level] = function (...args) {
      post({kind: 'console', level: level === 'warn' ? 'warning' : level === 'error' ? 'error' : 'info', message: args.map(v => {try {return typeof v === 'string' ? v : JSON.stringify(v);} catch {return '[unserializable]';}}).join(' ').slice(0, 16000)});
      return original.apply(this, args);
    };
  }
  addEventListener('error', e => post({kind:'console', level:'error', message:String(e.message || 'Page error'), source:e.filename, line:e.lineno}));
  addEventListener('unhandledrejection', () => post({kind:'console', level:'error', message:'Unhandled promise rejection'}));
  const request = window.fetch;
  window.fetch = async function (input, init) {
    const url = String(typeof input === 'string' ? input : input?.url || input);
    const method = init?.method || input?.method || 'GET';
    try { const response = await request.apply(this, arguments); post({kind:'network', method, url, status:response.status, ok:response.ok, resourceType:'fetch'}); return response; }
    catch(e) {post({kind:'network', method, url, ok:false, error:String(e), resourceType:'fetch'}); throw e;}
  };
  const open = XMLHttpRequest.prototype.open, send = XMLHttpRequest.prototype.send;
  const details = new WeakMap();
  XMLHttpRequest.prototype.open = function(method, url) { details.set(this,{method:String(method),url:String(url)}); return open.apply(this,arguments); };
  XMLHttpRequest.prototype.send = function() {this.addEventListener('loadend',()=>post({kind:'network',...details.get(this),status:this.status,ok:this.status>0&&this.status<400,resourceType:'xhr'}),{once:true});return send.apply(this,arguments);};
  try {new PerformanceObserver(list=>{for(const e of list.getEntries()) if(!['fetch','xmlhttprequest'].includes(e.initiatorType)) post({kind:'network', method:'GET', url:e.name,resourceType:e.initiatorType});}).observe({type:'resource',buffered:true});}catch{}
})();
