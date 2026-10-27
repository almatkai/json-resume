#!/usr/bin/env python3
"""Production SDK JSONL and owned loopback transport regression. Synthetic data only."""
import urllib.parse, gzip, zlib, uuid, contextlib, hashlib, http.server, json, os, pathlib, queue, ssl, subprocess, sys, tempfile, threading, time
ROOT=pathlib.Path(__file__).resolve().parents[1]
QA=ROOT/'.qa'; QA.mkdir(exist_ok=True)
BINARY=pathlib.Path(sys.argv[1] if len(sys.argv)>1 else ROOT/'.qa/target/debug/json-resume-v2').resolve()
assert BINARY.is_relative_to(ROOT)
PACKAGE=BINARY.parent.parent if BINARY.parent.name=='bin' else ROOT
checks=0

def check(value,message='assertion'):
 global checks
 assert value,message
 checks+=1

class Server(http.server.ThreadingHTTPServer):
 daemon_threads=True
 def __init__(self):
  super().__init__(('127.0.0.1',0),Handler);self.hits=[];self.gates={};self.seen=threading.Event();self.body_started=threading.Event();self.client_closed=threading.Event();self.cancel_cases={};self.response_gates={}
 def handle_error(self,request,client_address):
  if isinstance(sys.exc_info()[1],(ConnectionResetError,BrokenPipeError,TimeoutError)):return
  super().handle_error(request,client_address)
class Handler(http.server.BaseHTTPRequestHandler):
 protocol_version='HTTP/1.1'
 def log_message(self,*args):pass
 def do_GET(self):
  p=self.path.split('?')[0];self.server.hits.append(self.path);occurrence=self.server.hits.count(self.path)
  if (p,occurrence) in self.server.response_gates:self.server.response_gates[(p,occurrence)].wait(4)
  if p.startswith('/delay'):
   event=self.server.gates.setdefault(p,threading.Event());self.server.seen.set();event.wait(4)
  if p.startswith('/chain'):
   remaining=int(p[6:]);self.send_response(302);self.send_header('Location','/chain'+str(remaining-1) if remaining>1 else '/valid');self.send_header('Content-Length','0');self.end_headers();return
  if p=='/redirect':
   self.send_response(302);self.send_header('Location','/valid');self.send_header('Content-Length','0');self.end_headers();return
  if p=='/loop':
   self.send_response(302);self.send_header('Location','/loop');self.send_header('Content-Length','0');self.end_headers();return
  if p.startswith('/cancel-observe'):
   self.send_response(200);self.send_header('Content-Type','application/json');self.send_header('Content-Length','10000');self.end_headers();started,closed=self.server.cancel_cases.get(p,(self.server.body_started,self.server.client_closed));started.set();self.connection.settimeout(4)
   try:
    if self.rfile.read(1)==b'':closed.set()
   except TimeoutError:pass
   return
  if p=='/error':self.send_error(503);return
  if p=='/timeout':time.sleep(17)
  if p.startswith('/same-url') and occurrence==1:
   self.server.gates.setdefault(p,threading.Event()).wait(4)
  data={'basics':{'name':'Synthetic Ada','summary':'Raw **markdown** <b>retained</b>'}}
  if p=='/namedB':data={'basics':{'name':'Synthetic Bee'}}
  if p.startswith('/same-url'):data={'basics':{'name':'Newer record' if occurrence>1 else 'Late record'}}
  if p=='/nameless':data={}
  if p in ['/invalid','/aba-process-title']:data={'skills':'bad'}
  if p=='/null':data=None
  if p=='/depth':
   data={}
   for _ in range(66):data={'extra':data}
  if p=='/large':body=b' '* (1024*1024+1)
  elif p=='/malformed':body=b'{not JSON'
  elif p=='/mime':body=json.dumps(json.dumps(json.dumps({}))).encode()
  elif p in ['/json-string','/text-string']:body=json.dumps(json.dumps(data)).encode()
  else:body=json.dumps(data).encode()
  if p=='/gzip':body=gzip.compress(body)
  if p=='/deflate':body=zlib.compress(body)
  self.send_response(200)
  if p in ['/gzip','/deflate']:self.send_header('Content-Encoding',p[1:])
  self.send_header('Content-Type',urllib.parse.parse_qs(urllib.parse.urlsplit(self.path).query,keep_blank_values=True).get('type',[''])[0] if p=='/mime' else 'text/plain' if p in ['/text','/text-string'] else 'application/problem+json');self.send_header('Content-Length',str(len(body)));self.end_headers()
  try:self.wfile.write(body)
  except (BrokenPipeError,ConnectionResetError):pass

class Pipe:
 def __init__(self,package=None,shared_data=None):
  self.temp=tempfile.TemporaryDirectory(dir=QA,prefix='pipe-');self.dir=pathlib.Path(self.temp.name);self.data=shared_data or self.dir/'data';self.data.mkdir(exist_ok=True);self.n=0;self.rev={};self.views={};self.requests=[];self.events=[];self.auto_ack=True;self.ack={'ok':True}
  env={k:v for k,v in os.environ.items() if k in ['PATH','SYSTEMROOT','WINDIR']}
  for k in ['HOME','XDG_DATA_HOME','XDG_CACHE_HOME','XDG_CONFIG_HOME','TMPDIR']:env[k]=str(self.dir/k.lower());pathlib.Path(env[k]).mkdir()
  self.proc=subprocess.Popen([str(BINARY)],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,cwd=package or PACKAGE,env=env,bufsize=0)
  self.q=queue.Queue();self.reader=threading.Thread(target=self.read,daemon=True);self.reader.start();self.responses={}
  check(self.frame()['type']=='hello')
  self.call('initialize',{'protocol':1,'extensionId':'com.tezbar.native.json-resume','platform':'macOS','packageDir':str(package or PACKAGE),'dataDir':str(self.data),'tempDir':str(self.dir),'grantedCapabilities':['clipboard.write','shell.open'],'migrationPaths':{}})
 def read(self):
  try:
   for line in self.proc.stdout:self.q.put(json.loads(line))
  except Exception as e:self.q.put(e)
  self.q.put(None)
 def send(self,v):self.proc.stdin.write(json.dumps(v,ensure_ascii=False).encode()+b'\n');self.proc.stdin.flush()
 def frame(self,timeout=8):
  v=self.q.get(timeout=timeout)
  assert v is not None,'Unexpected EOF'
  if isinstance(v,Exception):raise v
  return v
 def request(self,method,params):
  self.n+=1;id='h:'+str(uuid.uuid4());self.send({'type':'request','id':id,'method':method,'params':params});return id
 def response(self,id,timeout=10):
  deadline=time.monotonic()+timeout
  while id not in self.responses:
   v=self.frame(max(.001,deadline-time.monotonic()))
   if v['type']=='request':
    self.requests.append(v)
    if self.auto_ack:self.send({'type':'response','id':v['id'],'result':self.ack})
   elif v['type']=='response':self.responses[v['id']]=v
   elif v['type']=='event':self.events.append(v)
  return self.responses.pop(id)
 def call(self,m,p,expect_error=False):
  v=self.response(self.request(m,p))
  if expect_error:check('error'in v);return v
  check('result'in v,str(v));return v['result']
 def open(self,s,command):
  self.rev[s]=0;v=self.call('session.open',{'sessionId':s,'commandName':command,'searchText':None});self.views[s]=v;return v
 def raw_action(self,s,id,values=None):return self.call('session.call',{'sessionId':s,'operation':'extension:invoke-action','payload':{'actionId':id,'formValues':values or {}}})
 def action(self,s,title,values=None,view=None):
  v=view or self.views[s];a=next(a for a in v['actions']if a['title']==title);values=dict(values or {})
  if v['root']['type']=='Form':self.rev[s]+=1;values['__formRevision']=self.rev[s]
  result=self.raw_action(s,a['id'],values)
  if result['mode']=='view':self.views[s]=result
  return result
 def edit(self,s,text,revision=None):
  v=self.views[s];self.rev[s]=max(self.rev[s]+1,revision or 0);revision=revision or self.rev[s]
  field=v['root']['children'][0]['props']['actionId']
  return self.raw_action(s,field,{'value':text,'__formRevision':revision})
 def refresh(self,s):
  v=self.call('session.call',{'sessionId':s,'operation':'extension:refresh-session','payload':{}})
  if v['mode']=='view':self.views[s]=v
  return v
 def wait(self,s,timeout=5):
  deadline=time.monotonic()+timeout
  while time.monotonic()<deadline:
   v=self.refresh(s)
   if v['mode']=='view'and not v['root'].get('props',{}).get('isLoading',False):return v
   time.sleep(.01)
  raise AssertionError('No completed view')
 def store(self):
  f=self.data/'saved-resumes/storage.json';return json.loads(f.read_text()) if f.exists() else {'records':[]}
 def close(self):
  if self.proc.poll() is None:
   self.call('shutdown',{});self.proc.stdin.close();self.proc.wait(5)
  check(self.proc.returncode==0);check(self.proc.stderr.read()==b'');self.temp.cleanup()

server=Server();thread=threading.Thread(target=server.serve_forever,daemon=True);thread.start();base='http://127.0.0.1:'+str(server.server_port)
try:
 with contextlib.ExitStack() as stack:
  p=Pipe();stack.callback(p.close)
  v=p.open('open','open-resume');check(v['root']['type']=='Form');check(p.store()['records']==[])
  before=len(server.hits);p.action('open','Save URL',{'url':base+'/unsubmitted'});check(p.views['open']['message']=='No URL to save');check(len(server.hits)==before);check(p.store()['records']==[])
  check(p.edit('open','  '+base+'/valid  ')['mode']=='unchanged')
  p.action('open','Open URL',{'url':'  '+base+'/valid  '});v=p.wait('open');check('Synthetic Ada'in v['root']['props']['markdown']);check('<b>retained</b>'in v['root']['props']['markdown']);check(p.store()['records'][0]['url']==base+'/valid');check(p.store()['records'][0]['title']=='Synthetic Ada')
  p.action('open','Unsave URL');check(p.store()['records']==[])
  p.action('open','Save URL');p.wait('open');check(len(p.store()['records'])==1)
  p.action('open','Re-Fetch');p.wait('open')
  p.action('open','Edit URL');p.action('open','Open URL',{'url':base+'/nameless'});p.wait('open');p.action('open','Unsave URL');before=server.hits.count('/nameless');p.action('open','Save URL');p.wait('open');check(server.hits.count('/nameless')==before+1,'extra title fetch');check('title'not in p.store()['records'][0])
  s=p.open('saved','search-resumes');check(s['root']['props']['__hasServerSearch'] is False);check(s['root']['children'][0]['props']['title']=='Untitled Resume');check('subtitle'not in s['root']['children'][0]['props']);check('date'in s['root']['children'][0]['props']['accessories'][0])
  check(p.call('session.call',{'sessionId':'saved','operation':'extension:search-text-changed','payload':{'searchText':'Ada'}})['mode']=='unchanged')
  before=len(p.requests);old=s
  p.action('saved','Copy URL');check(len(p.requests)==before+1);check(p.requests[-1]['method']=='host.clipboard.write');check(p.requests[-1]['params']['text']==base+'/nameless')
  check(p.raw_action('saved',next(a['id'] for a in old['actions']if a['title']=='Copy URL'))['mode']=='unchanged')
  p.action('saved','Copy URL');check(len(p.requests)==before+2,'new deliberate copy')
  p.action('saved','Open in Browser');check(p.requests[-1]['method']=='host.shell.open')
  p.action('saved','View Resume');d=p.wait('saved');check(d['root']['type']=='Detail');check(d['root']['props']['backActionId']);p.action('saved','Copy URL');p.action('saved','Open in Browser');p.action('saved','Delete from Saved');check(p.views['saved']['root']['type']=='Detail');p.action('saved','Back to Saved Resumes');check(p.views['saved']['root']['type']=='List');check(all(r['url']!=base+'/nameless'for r in p.store()['records']))
  # Strict ACK negatives, frozen URL and explicit manual recovery.
  for ack in [None,True,{}, {'ok':False},{'ok':True,'message':None},{'ok':True,'extra':1},{'ok':'true'}]:
   p.ack=ack;p.action('saved','Copy URL');check(any(a['title']=='Retry Copy URL'for a in p.views['saved']['actions']));before=len(p.requests);p.refresh('saved');check(len(p.requests)==before)
   p.ack={'ok':True,'message':'Copied'};p.action('saved','Retry Copy URL');check(p.requests[-1]['params']['text']==base+'/valid')
  # Queue the same ID twice while reverse RPC is awaiting. Either SDK task may win.
  p.auto_ack=False;a=next(a for a in p.views['saved']['actions']if a['title']=='Copy URL');before=len(p.requests)
  ids=[p.request('session.call',{'sessionId':'saved','operation':'extension:invoke-action','payload':{'actionId':a['id']}})for _ in range(2)]
  effect=p.frame();check(effect['type']=='request');p.requests.append(effect);p.send({'type':'response','id':effect['id'],'result':{'ok':True}})
  replies=[p.response(id)['result']['mode']for id in ids];check(sorted(replies)==['unchanged','view']);check(len(p.requests)==before+1);p.auto_ack=True;p.views['saved']=p.call('session.call',{'sessionId':'saved','operation':'extension:refresh-session','payload':{}}) if False else p.views['saved']
  # A previously rendered name must never label the next URL's valid response.
  p.open('names','open-resume');p.action('names','Open URL',{'url':base+'/valid'});p.wait('names');p.action('names','Edit URL');p.action('names','Open URL',{'url':base+'/namedB'});p.wait('names');check(next(r for r in p.store()['records']if r['url']==base+'/namedB')['title']=='Synthetic Bee')
  # Fetch errors preserve editable draft and never retry automatically.
  for route in ['/redirect','/chain5','/text','/text-string','/json-string','/gzip','/deflate']:
   p.open('http'+route,'open-resume');p.action('http'+route,'Open URL',{'url':base+route});check(p.wait('http'+route)['root']['type']=='Detail')
  for route in ['/invalid','/malformed','/null','/large','/depth','/error','/loop','/chain6']:
   sid='bad'+route;p.open(sid,'open-resume');p.action(sid,'Open URL',{'url':base+route});v=p.wait(sid);check(v['root']['type']=='Form');check(v['root']['children'][0]['props']['value']==base+route);check(v.get('message'));before=len(server.hits);p.refresh(sid);check(len(server.hits)==before)
  # Execute locked MIME-parser oracles through the actual production HTTP stack.
  for i,c in enumerate(json.loads((ROOT/'qa/oracles.json').read_text())['mime']):
   sid='mime'+str(i);p.open(sid,'open-resume');p.action(sid,'Open URL',{'url':base+'/mime?type='+urllib.parse.quote(c['type'],safe='')});v=p.wait(sid);check((v['root']['type']=='Detail')==c['json'])
  # A/B/A and invalid intermediate drafts: delayed A cannot save or restore itself.
  v=p.open('race','open-resume');field=v['root']['children'][0]['props']['actionId'];p.action('race','Open URL',{'url':base+'/delayA'});time.sleep(.03)
  for rev,text in [(2,base+'/B'),(3,'invalid'),(4,base+'/delayA'),(5,base+'/valid')]:p.raw_action('race',field,{'value':text,'__formRevision':rev})
  p.raw_action('race',field,{'value':'stale','__formRevision':3});p.refresh('race');p.rev['race']=5
  p.action('race','Open URL',{'url':base+'/valid'});p.wait('race');server.gates['/delayA'].set();time.sleep(.03);check(all(r['url']!=base+'/delayA'for r in p.store()['records']));check(p.views['race']['root']['type']=='Detail')
  # Invalid current field input must revoke the pending job before validation.
  for suffix,value in [('oversize','x'*8193),('nontext',None)]:
   sid='invalid-draft-'+suffix;route='/cancel-observe-'+suffix;started=threading.Event();closed=threading.Event();server.cancel_cases[route]=(started,closed)
   v=p.open(sid,'open-resume');field=v['root']['children'][0]['props']['actionId'];p.action(sid,'Open URL',{'url':base+route});check(started.wait(2))
   result=p.raw_action(sid,field,{'value':value,'__formRevision':2});check(closed.wait(2),'invalid draft must cancel previous HTTP body read');check(result['root']['type']=='Form');check(result.get('message'));check(all(r['url']!=base+route for r in p.store()['records']))
   if suffix=='oversize':
    # A Save running on a Form must also be canceled by an oversized submit.
    p.views[sid]=result;p.rev[sid]=2;started.clear();closed.clear();p.action(sid,'Save URL',{'url':base+route});check(started.wait(2));p.action(sid,'Open URL',{'url':'y'*8193});check(closed.wait(2),'invalid submit must cancel pending title lookup');check(p.views[sid]['root']['type']=='Form');check(all(r['url']!=base+route for r in p.store()['records']))
  # Late autosave cannot overwrite a newer same-URL record from another session.
  p.open('slow-save','open-resume');p.action('slow-save','Open URL',{'url':base+'/same-url'});time.sleep(.02);p.open('new-save','open-resume');p.action('new-save','Open URL',{'url':base+'/same-url'});p.wait('new-save');server.gates['/same-url'].set();old=p.wait('slow-save');check('changed'in old['message']);check(next(r for r in p.store()['records']if r['url']==base+'/same-url')['title']=='Newer record')
  # Stale list deletion and reused URL identity never remove a newer record.
  stale=p.open('stale','search-resumes');p.action('race','Re-Fetch');p.wait('race');row=next(r for r in stale['root']['children']if r.get('props',{}).get('subtitle')==base+'/valid');p.views['stale']=p.raw_action('stale',row['props']['actionIds'][-1]);check('changed'in p.views['stale']['message']);check(any(r['url']==base+'/valid'for r in p.store()['records']))
  # Reused URL identity: an old still-open detail cannot delete the replacement.
  p.open('reuse','search-resumes');row=next(r for r in p.views['reuse']['root']['children']if r.get('props',{}).get('subtitle')==base+'/valid');p.views['reuse']=p.raw_action('reuse',row['props']['actionIds'][0]);p.wait('reuse');p.action('reuse','Delete from Saved');check(p.views['reuse']['root']['type']=='Detail');p.action('race','Re-Fetch');p.wait('race');p.action('reuse','Delete from Saved');check('changed'in p.views['reuse']['message']);check(any(r['url']==base+'/valid'for r in p.store()['records']))
  # Save after editing an unsubmitted draft uses the source's last submitted URL.
  p.open('save-target','open-resume');p.action('save-target','Open URL',{'url':base+'/invalid'});p.wait('save-target');p.edit('save-target',base+'/never-submitted');p.action('save-target','Save URL',{'url':base+'/never-submitted'});p.wait('save-target');check(any(r['url']==base+'/invalid'for r in p.store()['records']));check(all(r['url']!=base+'/never-submitted'for r in p.store()['records']));p.action('save-target','Unsave URL',{'url':base+'/never-submitted'});check(all(r['url']!=base+'/invalid'for r in p.store()['records']))
  # An absent URL created and deleted elsewhere must not be resurrected by old autosave.
  p.open('aba-slow','open-resume');p.action('aba-slow','Open URL',{'url':base+'/same-url-aba'});time.sleep(.02)
  p.open('aba-new','open-resume');p.action('aba-new','Open URL',{'url':base+'/same-url-aba'});p.wait('aba-new')
  p.open('aba-list','search-resumes');p.action('aba-list','Delete');check(all(r['url']!=base+'/same-url-aba'for r in p.store()['records']))
  server.gates['/same-url-aba'].set();stale=p.wait('aba-slow');check(all(r['url']!=base+'/same-url-aba'for r in p.store()['records']),'old autosave must not resurrect a deleted URL');check('changed'in stale.get('message',''),'absent-create-delete ABA must conflict')
  # Explicit save ignores failed title lookup, and invalid saved detail keeps its actions.
  p.open('save-error','open-resume');p.action('save-error','Open URL',{'url':base+'/error'});p.wait('save-error');p.action('save-error','Save URL',{'url':base+'/error'});p.wait('save-error');check(any(r['url']==base+'/error'for r in p.store()['records']))
  p.open('invalid-save','open-resume');p.action('invalid-save','Open URL',{'url':base+'/invalid'});p.wait('invalid-save');p.action('invalid-save','Save URL',{'url':base+'/invalid'});p.wait('invalid-save');p.open('invalid-view','search-resumes');p.action('invalid-view','View Resume');iv=p.wait('invalid-view');check(iv['root']['props']['markdown']=='# Invalid Resume');p.action('invalid-view','Copy URL');p.action('invalid-view','Open in Browser');p.action('invalid-view','Delete from Saved')
  # Disposed network work cannot autosave.
  p.open('dispose','open-resume');p.action('dispose','Open URL',{'url':base+'/delayDispose'});time.sleep(.02);p.call('session.dispose',{'sessionId':'dispose'});server.gates['/delayDispose'].set();time.sleep(.03);check(all(r['url']!=base+'/delayDispose'for r in p.store()['records']))
  p.call('session.call',{'sessionId':'race','operation':'invented','payload':{}},expect_error=True)
  check(p.raw_action('race','foreign-id')['mode']=='unchanged')
  # Saved-list empty action navigates internally to a fully working form.
  empty=Pipe();stack.callback(empty.close);empty.open('e','search-resumes');empty.action('e','Open Resume');check(empty.views['e']['root']['type']=='Form');empty.action('e','Open URL',{'url':base+'/valid'});empty.wait('e');check(empty.store()['records'][0]['title']=='Synthetic Ada')
  # Corrupt storage is surfaced; bytes survive actions.
  bad=Pipe();stack.callback(bad.close);d=bad.data/'saved-resumes';d.mkdir();(d/'storage.json').write_bytes(b'{synthetic corrupt');bad.open('x','open-resume');check('corrupt'in bad.views['x']['message']);bad.action('x','Save URL',{'url':base+'/valid'});check((d/'storage.json').read_bytes()==b'{synthetic corrupt')
  # Cancellation after adapter return but before open response commit, repeatedly.
  cap=Pipe();stack.callback(cap.close)
  for i in range(140):
   sid='cancel'+str(i);id=cap.request('session.open',{'sessionId':sid,'commandName':'open-resume','searchText':None});cap.send({'type':'cancel','requestId':id,'reason':'owned QA'});reply=cap.response(id)
   # A delivered successful open is deliberately disposed; canceled opens need no dispose.
   if 'result'in reply:cap.call('session.dispose',{'sessionId':sid})
  for i in range(128):cap.open('live'+str(i),'open-resume')
  cap.call('session.open',{'sessionId':'overflow','commandName':'open-resume','searchText':None},expect_error=True)
  # Saturate the actual SDK ordinary queue and prove reserved disposal still works.
  queued=Pipe();stack.callback(queued.close);queued.open('seed','open-resume');queued.action('seed','Open URL',{'url':base+'/valid'});queued.wait('seed');ids=[]
  for i in range(124):
   sid='queue'+str(i);v=queued.open(sid,'search-resumes');a=next(a for a in v['actions']if a['title']=='Copy URL');ids.append((sid,a['id']))
  queued.auto_ack=False;pending=[queued.request('session.call',{'sessionId':sid,'operation':'extension:invoke-action','payload':{'actionId':aid}})for sid,aid in ids];effects=[];rejected=0
  while len(effects)<120 or rejected<4:
   frame=queued.frame()
   if frame['type']=='request':effects.append(frame)
   elif frame['type']=='response':queued.responses[frame['id']]=frame;rejected+=1;check('busy'in frame['error']['message'].lower())
  check(len(effects)==120);queued.call('session.dispose',{'sessionId':ids[0][0]})
  for id in pending:queued.send({'type':'cancel','requestId':id})
  for id in pending:check('error'in queued.response(id))
  queued.auto_ack=True
  # Pending effect cancellation retains a manual-only frozen retry.
  cancel=Pipe();stack.callback(cancel.close);cancel.open('o','open-resume');cancel.action('o','Open URL',{'url':base+'/valid'});cancel.wait('o');cancel.auto_ack=False;a=next(a for a in cancel.views['o']['actions']if a['title']=='Open Source URL');id=cancel.request('session.call',{'sessionId':'o','operation':'extension:invoke-action','payload':{'actionId':a['id']}});req=cancel.frame();check(req['type']=='request');cancel.send({'type':'cancel','requestId':id});check('error'in cancel.response(id));cancel.auto_ack=True;cancel.refresh('o');check(any(a['title']=='Retry Open URL'for a in cancel.views['o']['actions']));cancel.action('o','Retry Open URL');check(cancel.requests[-1]['params']['target']==base+'/valid')
  # Two real SDK processes share private storage without losing unrelated updates.
  first=Pipe();stack.callback(first.close);second=Pipe(shared_data=first.data);stack.callback(second.close)
  first.open('a','open-resume');second.open('b','open-resume');first.action('a','Open URL',{'url':base+'/delayProcessA'});second.action('b','Open URL',{'url':base+'/delayProcessB'});time.sleep(.03);server.gates['/delayProcessA'].set();first.wait('a');server.gates['/delayProcessB'].set();second.wait('b');check({r['url']for r in first.store()['records']}=={base+'/delayProcessA',base+'/delayProcessB'});check((first.data/'saved-resumes/storage.json').stat().st_mode&0o777==0o600);check((first.data/'saved-resumes').stat().st_mode&0o777==0o700)
  # Cross-process absence ABA, including history from an earlier deletion.
  for preexisting in [False,True]:
   route='/aba-process-'+str(preexisting);url=base+route
   if preexisting:
    second.open('seed','open-resume');second.action('seed','Open URL',{'url':url});second.wait('seed');second.open('seed-list','search-resumes');second.action('seed-list','Delete')
   occurrence=server.hits.count(route)+1;gate=threading.Event();server.response_gates[(route,occurrence)]=gate
   first.open('aba','open-resume');first.action('aba','Open URL',{'url':url})
   deadline=time.monotonic()+2
   while server.hits.count(route)<occurrence and time.monotonic()<deadline:time.sleep(.005)
   check(server.hits.count(route)==occurrence,'delayed process A entered HTTP fixture')
   second.open('new','open-resume');second.action('new','Open URL',{'url':url});second.wait('new');second.open('new-list','search-resumes');second.action('new-list','Delete')
   before=(first.data/'saved-resumes/storage.json').read_bytes();gate.set();v=first.wait('aba');check('changed'in v.get('message',''));check(all(r['url']!=url for r in first.store()['records']));check((first.data/'saved-resumes/storage.json').read_bytes()==before,'late autosave preserves exact deleted store bytes')
   first.action('aba','Save URL');first.wait('aba');check(next(r for r in first.store()['records']if r['url']==url)['title']=='Synthetic Ada','new explicit Save resolves conflict')
   first.call('session.dispose',{'sessionId':'aba'});second.call('session.dispose',{'sessionId':'new'});second.call('session.dispose',{'sessionId':'new-list'})
  # Existing same-record update and stale deletion after URL reuse across real processes.
  route='/aba-process-update';url=base+route;second.open('update','open-resume');second.action('update','Open URL',{'url':url});second.wait('update')
  occurrence=server.hits.count(route)+1;gate=threading.Event();server.response_gates[(route,occurrence)]=gate;first.open('update-slow','open-resume');first.action('update-slow','Open URL',{'url':url})
  deadline=time.monotonic()+2
  while server.hits.count(route)<occurrence and time.monotonic()<deadline:time.sleep(.005)
  check(server.hits.count(route)==occurrence);second.action('update','Re-Fetch');second.wait('update');before=(first.data/'saved-resumes/storage.json').read_bytes();gate.set();v=first.wait('update-slow');check('changed'in v.get('message',''));check((first.data/'saved-resumes/storage.json').read_bytes()==before)
  stale=first.open('reuse-cross','search-resumes');row=next(r for r in stale['root']['children']if r.get('props',{}).get('subtitle')==url);old_id=next(r['id']for r in first.store()['records']if r['url']==url);second.action('update','Unsave URL');second.action('update','Save URL');second.wait('update');check(next(r['id']for r in first.store()['records']if r['url']==url)!=old_id);before=(first.data/'saved-resumes/storage.json').read_bytes();v=first.raw_action('reuse-cross',row['props']['actionIds'][-1]);check('changed'in v.get('message',''));check((first.data/'saved-resumes/storage.json').read_bytes()==before)
  # A conflict after observing a present record must expose a deliberate Save even
  # when the session still has the old saved-list snapshot.
  route='/aba-process-present-delete';url=base+route;second.open('present-new','open-resume');second.action('present-new','Open URL',{'url':url});second.wait('present-new')
  occurrence=server.hits.count(route)+1;gate=threading.Event();server.response_gates[(route,occurrence)]=gate;first.open('present-slow','open-resume');first.action('present-slow','Open URL',{'url':url})
  deadline=time.monotonic()+2
  while server.hits.count(route)<occurrence and time.monotonic()<deadline:time.sleep(.005)
  check(server.hits.count(route)==occurrence);second.action('present-new','Unsave URL');before=(first.data/'saved-resumes/storage.json').read_bytes();gate.set();v=first.wait('present-slow');check('changed'in v.get('message',''));check((first.data/'saved-resumes/storage.json').read_bytes()==before);check(any(a['title']=='Save URL'for a in v['actions']),'conflict from a present snapshot must offer deliberate Save URL')
  first.action('present-slow','Save URL');first.wait('present-slow');check(any(r['url']==url for r in first.store()['records']))
  # Explicit title-only Save owns its initial observation across another process deletion.
  url=base+'/aba-process-title';first.open('title','open-resume');first.action('title','Open URL',{'url':url});first.wait('title');second.open('title-new','open-resume');second.action('title-new','Open URL',{'url':url});second.wait('title-new');second.action('title-new','Save URL');second.wait('title-new')
  occurrence=server.hits.count('/aba-process-title')+1;gate=threading.Event();server.response_gates[('/aba-process-title',occurrence)]=gate
  first.action('title','Save URL')
  deadline=time.monotonic()+2
  while server.hits.count('/aba-process-title')<occurrence and time.monotonic()<deadline:time.sleep(.005)
  check(server.hits.count('/aba-process-title')==occurrence);second.open('title-list','search-resumes');second.action('title-list','Delete');before=(first.data/'saved-resumes/storage.json').read_bytes();gate.set();v=first.wait('title');check('changed'in v.get('message',''));check((first.data/'saved-resumes/storage.json').read_bytes()==before);check(all(r['url']!=url for r in first.store()['records']))
  first.action('title','Save URL');first.wait('title');check(any(r['url']==url for r in first.store()['records']),'deliberate title-only Save resolves deletion conflict')
  # Observe socket EOF at the real server when a response-body fetch is disposed.
  observed=Pipe();stack.callback(observed.close);observed.open('socket','open-resume');observed.action('socket','Open URL',{'url':base+'/cancel-observe'});check(server.body_started.wait(2));observed.call('session.dispose',{'sessionId':'socket'});check(server.client_closed.wait(2));check(observed.store()['records']==[])
  # EOF and shutdown cancel owned work without an explicit session dispose.
  eof=Pipe();stack.callback(eof.close);eof.open('eof','open-resume');eof.action('eof','Open URL',{'url':base+'/delayEof'});time.sleep(.02);eof.proc.stdin.close();check(eof.proc.wait(5)==0);server.gates['/delayEof'].set();check(eof.store()['records']==[])
  stopping=Pipe();stack.callback(stopping.close);stopping.open('stop','open-resume');stopping.action('stop','Open URL',{'url':base+'/delayShutdown'});time.sleep(.02);stopping.call('shutdown',{});stopping.proc.stdin.close();check(stopping.proc.wait(5)==0);server.gates['/delayShutdown'].set();check(stopping.store()['records']==[])
  # Entire HTTP deadline is visible and requires a deliberate retry.
  p.open('timeout','open-resume');started=time.monotonic();p.action('timeout','Open URL',{'url':base+'/timeout'});tv=p.wait('timeout',20);check(14<time.monotonic()-started<18);check(tv['root']['type']=='Form' and 'timed out'in tv['message'])
 # Frame/EOF paths run distinct processes with no initialization or effects.
 for payload in [b'{bad\n',b'[]\n',b'{}\r\n',b'x'*(16*1024*1024+1),b'{"type":']:
  proc=subprocess.Popen([str(BINARY)],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,cwd=ROOT,env={'HOME':str(QA/'home'),'TMPDIR':str(QA/'tmp')});out,err=proc.communicate(payload,timeout=10);check(proc.returncode!=0);check(b'hello'in out)
 proc=subprocess.Popen([str(BINARY)],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,cwd=ROOT,env={'HOME':str(QA/'home'),'TMPDIR':str(QA/'tmp')});out,err=proc.communicate(b'',timeout=5);check(proc.returncode==0)
 print(json.dumps({'checks':checks,'ownedHttpRequests':len(server.hits),'binarySha256':hashlib.sha256(BINARY.read_bytes()).hexdigest(),'suite':'production JSONL; both commands; effects; races; cancellation; 140 canceled +128 live; frames/EOF/shutdown'}))
finally:
 for gate in server.gates.values():gate.set()
 server.shutdown();server.server_close()
