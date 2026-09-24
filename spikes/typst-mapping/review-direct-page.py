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
 paste('可以直接在排版后的正文中输入、选择和修改。\n频率比 $alpha/2 + sqrt(T/rho)$ 决定振动模态。\n$ R = sum_(k in K) [V^*(pi^*) - V(mu^k, pi^*)] $')
 time.sleep(4);shot('direct.png')
 x('mousemove','--window',win,558,433);x('mousedown',1);time.sleep(.2)
 x('mousemove','--window',win,784,433);time.sleep(.4);x('mouseup',1);time.sleep(.5)
 shot('math-selection.png')
 x('key','ctrl+c');time.sleep(.2)
 copied=subprocess.check_output(['xclip','-selection','clipboard','-o'],env=env,text=True)
 print('Selected formula source:',copied,flush=True)
 assert copied == '[V^*(pi^*) - V(mu^k, pi^*)]', copied
 x('type','--delay',25,'x');time.sleep(3);shot('math-edited.png')
 x('mousemove','--window',win,199,327);x('mousedown',1);time.sleep(.2)
 x('mousemove','--window',win,716,327);time.sleep(.4);x('mouseup',1);time.sleep(.5)
 shot('text-selection.png');x('key','ctrl+c');time.sleep(.2)
 copied=subprocess.check_output(['xclip','-selection','clipboard','-o'],env=env,text=True)
 print('Selected Chinese text:',copied,flush=True)
 assert copied == '可以直接在排版后的正文中输入、选择和修改。', copied
 x('key','ctrl+2');time.sleep(1);shot('source.png')
 x('key','ctrl+1');time.sleep(.5);x('windowsize',win,640,480);time.sleep(1);shot('narrow.png')
 print(out/'math-selection.png',flush=True)
finally:
 app.terminate();app.wait(timeout=5)
