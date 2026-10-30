// Original test-only flat-world adapter. Unsupported services fail explicitly.
// Actual movement/knockback methods execute in the operator-owned vanilla JAR.
import java.lang.reflect.*;
class FlatLevel extends net.minecraft.world.level.Level {
FlatLevel(net.minecraft.core.RegistryAccess a, net.minecraft.core.Holder<net.minecraft.world.level.dimension.DimensionType> d){ super(data(), OVERWORLD,a,d,false,false,0,0); }
static net.minecraft.world.level.storage.WritableLevelData data(){return (net.minecraft.world.level.storage.WritableLevelData) Proxy.newProxyInstance(FlatLevel.class.getClassLoader(),new Class[]{net.minecraft.world.level.storage.WritableLevelData.class},(p,m,a)-> {if(m.getName().equals("getDifficulty"))return net.minecraft.world.Difficulty.NORMAL; if(m.getReturnType()==boolean.class)return false;if(m.getReturnType()==long.class)return 0L;if(m.getReturnType()==int.class)return 0;return null;});}
public void playSeededSound(net.minecraft.world.entity.Entity p0,double p1,double p2,double p3,net.minecraft.core.Holder p4,net.minecraft.sounds.SoundSource p5,float p6,float p7,long p8){/* Audio sink is intentionally absent in a numeric physics fixture. */}
public void explode(net.minecraft.world.entity.Entity p0,net.minecraft.world.damagesource.DamageSource p1,net.minecraft.world.level.ExplosionDamageCalculator p2,double p3,double p4,double p5,float p6,boolean p7,net.minecraft.world.level.Level.ExplosionInteraction p8,net.minecraft.core.particles.ParticleOptions p9,net.minecraft.core.particles.ParticleOptions p10,net.minecraft.util.random.WeightedList p11,net.minecraft.core.Holder p12){throw new UnsupportedOperationException("explode");}
public net.minecraft.world.level.chunk.ChunkSource getChunkSource(){throw new UnsupportedOperationException("getChunkSource");}
public void levelEvent(net.minecraft.world.entity.Entity p0,int p1,net.minecraft.core.BlockPos p2,int p3){throw new UnsupportedOperationException("levelEvent");}
public net.minecraft.world.ticks.LevelTickAccess getFluidTicks(){throw new UnsupportedOperationException("getFluidTicks");}
public int getSeaLevel(){throw new UnsupportedOperationException("getSeaLevel");}
public java.util.Collection dragonParts(){throw new UnsupportedOperationException("dragonParts");}
public java.lang.String gatherChunkSourceStats(){throw new UnsupportedOperationException("gatherChunkSourceStats");}
public void sendBlockUpdated(net.minecraft.core.BlockPos p0,net.minecraft.world.level.block.state.BlockState p1,net.minecraft.world.level.block.state.BlockState p2,int p3){throw new UnsupportedOperationException("sendBlockUpdated");}
public net.minecraft.world.clock.ClockManager clockManager(){throw new UnsupportedOperationException("clockManager");}
public net.minecraft.world.level.storage.LevelData.RespawnData getRespawnData(){throw new UnsupportedOperationException("getRespawnData");}
public void gameEvent(net.minecraft.core.Holder p0,net.minecraft.world.phys.Vec3 p1,net.minecraft.world.level.gameevent.GameEvent.Context p2){/* Vibration/audio sink only; no entities listen in this fixture. */}
public net.minecraft.world.level.border.WorldBorder getWorldBorder(){return new net.minecraft.world.level.border.WorldBorder();}
public net.minecraft.world.attribute.EnvironmentAttributeSystem environmentAttributes(){throw new UnsupportedOperationException("environmentAttributes");}
public net.minecraft.core.Holder getUncachedNoiseBiome(int p0,int p1,int p2){throw new UnsupportedOperationException("getUncachedNoiseBiome");}
public void playSeededSound(net.minecraft.world.entity.Entity p0,net.minecraft.world.entity.Entity p1,net.minecraft.core.Holder p2,net.minecraft.sounds.SoundSource p3,float p4,float p5,long p6){/* Audio sink is intentionally absent in a numeric physics fixture. */}
public net.minecraft.world.item.crafting.RecipeAccess recipeAccess(){throw new UnsupportedOperationException("recipeAccess");}
public net.minecraft.world.scores.Scoreboard getScoreboard(){return new net.minecraft.world.scores.Scoreboard();}
public net.minecraft.world.entity.Entity getEntity(int p0){throw new UnsupportedOperationException("getEntity");}
public net.minecraft.world.level.saveddata.maps.MapItemSavedData getMapData(net.minecraft.world.level.saveddata.maps.MapId p0){throw new UnsupportedOperationException("getMapData");}
public void setRespawnData(net.minecraft.world.level.storage.LevelData.RespawnData p0){throw new UnsupportedOperationException("setRespawnData");}
public net.minecraft.world.level.entity.LevelEntityGetter getEntities(){throw new UnsupportedOperationException("getEntities");}
public void destroyBlockProgress(int p0,net.minecraft.core.BlockPos p1,int p2){throw new UnsupportedOperationException("destroyBlockProgress");}
public net.minecraft.world.ticks.LevelTickAccess getBlockTicks(){throw new UnsupportedOperationException("getBlockTicks");}
public net.minecraft.world.flag.FeatureFlagSet enabledFeatures(){throw new UnsupportedOperationException("enabledFeatures");}
public net.minecraft.world.TickRateManager tickRateManager(){throw new UnsupportedOperationException("tickRateManager");}
public java.util.List players(){throw new UnsupportedOperationException("players");}

public int getNextEntityId(){return 1;}
public net.minecraft.world.level.block.state.BlockState getBlockState(net.minecraft.core.BlockPos p){return (p.getY()<64?net.minecraft.world.level.block.Blocks.STONE:net.minecraft.world.level.block.Blocks.AIR).defaultBlockState();}
public net.minecraft.world.level.material.FluidState getFluidState(net.minecraft.core.BlockPos p){return net.minecraft.world.level.material.Fluids.EMPTY.defaultFluidState();}
public java.util.List<net.minecraft.world.phys.shapes.VoxelShape> getEntityCollisions(net.minecraft.world.entity.Entity e,net.minecraft.world.phys.AABB b){return java.util.List.of();}
public net.minecraft.world.level.BlockGetter getChunkForCollisions(int x,int z){return this;}
public boolean hasChunk(int x,int z){return true;}
public java.util.List<net.minecraft.world.entity.Entity> getEntities(net.minecraft.world.entity.Entity e,net.minecraft.world.phys.AABB b,java.util.function.Predicate<? super net.minecraft.world.entity.Entity> p){return java.util.List.of();}
}
