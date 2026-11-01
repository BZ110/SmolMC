# M4B — Protocol and mechanics evidence

Target: installed Java 26.3 / protocol 777, 2026-10-30. Primary observed reference
is the operator-owned runtime/source archive. No reference source bodies or assets
are included. [Original codec/mechanics adapter](../reference-tests/Items777ReferenceCheck.java)
and [runner](../scripts/check_items777_reference.py) retain reproducible checks.

## Observed runtime facts

| Surface | Observed facts / verification |
|---|---|
| Stacks | Count, item registry ID, added/removed component patch; damage component ID 3 and VarInt value. Default maximum stack counts/durabilities come from actual initialized item components. |
| Item spawn | AddEntity `0x01`, actual item entity type 72, UUID, double position, LP motion, rotations/data. |
| Item metadata | SetEntityData `0x65`, item entity field 8, item-stack serializer 7, ordinary component stack, terminator 255. |
| Collection | TakeItemEntity `0x7f`, entity ID, player ID and actual collected count. Partial pickup sends the remaining stack; removal uses `0x4e`. |
| Motion | Existing exact position-sync `0x23` and LP velocity `0x67` codecs, applied to non-player entities. |
| Equipment | `0x68` main/off-hand plus feet/legs/chest/head entries, continuation bits and real damaged stacks. Attributes `0x86` include armor/toughness/resistance and attack speed. |
| Inventory input | Hashed predictions remain bounded and discarded as authority. Actual `handleContainerClick` executes valid slot actions despite mismatched menu state IDs, then broadcasts full authoritative state. |
| Mining | Existing actual action enum 0/1/2/3 is start/direction/abort/stop. Actual game mode supports a delayed completion when an early stop is below the immediate 0.7 threshold. |
| Replacement | Actual supported short/tall grass and large-fern states are replaceable; solid grass blocks are not. Matching tall vegetation halves are removed atomically when replaced. |

`evidence/m4b-reference.json` records exact actual-codec decode, complete consumption
and byte-for-byte re-encoding, plus semantic stack/entity checks. It compares 72
Rust armor outputs to actual `CombatRules`, seven actual durability/break transitions,
gear wear/maximum/resistance facts, 12 actual `LivingEntity.knockback` vectors and
five replacement flags. These primitive checks do not establish complete vanilla
server ticking, rendering, loot-engine equivalence or real-client acceptance.

The first new collection vector exposed an incorrect registration count: `0x7e`
is TagQuery and `0x7f` is TakeItemEntity. The actual-codec gate caught this before
the live arena was started. Failed vectors are not successful client evidence.

## Observed reference sources

Paths are relative to the private `unz/net/minecraft/` archive:

- `network/protocol/game/{GameProtocols,ClientboundAddEntityPacket,ClientboundTakeItemEntityPacket,ClientboundSetEntityDataPacket,ClientboundSetEquipmentPacket,ServerboundContainerClickPacket}`.
- `network/syncher/EntityDataSerializers`, `core/component/DataComponents`.
- `world/entity/item/ItemEntity`: 10-tick default pickup delay, 6000-tick lifetime,
  compatible-component/full-combined-stack merge, pickup delay/youngest age on merge.
- `world/item/{ItemStack,Item,ToolMaterial}` and `world/item/component/{Tool,Weapon}`:
  supported iron pickaxe costs one per applicable block/two per successful attack;
  supported swords cost two per applicable block/one per successful attack.
- `world/item/equipment/ArmorMaterials`, `world/entity/{LivingEntity,player/Player}`,
  `world/damagesource/CombatRules`: armor/toughness, wear and resistance semantics.
- `server/network/ServerGamePacketListenerImpl`, `server/level/ServerPlayerGameMode`,
  `world/item/context/BlockPlaceContext`, `world/level/block/state/BlockBehaviour`.
- Private `data/minecraft/loot_table/blocks/` inputs are read only by the original
  bounded derivation script. The output is identifier/numeric rules, not copied
  loot-table JSON assets. Supported/deferred names are listed in
  `evidence/m4b-loot-catalog.json`.

## Local policies and unverified scope

The supported catalog is 475 items, including M4A block items and the selected
tools/swords plus leather/iron/diamond/Netherite armor. Only default components and
damage patches are mutable. M3's historical knockback equations and landing policy
remain; 26.3 Netherite resistance scales their strength inputs explicitly.

Item motion uses bounded static shape collision, gravity and drag. Full fluid,
fire, general neighboring-block/entity physics, stack ownership targeting and
complete vanilla simulation are deferred. Last departure clears remaining arena
item entities; inventory survives relog only within the running process. No GUI
claim follows from accepting a packet or from these numeric/codecs checks.
