#!/usr/bin/env python3
import http.server,ssl,subprocess,tempfile,threading,pathlib,os,json,sys,uuid
ROOT=pathlib.Path(__file__).resolve().parents[1];QA=ROOT/'.qa'
class Handler(http.server.BaseHTTPRequestHandler):
 def log_message(self,*args):pass
 def do_GET(self):
  body=b'{"basics":{"name":"Synthetic TLS"}}';self.send_response(200);self.send_header('Content-Type','application/json');self.send_header('Content-Length',str(len(body)));self.end_headers();self.wfile.write(body)
with tempfile.TemporaryDirectory(dir=QA,prefix='tls-')as d:
 p=pathlib.Path(d);config=p/'openssl.cnf';config.write_text('[req]\ndistinguished_name=dn\nx509_extensions=ext\nprompt=no\n[dn]\nCN=Owned synthetic fixture\n[ext]\nsubjectAltName=IP:127.0.0.1\nbasicConstraints=critical,CA:FALSE\nkeyUsage=critical,digitalSignature,keyEncipherment\nextendedKeyUsage=serverAuth\n')
 subprocess.run(['openssl','req','-x509','-newkey','rsa:2048','-nodes','-days','2','-config',str(config),'-keyout',str(p/'key.pem'),'-out',str(p/'cert.pem')],check=True,capture_output=True)
 server=http.server.ThreadingHTTPServer(('127.0.0.1',0),Handler);context=ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER);context.load_cert_chain(p/'cert.pem',p/'key.pem');server.socket=context.wrap_socket(server.socket,server_side=True);threading.Thread(target=server.serve_forever,daemon=True).start()
 url='https://127.0.0.1:'+str(server.server_port)+'/'
 try:
  env=os.environ.copy();env['JSON_RESUME_TEST_TLS_URL']=url;env['JSON_RESUME_TEST_TLS_CERT']=str(p/'cert.pem')
  subprocess.run([str(ROOT/'scripts/owned.sh'),'cargo','+1.85.0','test','--locked','--manifest-path',str(ROOT/'Cargo.toml'),'--test','tls','--','--nocapture'],check=True,env=env)
  # Genuine production process independently rejects untrusted TLS. No certificate override exists in product.
  binary=pathlib.Path(sys.argv[1]).resolve() if len(sys.argv)>1 else ROOT/'.qa/target/debug/json-resume-v2';assert binary.is_relative_to(ROOT)
  proc=subprocess.Popen([str(binary)],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,cwd=d,env=env)
  def call(method,params):
   id='h:'+str(uuid.uuid4());proc.stdin.write((json.dumps({'type':'request','id':id,'method':method,'params':params})+'\n').encode());proc.stdin.flush()
   while True:
    v=json.loads(proc.stdout.readline())
    if v.get('id')==id:return v['result']
  assert json.loads(proc.stdout.readline())['type']=='hello'
  call('initialize',{'protocol':1,'extensionId':'com.tezbar.native.json-resume','platform':'macOS','packageDir':str(binary.parent.parent if binary.parent.name=='bin' else ROOT),'dataDir':str(p/'data'),'tempDir':str(p/'temp'),'grantedCapabilities':[]})
  view=call('session.open',{'sessionId':'tls','commandName':'open-resume','searchText':None})
  view=call('session.call',{'sessionId':'tls','operation':'extension:invoke-action','payload':{'actionId':view['actions'][0]['id'],'formValues':{'url':url,'__formRevision':1}}})
  # Event is guaranteed on owned completion; no polling/sleep is needed.
  while json.loads(proc.stdout.readline()).get('event')!='session.invalidated':pass
  view=call('session.call',{'sessionId':'tls','operation':'extension:refresh-session','payload':{}});assert view['root']['type']=='Form';assert 'TLS'in view['message']
  call('shutdown',{});proc.stdin.close();assert proc.wait(5)==0;assert proc.stderr.read()==b''
  print(json.dumps({'ownedTLS':'trusted transport succeeds; untrusted transport and extracted production process reject fixture certificate'}))
 finally:server.shutdown();server.server_close()
