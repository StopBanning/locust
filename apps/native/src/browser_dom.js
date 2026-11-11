(arg => {
  const describe = e => ({tag:e.tagName.toLowerCase(),role:e.getAttribute('role') || (e.tagName==='A'?'link':e.tagName==='BUTTON'?'button':'textbox'),name:(e.getAttribute('aria-label') || e.labels?.[0]?.innerText || (!['INPUT','TEXTAREA','SELECT'].includes(e.tagName) ? e.innerText : '') || e.getAttribute('name') || '').slice(0,200),type:e.type || '',placeholder:e.getAttribute('placeholder') || '',href:e.getAttribute('href') || '',formAction:e.form?.action || '',submit:e.type==='submit' || !!e.form,download:e.hasAttribute('download'),sensitive:e.type==='password' || /password|passcode|cc-number|cc-csc|one-time-code/i.test(e.autocomplete || ''),disabled:!!e.disabled});
  if (arg.action === 'read') {
    globalThis.__locustRefs = Array.from(document.querySelectorAll('a,button,input,textarea,select,[role],[contenteditable="true"]')).filter(e => e.getClientRects().length).slice(0,120);
    return JSON.stringify({title:document.title,url:location.href,text:(document.body?.innerText || '').slice(0,12000),elements:globalThis.__locustRefs.map((e,i)=>({ref:'@e'+(i+1),...describe(e)}))});
  }
  const e = globalThis.__locustRefs?.[Number(String(arg.ref || '').replace('@e',''))-1];
  if (!e?.isConnected) throw Error('Element was not found on the current page. Call browser_read to refresh element refs.');
  const meta = describe(e);
  if (arg.action === 'probe') return JSON.stringify(meta);
  if (JSON.stringify(meta) !== JSON.stringify(arg.expected)) throw Error('The element changed while waiting for approval. Nothing was changed.');
  if (meta.disabled) throw Error('The element is disabled. Nothing was changed.');
  if (arg.action === 'click') e.click();
  else if (arg.action === 'type') {
    if (!['INPUT','TEXTAREA'].includes(e.tagName) || e.type==='file') throw Error('This field cannot be typed into.');
    e.focus();const proto=e.tagName==='TEXTAREA'?HTMLTextAreaElement.prototype:HTMLInputElement.prototype;
    Object.getOwnPropertyDescriptor(proto,'value').set.call(e,arg.value);e.dispatchEvent(new Event('input',{bubbles:true}));e.dispatchEvent(new Event('change',{bubbles:true}));
  } else if (arg.action === 'select') {
    if (e.tagName!=='SELECT') throw Error('This is not a select control.');
    e.value=arg.value;e.dispatchEvent(new Event('input',{bubbles:true}));e.dispatchEvent(new Event('change',{bubbles:true}));
  } else if (arg.action === 'press') {
    e.focus();e.dispatchEvent(new KeyboardEvent('keydown',{key:arg.key,bubbles:true}));
    if (arg.key==='Enter' && e.form) e.form.requestSubmit();else if(arg.key==='Enter'&&['BUTTON','A'].includes(e.tagName))e.click();
    e.dispatchEvent(new KeyboardEvent('keyup',{key:arg.key,bubbles:true}));
  } else throw Error('Unknown browser action. Nothing was changed.');
  return JSON.stringify({ok:true});
})(ARGUMENT)
