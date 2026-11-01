#!/usr/bin/env python3
"""Generate bounded registry/component and deterministic loot facts, no game assets.
Reads operator-owned reference JSON; rejects loot structures outside this subset.
"""
import json,re,argparse
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('--items',type=Path,required=True);p.add_argument('--reference',type=Path,default=Path('unz'));a=p.parse_args()
data=json.loads(a.items.read_text());items={r['name']:r for r in data['items']}
states=Path('crates/world/src/block_catalog.rs').read_text()
names=set(re.findall(r'name: "([^"]+)"',states))
loot={};sources={}
def condition(c):
 if c is None:return True
 if c=='minecraft:tool/can_silk_touch':return False
 if isinstance(c,dict)and c.get('type')=='minecraft:survives_explosion':return True
 raise ValueError('unsupported condition')
def entry(e):
 if not condition(e.get('condition')):return None
 if e.get('functions'):raise ValueError('functions')
 if e['type']=='minecraft:item':return e['name']
 if e['type']=='minecraft:alternatives':
  for c in e['children']:
   v=entry(c)
   if v:return v
  return None
 raise ValueError('unsupported entry')
for name in sorted(names):
 f=a.reference/'data/minecraft/loot_table/blocks'/f'{name.split(":")[1]}.json'
 if not f.exists():continue
 try:
  d=json.loads(f.read_text());pools=d.get('pools',[])
  if len(pools)!=1:continue
  pool=pools[0]
  if pool.get('rolls')!=1 or pool.get('functions') or len(pool['entries'])!=1 or not condition(pool.get('condition')):continue
  target=entry(pool['entries'][0])
  if target:loot[name]=items[target]['id'];sources[name]=target
 except (ValueError,KeyError,TypeError):pass
gear={n for n in items if (n in ['minecraft:iron_sword','minecraft:diamond_sword','minecraft:iron_pickaxe']or any(n==f'minecraft:{m}_{s}'for m in ['leather','iron','diamond','netherite']for s in ['helmet','chestplate','leggings','boots']))}
allowed=names|gear|set(sources.values());rows=[items[n]for n in sorted(allowed)if n in items and items[n]['id']!=0]
lines=['// Numeric/identifier facts from Items777Catalog; no game assets or implementation.','#[rustfmt::skip]','pub static ITEMS: &[ItemInfo] = &[']
for r in sorted(rows,key=lambda r:r['id']):
 n=r['name'];at=r['attributes'];slot={'HEAD':5,'CHEST':6,'LEGS':7,'FEET':8}.get(r['slot'],0)if n in gear else 0
 state=next((int(m[0])for m in re.findall(r'StateInfo \{ id: (\d+), name: "'+re.escape(n)+r'",[^\n]*?default: (true|false)',states)if m[1]=='true'),-1)
 fields=dict(id=r['id'],name=json.dumps(n),max_count=r['max'],max_damage=r['durability'],armor_slot=slot,armor=float(at.get('minecraft:armor',0)),toughness=float(at.get('minecraft:armor_toughness',0)),resistance=float(at.get('minecraft:knockback_resistance',0)),attack=float(1+at.get('minecraft:attack_damage',0)),speed=round(float(at.get('minecraft:attack_speed',0)),6),mine_cost=r['mine_cost']if n in gear else 0,attack_cost=r['attack_cost']if n in gear else 0,state=state,sword=str(n.endswith('_sword')).lower(),pickaxe=str(n.endswith('_pickaxe')).lower())
 lines.append('ItemInfo { '+', '.join(f'{k}: {v}'for k,v in fields.items())+' },')
lines+= ['];',f'pub const KNOCKBACK_ATTRIBUTE: i32 = {data["knockback_attribute"]};',f'pub const DAMAGE_COMPONENT: i32 = {data["damage_component"]};',f'pub const ITEM_ENTITY: i32 = {data["item_entity"]};',f'pub const ITEM_SERIALIZER: i32 = {data["item_serializer"]};']
Path('crates/protocol-777/src/item_catalog.rs').write_text('\n'.join(lines)+'\n')
Path('crates/sessions/data/block_loot.json').write_text(json.dumps(loot,indent=2)+'\n')
Path('evidence/m4b-loot-catalog.json').write_text(json.dumps(dict(supported=sources,deferred=sorted(names-set(loot)),scope='Derived deterministic single-pool loot without enchantments/explosion; unsupported functions and random/multi-pool rules deferred. Harvest tool checks are separate.'),indent=2)+'\n')
print(len(rows),'item facts;',len(loot),'deterministic block loot rules')
