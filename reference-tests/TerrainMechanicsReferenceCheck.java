// Original test adapter; invokes real collision and mining methods in operator-owned 26.3.
import java.nio.file.*;
import java.util.*;
import net.minecraft.core.*;
import net.minecraft.core.registries.*;
import net.minecraft.world.phys.*;
import net.minecraft.world.phys.shapes.*;
import net.minecraft.world.entity.player.Player;
import net.minecraft.world.level.GameType;
public final class TerrainMechanicsReferenceCheck {
 static class TestPlayer extends Player {TestPlayer(FlatLevel l){super(l,new com.mojang.authlib.GameProfile(UUID.randomUUID(),"terrain-fixture"));}public GameType gameMode(){return GameType.SURVIVAL;} }
 public static void main(String[]args)throws Exception{
  net.minecraft.SharedConstants.tryDetectVersion();net.minecraft.server.Bootstrap.bootStrap();var lookup=net.minecraft.data.registries.VanillaRegistries.createWorldLookup();BuiltInRegistries.DATA_COMPONENT_INITIALIZERS.build(lookup).forEach(net.minecraft.core.component.DataComponentInitializers.PendingComponents::apply);
  var base=RegistryAccess.fromRegistryOfRegistries(BuiltInRegistries.REGISTRY);var dimensions=Protocol777ReferenceCheck.materialize(Registries.DIMENSION_TYPE,lookup);var damages=Protocol777ReferenceCheck.materialize(Registries.DAMAGE_TYPE,lookup);var biomes=Protocol777ReferenceCheck.materialize(Registries.BIOME,lookup);
  var access=new RegistryAccess.ImmutableRegistryAccess(java.util.stream.Stream.concat(base.registries(),java.util.stream.Stream.of(new RegistryAccess.RegistryEntry<>(Registries.DIMENSION_TYPE,dimensions),new RegistryAccess.RegistryEntry<>(Registries.DAMAGE_TYPE,damages),new RegistryAccess.RegistryEntry<>(Registries.BIOME,biomes))));
  var tags=com.google.gson.JsonParser.parseString(Files.readString(Path.of(args[1]))).getAsJsonObject();var bindings=new HashMap<net.minecraft.tags.TagKey<net.minecraft.world.level.block.Block>,List<Holder<net.minecraft.world.level.block.Block>>>();
  for(var e:tags.entrySet()){var holders=new ArrayList<Holder<net.minecraft.world.level.block.Block>>();for(var id:e.getValue().getAsJsonArray())holders.add(BuiltInRegistries.BLOCK.get(id.getAsInt()).orElseThrow());bindings.put(net.minecraft.tags.TagKey.create(Registries.BLOCK,net.minecraft.resources.Identifier.parse(e.getKey())),holders);}
  BuiltInRegistries.BLOCK.prepareTagReload(new net.minecraft.tags.TagLoader.LoadResult<>(Registries.BLOCK,bindings)).apply();
  var level=new FlatLevel(access,dimensions.getOrThrow(net.minecraft.world.level.dimension.BuiltinDimensionTypes.OVERWORLD));var player=new TestPlayer(level);player.setPos(8,64,8);
  var corpus=com.google.gson.JsonParser.parseString(Files.readString(Path.of(args[0]))).getAsJsonObject();int count=0;
  for(var row:corpus.getAsJsonArray("mining")){var r=row.getAsJsonObject();player.setOnGround(r.get("grounded").getAsBoolean());int item=r.get("item").getAsInt();player.getInventory().setSelectedItem(item==0?net.minecraft.world.item.ItemStack.EMPTY:new net.minecraft.world.item.ItemStack(BuiltInRegistries.ITEM.byIdOrThrow(item)));var state=net.minecraft.world.level.block.Block.stateById(r.get("block").getAsInt());float actual=state.getDestroyProgress(player,level,BlockPos.ZERO);double expected=r.get("progress").getAsDouble();if(expected==-1?Float.isFinite(actual):Math.abs(actual-expected)>1e-7)throw new AssertionError("mining "+r+" actual="+actual);count++;}
  var method=net.minecraft.world.entity.Entity.class.getDeclaredMethod("collideWithShapes",Vec3.class,AABB.class,List.class);method.setAccessible(true);int collisionCount=0;
  var shapes=new ArrayList<VoxelShape>();for(int z=-8;z<=8;z++)for(int x=8;x<=24;x++){shapes.add(Shapes.box(x,-1,z,x+1,0,z+1));if(x==16)for(int y=0;y<3;y++)shapes.add(Shapes.box(x,y,z,x+1,y+1,z+1));}
  for(var row:corpus.getAsJsonArray("collisions")){var r=row.getAsJsonObject();var p=vec(r.getAsJsonArray("position"));var wanted=vec(r.getAsJsonArray("wanted"));var expected=vec(r.getAsJsonArray("result"));var box=new AABB(p.x-.3,p.y,p.z-.3,p.x+.3,p.y+1.8,p.z+.3);var actual=(Vec3)method.invoke(null,wanted,box,shapes);if(actual.subtract(expected).lengthSqr()>1e-12)throw new AssertionError("collision "+r+" actual="+actual);collisionCount++;}
  System.out.println("PASS actual 26.3 mechanics: "+count+" tool/hardness/airborne mining vectors and "+collisionCount+" full-block collision vectors");
 }
 static Vec3 vec(com.google.gson.JsonArray v){return new Vec3(v.get(0).getAsDouble(),v.get(1).getAsDouble(),v.get(2).getAsDouble());}
}
