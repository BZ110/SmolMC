#!/usr/bin/env python3
"""Generate supported identifier/shape/mining facts from original actual-runtime adapter output.
Private reference tag JSON is read only to derive supported numeric memberships;
no assets, JSON bodies or Mojang implementation are copied into deliverables.
"""
import argparse,json,re
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('--candidates',type=Path,required=True);p.add_argument('--reference',type=Path,default=Path('unz'));a=p.parse_args()
current=Path('crates/world/src/block_catalog.rs');supported=set(map(int,re.findall(r'StateInfo \{ id: (\d+)',current.read_text())))
rows=[r for r in json.loads(a.candidates.read_text())if r['id']in supported];assert len(rows)==len(supported)
def tag(name,seen=frozenset()):
 if name in seen:raise ValueError('cyclic reference tag')
 data=json.loads((a.reference/'data/minecraft/tags/block'/f'{name}.json').read_text());out=set()
 for v in data['values']:
  if isinstance(v,dict):v=v['id']
  if v.startswith('#'):out|=tag(v.removeprefix('#minecraft:'),seen|{name})
  else:out.add(v)
 return out
names=['mineable/pickaxe','incorrect_for_iron_tool','sword_efficient','sword_instantly_mines']
tags={n:tag(n)for n in names};ids={r['name']:r['block_id']for r in rows}
Path('crates/sessions/data/mining_tags.json').write_text(json.dumps({'minecraft:'+n:sorted(ids[name]for name in tags[n]if name in ids)for n in names},indent=2)+'\n')
lines=['// Protocol-777 identifier/geometry/mining facts; actual runtime adapter and derived supported tags.','// No game assets or implementation bodies. Regenerate with scripts/generate_terrain_catalog.py.','#[rustfmt::skip]','pub static STATES: &[StateInfo] = &[']
for r in sorted(rows,key=lambda r:r['id']):
 props=','.join(k+'='+v for k,v in sorted(r['properties'].items()));boxes='&['+','.join('['+','.join(str(float(v))for v in b)+']'for b in r['boxes'])+']';n=r['name'];hardness=repr(float(r['hardness']))
 fields=f"id: {r['id']}, name: {json.dumps(n)}, properties: {json.dumps(props)}, solid: {str(r['solid']).lower()}, opacity: {r['opacity']}, item: {r['item']}, default: {str(r['default']).lower()}, boxes: {boxes}, hardness: {hardness}, requires_tool: {str(r['requires_tool']).lower()}, pickaxe: {str(n in tags['mineable/pickaxe']).lower()}, iron_correct: {str(n in tags['mineable/pickaxe']and n not in tags['incorrect_for_iron_tool']).lower()}, sword_speed: {340282350000000000000000000000000000000.0 if n in tags['sword_instantly_mines'] else 1.5 if n in tags['sword_efficient'] else 1.0}"
 lines.append('StateInfo { '+fields+' },')
lines.append('];');current.write_text('\n'.join(lines)+'\n');print('Generated',len(rows),'supported states and',len(tags),'bounded mining tags')
