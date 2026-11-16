import json, os, sys
from PIL import Image, ImageChops
import sys
d=sys.argv[1] if len(sys.argv)>1 else 'build/shots'; ref=sys.argv[2] if len(sys.argv)>2 else '../docs/images/components'
rows=[]
for r in json.load(open(f'{d}/index.json')):
    p=f"{ref}/{r['name']}.png"
    if not os.path.exists(p): continue
    a=Image.frombytes('RGBA',(r['w'],r['h']),open(f"{d}/{r['name']}.rgba",'rb').read())
    b=Image.open(p).convert('RGBA')
    if a.size!=b.size: rows.append((r['name'],'SIZE',a.size,b.size)); continue
    diff=ImageChops.difference(a.convert('RGB'),b.convert('RGB')).convert('L')
    hist=diff.histogram(); n=a.size[0]*a.size[1]
    big=sum(hist[16:])/n*100; mx=max(i for i,v in enumerate(hist) if v)
    rows.append((r['name'],round(big,3),mx))
rows.sort(key=lambda t: -t[1] if isinstance(t[1],float) else -1e9)
for t in rows[:30]: print(*t)
print('compared',len(rows),'| identical:',sum(1 for t in rows if t[1]==0.0 and t[2]==0),'| <0.1% pixels off by >15:',sum(1 for t in rows if isinstance(t[1],float) and t[1]<0.1))
