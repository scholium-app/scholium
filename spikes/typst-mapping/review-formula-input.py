"""Run with xvfb-run from the repository root; requires xdotool, xclip and magick."""
import os, subprocess, tempfile, time
from pathlib import Path
out=Path(tempfile.mkdtemp(prefix='scholium-direct-review-'))
print(out, flush=True)
env=os.environ.copy();env.pop('WAYLAND_DISPLAY',None)
env.update(WINIT_UNIX_BACKEND='x11', WINIT_X11_SCALE_FACTOR='1', SCHOLIUM_SESSION_FILE=str(out/'session.sqlite'))
app=subprocess.Popen(['target/debug/scholium-app'],env=env,stdout=open(out/'app.log','w'),stderr=subprocess.STDOUT)
def x(*args): return subprocess.check_output(['xdotool',*map(str,args)],env=env,text=True).strip()
def paste(text):
 subprocess.run(['xclip','-selection','clipboard'],input=text.encode(),env=env,check=True,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
 x('key','ctrl+v');time.sleep(.5)
def shot(name): subprocess.run(['magick','import','-window',win,str(out/name)],env=env,check=True)
try:
 for _ in range(100):
  time.sleep(.1)
  result=subprocess.run(['xdotool','search','--onlyvisible','--pid',str(app.pid)],env=env,capture_output=True,text=True)
  if result.returncode==0: break
 win=result.stdout.splitlines()[-1]
 x('windowsize',win,1280,900);x('windowfocus',win);time.sleep(1)
 x('key','ctrl+n');time.sleep(4)
 # Click the actual display-formula toolbar entry.
 x('mousemove','--window',win,300,44);x('click',1);time.sleep(.5)
 x('type','--delay',150,'al');time.sleep(.8);shot('formula-al.png')
 x('type','--delay',150,'pha');time.sleep(.8);shot('formula-alpha.png')
 x('type','--delay',150,'/');time.sleep(.8);shot('formula-incomplete.png')
 x('key','ctrl+j');time.sleep(.5);shot('formula-diagnostic.png')
 x('key','ctrl+j');time.sleep(.2)
 # Restore the page via the normal mode commands, preserving the caret.
 x('key','ctrl+2');time.sleep(.5);x('key','ctrl+1');time.sleep(.5)
 x('type','--delay',150,'2');time.sleep(.8);shot('formula-completed.png')
 x('key','ctrl+2');time.sleep(.5);shot('formula-source.png')
 print(out,flush=True)

finally:
 app.terminate();app.wait(timeout=5)
