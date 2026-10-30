// Original executable-method interoperability test; no Mojang bodies included.
import net.minecraft.world.phys.*;
import net.minecraft.world.entity.player.Player;
import net.minecraft.world.level.GameType;
import net.minecraft.core.*;
import net.minecraft.core.registries.*;
public class FlatMotionReferenceCheck {
 static class TestPlayer extends Player {
  TestPlayer(FlatLevel l){super(l,new com.mojang.authlib.GameProfile(java.util.UUID.randomUUID(),"math-fixture"));}
  public GameType gameMode(){return GameType.ADVENTURE;}
  public void gameEvent(net.minecraft.core.Holder<net.minecraft.world.level.gameevent.GameEvent> e){}
 }
 public static void main(String[]args)throws Exception{
 net.minecraft.SharedConstants.tryDetectVersion();net.minecraft.server.Bootstrap.bootStrap();
 var lookup=net.minecraft.data.registries.VanillaRegistries.createWorldLookup();
 BuiltInRegistries.DATA_COMPONENT_INITIALIZERS.build(lookup).forEach(net.minecraft.core.component.DataComponentInitializers.PendingComponents::apply);
 var base=RegistryAccess.fromRegistryOfRegistries(BuiltInRegistries.REGISTRY);
 var dimensions=Protocol777ReferenceCheck.materialize(Registries.DIMENSION_TYPE,lookup);
 var damages=Protocol777ReferenceCheck.materialize(Registries.DAMAGE_TYPE,lookup);
 var biomes=Protocol777ReferenceCheck.materialize(Registries.BIOME,lookup);
 var access=new RegistryAccess.ImmutableRegistryAccess(java.util.stream.Stream.concat(base.registries(),java.util.stream.Stream.of(new RegistryAccess.RegistryEntry<>(Registries.DIMENSION_TYPE,dimensions),new RegistryAccess.RegistryEntry<>(Registries.DAMAGE_TYPE,damages),new RegistryAccess.RegistryEntry<>(Registries.BIOME,biomes))));
 var level=new FlatLevel(access,dimensions.getOrThrow(net.minecraft.world.level.dimension.BuiltinDimensionTypes.OVERWORLD));

 for(int cadence:new int[]{10,11,12,13}) {
  var server=new TestPlayer(level);var client=new TestPlayer(level);
  server.setPos(8,64,8);client.setPos(8,64,8);server.setOnGround(true);client.setOnGround(true);
  for(int tick=0;tick<cadence*6;tick++){
   var saved=server.position();server.aiStep();server.setPos(saved);
   client.aiStep();
   var movement=client.position().subtract(server.position());
   if(server.onGround()&&!client.onGround()&&movement.y>0)server.jumpFromGround();
   server.move(net.minecraft.world.entity.MoverType.PLAYER,movement);
   server.setPos(client.position());server.setOnGroundWithMovement(client.onGround(),movement);
   if(tick%cadence==0){
    var old=server.getDeltaMovement();server.knockback(.4000000059604645,0,-1,level.damageSources().generic(),1);
    var sent=server.getDeltaMovement();server.setDeltaMovement(old);client.setDeltaMovement(sent);
    System.out.println("DIAGNOSTIC paired cadence="+cadence+" tick="+tick+" ground="+server.onGround()+" oldY="+old.y+" sentY="+sent.y);
   }
  }
 }

 }
 static Vec3 vector(com.google.gson.JsonArray v){return new Vec3(v.get(0).getAsDouble(),v.get(1).getAsDouble(),v.get(2).getAsDouble());}
 static void close(Vec3 a,Vec3 b,String label){if(a.subtract(b).lengthSqr()>1e-20)throw new AssertionError(label+" expected="+b+" actual="+a);}
}
