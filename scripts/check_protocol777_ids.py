#!/usr/bin/env python3
"""Check identifier facts against an operator-supplied private 26.3 source inventory.
Does not copy implementation, credentials or assets. Includes the bundle delimiter
slot, and checks old known packet anchors before trusting any newly extracted ID.
"""
import argparse,re
from pathlib import Path

def main():
 p=argparse.ArgumentParser();p.add_argument('--reference',default='unz');args=p.parse_args()
 source=(Path(args.reference)/'net/minecraft/network/protocol/game/GameProtocols.java').read_text()
 client=source.split('CLIENTBOUND_TEMPLATE =',1)[1].split('    });',1)[0]
 assert 'withBundlePacket(' in client
 names=['CLIENTBOUND_BUNDLE']+re.findall(r'addPacket\(\w+\.(\w+)',client)
 ids={n:i for i,n in enumerate(names)}
 expected={'CLIENTBOUND_SOUND':0x77,'CLIENTBOUND_LEVEL_PARTICLES':0x30,'CLIENTBOUND_UPDATE_ATTRIBUTES':0x86,'CLIENTBOUND_ANIMATE':0x02,'CLIENTBOUND_ADD_ENTITY':1,'CLIENTBOUND_SET_ENTITY_DATA':0x65,'CLIENTBOUND_SYSTEM_CHAT':0x7c,'CLIENTBOUND_COMMANDS':0x10,'CLIENTBOUND_CONTAINER_SET_CONTENT':0x12,'CLIENTBOUND_DAMAGE_EVENT':0x19,'CLIENTBOUND_ENTITY_EVENT':0x22,'CLIENTBOUND_HURT_ANIMATION':0x2b,'CLIENTBOUND_PLAYER_COMBAT_KILL':0x45,'CLIENTBOUND_RESPAWN':0x54,'CLIENTBOUND_SET_ENTITY_MOTION':0x67,'CLIENTBOUND_SET_EQUIPMENT':0x68,'CLIENTBOUND_SET_HEALTH':0x6a,'CLIENTBOUND_SWING_ANIMATION':0x7b}
 for name,value in expected.items():assert ids[name]==value,(name,ids[name],value)
 # Also verify the public code emits those exact packet kinds.
 rust=Path('crates/protocol-777/src/pvp.rs').read_text();functions={'sound':0x77,'particles':0x30,'inventory':0x12,'equipment':0x68,'swing':0x7b,'health':0x6a,'metadata':0x65,'damage':0x19,'hurt':0x2b,'velocity':0x67,'death':0x22,'killed':0x45,'respawn':0x54,'attributes':0x86,'critical':0x02}
 for function,value in functions.items():
  body=rust.split('pub fn '+function+'(',1)[1].split('\npub fn ',1)[0]
  found=re.search(r'Encoder::new\((0x[0-9a-f]+)\)',body)
  assert found and int(found[1],16)==value,(function,found[1]if found else None)
 print(f'PASS protocol 777 clientbound IDs: bundle delimiter included; {len(expected)} reference anchors and {len(functions)} encoder mappings match')
if __name__=='__main__':main()
