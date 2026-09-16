#!/usr/bin/env python3
"""Native keyboard/AT-SPI acceptance; temporarily enables a11y and ydotool, then restores them."""
import subprocess,time,json,os,signal,re
from pathlib import Path
import pyatspi
import sys
root=Path(__file__).resolve().parents[4]
os.chdir(root)
out=Path(sys.argv[1] if len(sys.argv)>1 else '/tmp/scholium-ui-acceptance').resolve()
out.mkdir(parents=True,exist_ok=True)
def call(*args): return subprocess.run(args,capture_output=True,text=True,check=True).stdout
def prop(name,val): call('busctl','--user','set-property','org.a11y.Bus','/org/a11y/bus','org.a11y.Status',name,'b',val)
saved_props={name:call('busctl','--user','get-property','org.a11y.Bus','/org/a11y/bus','org.a11y.Status',name).strip().split()[-1] for name in ['IsEnabled','ScreenReaderEnabled']}
was_active=subprocess.run(['systemctl','--user','is-active','--quiet','ydotool']).returncode==0
process=None
try:
 prop('IsEnabled','true');prop('ScreenReaderEnabled','true')
 call('systemctl','--user','start','ydotool')
 log=open(out/'app.log','w')
 process=subprocess.Popen(['spikes/native-ui/candidate-egui/target/debug/scholium-spike-egui'],stdout=log,stderr=log)
 time.sleep(7)
 windows=json.loads(call('niri','msg','--json','windows'))
 window=next(w for w in windows if w.get('pid')==process.pid)
 (out/'window.json').write_text(json.dumps(window,ensure_ascii=False,indent=2))
 call('niri','msg','action','focus-window','--id',str(window['id']))
 desktop=pyatspi.Registry.getDesktop(0)
 app=next(a for a in desktop if 'scholium-spike-egui' in (a.name or ''))
 def children(): return list(list(app)[0])
 def named(name): return next(n for n in children() if n.name==name)
 def action(node):
  a=node.queryAction(); assert a.nActions; assert a.doAction(0);time.sleep(0.8)
 def focused():
  w=next(w for w in json.loads(call('niri','msg','--json','windows')) if w.get('is_focused'))
  assert w['pid']==process.pid, 'focus lost; no injection'
 def wait_preview():
  deadline=time.monotonic()+20
  while time.monotonic()<deadline:
   if any(re.search(r'预览 revision Some\((\d+)\) / 正文 \1$', n.name or '') for n in children()): break
   time.sleep(0.5)
  else: raise AssertionError('preview did not catch up with body revision')
 entry=next(n for n in children() if n.getRoleName()=='entry' and n.name!='正文结构编辑器')
 entry.queryComponent().grabFocus();time.sleep(0.6);focused()
 call('ydotool','key','29:1','102:1','102:0','29:0');time.sleep(0.3);focused()
 call('ydotool','type','CHECK');time.sleep(1)
 after=next(n for n in children() if n.getRoleName()=='entry' and n.name!='正文结构编辑器').queryText().getText(0,-1)
 (out/'source-after.txt').write_text(after)
 assert 'CHECK' in after, 'source did not receive keyboard text'
 focused();call('ydotool','key','42:1','105:1','105:0','105:1','105:0','42:0');time.sleep(0.4)
 focused();call('ydotool','key','29:1','46:1','46:0','29:0');time.sleep(0.3)
 focused();call('ydotool','key','106:1','106:0','29:1','47:1','47:0','29:0');time.sleep(0.7)
 copied=next(n for n in children() if n.getRoleName()=='entry' and n.name!='正文结构编辑器').queryText().getText(0,-1)
 assert copied.startswith('CHECKCK'), copied
 (out/'clipboard.txt').write_text(copied)
 action(named('应用源码'))
 action(named('启用后台预览'))
 wait_preview()
 body=named('正文结构编辑器').queryText()
 original=body.getText(0,-1)
 assert 'CHECKCK' in original, 'body text unavailable while source focused'
 assert body.setSelection(0,0,original.index('a')+1), 'AT-SPI cross-node selection request failed'
 time.sleep(0.8)
 selection=body.getSelection(0)
 assert tuple(selection)==(0,original.index('a')+1), selection
 focused();call('ydotool','key','29:1','45:1','45:0','29:0');time.sleep(0.8)
 cut=body.getText(0,-1)
 assert cut!=original, 'body cut had no effect'
 focused();call('ydotool','key','29:1','44:1','44:0','29:0');time.sleep(0.8)
 restored=body.getText(0,-1)
 assert restored==original, 'body undo failed'
 (out/'body-selection.json').write_text(json.dumps({'before':original,'selection':list(selection),'after_cut':cut,'after_undo':restored},ensure_ascii=False,indent=2))
 wait_preview()
 (out/'interaction.txt').write_text('source input received via native keyboard; apply and preview actions invoked\n'+'\n'.join(n.name or '' for n in children()))
 nodes=[]
 def walk(node,depth=0):
  record={'depth':depth,'role':node.getRoleName(),'name':node.name,'interfaces':list(node.get_interfaces())}
  try:
   t=node.queryText();record['text']=t.getText(0,-1);record['selections']=t.getNSelections()
  except Exception: pass
  nodes.append(record)
  for child in node: walk(child,depth+1)
 walk(app)
 (out/'a11y.json').write_text(json.dumps(nodes,ensure_ascii=False,indent=2))
 call('niri','msg','action','screenshot-window','--id',str(window['id']),'--path',str(out/'window.png'),'-p','false')
 print('window and AT-SPI captured',process.pid,window['id'])
finally:
 if process:
  process.terminate();process.wait(timeout=5)
 if not was_active: call('systemctl','--user','stop','ydotool')
 for name,value in saved_props.items(): prop(name,value)
