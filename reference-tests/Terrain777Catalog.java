// Original metadata adapter; exports identifiers/numeric/collision facts, not assets or source.
import java.nio.file.*;
import java.util.*;
import net.minecraft.SharedConstants;
import net.minecraft.server.Bootstrap;
import net.minecraft.core.BlockPos;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.world.level.EmptyBlockGetter;
import net.minecraft.world.level.block.Block;

public final class Terrain777Catalog {
  public static void main(String[] args) throws Exception {
    SharedConstants.tryDetectVersion(); Bootstrap.bootStrap();
    var out = new ArrayList<Map<String,Object>>();
    for (var block : BuiltInRegistries.BLOCK) {
      String name=BuiltInRegistries.BLOCK.getKey(block).toString();
      for (var state : block.getStateDefinition().getPossibleStates()) {
        var shape=state.getCollisionShape(EmptyBlockGetter.INSTANCE,BlockPos.ZERO);
        boolean full=Block.isShapeFullBlock(shape);
        if (!shape.isEmpty() && !full && !name.endsWith("_slab") && !name.endsWith("_stairs") && !name.equals("minecraft:snow")) continue;
        if (!state.getFluidState().isEmpty() || state.hasBlockEntity()) continue;
        if (state.getLightEmission()!=0) continue;
        var row=new LinkedHashMap<String,Object>();
        row.put("replaceable",state.canBeReplaced());row.put("hardness",state.getDestroySpeed(EmptyBlockGetter.INSTANCE,BlockPos.ZERO));row.put("requires_tool",state.requiresCorrectToolForDrops());row.put("block_id",BuiltInRegistries.BLOCK.getId(block));row.put("id",Block.getId(state)); row.put("name",name);
        var props=new TreeMap<String,String>();
        state.getValues().forEach(v->{String s=v.toString();int at=s.indexOf('=');props.put(s.substring(0,at),s.substring(at+1));});
        row.put("properties",props);row.put("solid",full);row.put("opacity",state.getLightDampening());
        row.put("item",BuiltInRegistries.ITEM.getId(block.asItem()));row.put("default",state==block.defaultBlockState());
        row.put("friction",block.getFriction());
        row.put("boxes",shape.toAabbs().stream().map(b->new double[]{b.minX,b.minY,b.minZ,b.maxX,b.maxY,b.maxZ}).toList());out.add(row);
      }
    }
    Files.writeString(Path.of(args[0]),new com.google.gson.GsonBuilder().setPrettyPrinting().create().toJson(out));
    System.out.println("Exported bounded candidate metadata for "+out.size()+" states; block_states="+Block.BLOCK_STATE_REGISTRY.size()+" iron_pickaxe="+BuiltInRegistries.ITEM.getId(net.minecraft.world.item.Items.IRON_PICKAXE));
  }
}
