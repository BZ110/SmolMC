// Original metadata adapter. Exports numeric registry/component facts only.
import java.nio.file.*;
import java.util.*;
import net.minecraft.SharedConstants;
import net.minecraft.server.Bootstrap;
import net.minecraft.core.component.DataComponents;
import net.minecraft.core.registries.BuiltInRegistries;
public final class Items777Catalog {
  public static void main(String[] args) throws Exception {
    SharedConstants.tryDetectVersion(); Bootstrap.bootStrap();
    var lookup=net.minecraft.data.registries.VanillaRegistries.createWorldLookup();
    BuiltInRegistries.DATA_COMPONENT_INITIALIZERS.build(lookup).forEach(net.minecraft.core.component.DataComponentInitializers.PendingComponents::apply);
    var rows=new ArrayList<Map<String,Object>>();
    for(var item:BuiltInRegistries.ITEM) {
      var s=item.getDefaultInstance(); var r=new LinkedHashMap<String,Object>();
      r.put("id",BuiltInRegistries.ITEM.getId(item));r.put("name",BuiltInRegistries.ITEM.getKey(item).toString());
      r.put("max",s.getMaxStackSize());r.put("durability",s.getMaxDamage());
      var eq=s.get(DataComponents.EQUIPPABLE);r.put("slot",eq==null?"":eq.slot().name());
      var tool=s.get(DataComponents.TOOL);r.put("mine_cost",tool==null?0:tool.damagePerBlock());
      var weapon=s.get(DataComponents.WEAPON);r.put("attack_cost",weapon==null?0:weapon.itemDamagePerAttack());
      var attr=s.get(DataComponents.ATTRIBUTE_MODIFIERS);var attrs=new TreeMap<String,Double>();
      if(attr!=null)for(var entry:attr.modifiers())attrs.put(BuiltInRegistries.ATTRIBUTE.getKey(entry.attribute().value()).toString(),entry.modifier().amount());
      r.put("attributes",attrs);rows.add(r);
    }
    var out=new LinkedHashMap<String,Object>();out.put("items",rows);
    out.put("knockback_attribute",BuiltInRegistries.ATTRIBUTE.getId(net.minecraft.world.entity.ai.attributes.Attributes.KNOCKBACK_RESISTANCE.value()));
    out.put("damage_component",BuiltInRegistries.DATA_COMPONENT_TYPE.getId(DataComponents.DAMAGE));
    out.put("item_entity",BuiltInRegistries.ENTITY_TYPE.getId(net.minecraft.world.entity.EntityTypes.ITEM));
    out.put("item_serializer",net.minecraft.network.syncher.EntityDataSerializers.getSerializedId(net.minecraft.network.syncher.EntityDataSerializers.ITEM_STACK));
    Files.writeString(Path.of(args[0]),new com.google.gson.GsonBuilder().setPrettyPrinting().create().toJson(out));
  }
}
