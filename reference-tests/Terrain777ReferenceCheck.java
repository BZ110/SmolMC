// Original adapter: actual 26.3 disk palettes and network sections, no source/assets included.
import java.nio.file.*;
import java.io.*;
import io.netty.buffer.Unpooled;
import net.minecraft.core.RegistryAccess;
import net.minecraft.core.registries.*;
import net.minecraft.network.*;
import net.minecraft.network.protocol.game.*;
import net.minecraft.nbt.*;
import net.minecraft.world.level.chunk.*;
import net.minecraft.world.level.block.Block;
public final class Terrain777ReferenceCheck {
 public static void main(String[] args)throws Exception{
  net.minecraft.SharedConstants.tryDetectVersion();net.minecraft.server.Bootstrap.bootStrap();
  var lookup=net.minecraft.data.registries.VanillaRegistries.createWorldLookup();
  BuiltInRegistries.DATA_COMPONENT_INITIALIZERS.build(lookup).forEach(net.minecraft.core.component.DataComponentInitializers.PendingComponents::apply);
  var biomes=Protocol777ReferenceCheck.materialize(Registries.BIOME,lookup);
  var access=new RegistryAccess.ImmutableRegistryAccess(java.util.stream.Stream.concat(RegistryAccess.fromRegistryOfRegistries(BuiltInRegistries.REGISTRY).registries(),java.util.stream.Stream.of(new RegistryAccess.RegistryEntry<>(Registries.BIOME,biomes))));
  var factory=PalettedContainerFactory.create(access);
  var protocol=GameProtocols.CLIENTBOUND_TEMPLATE.bind(RegistryFriendlyByteBuf.decorator(access));
  var supported=new java.util.HashSet<Integer>();for(String line:Files.readAllLines(Path.of(args[0]).resolve("supported.txt")))supported.add(Integer.parseInt(line));int chunks=0,cells=0;
  for(var file:Files.list(Path.of(args[0])).filter(p->p.toString().endsWith(".wire")).toList()){
   byte[] body=Files.readAllBytes(file);var buf=Unpooled.wrappedBuffer(body);var packet=protocol.codec().decode(buf);
   if(!(packet instanceof ClientboundLevelChunkWithLightPacket c)||buf.isReadable())throw new AssertionError("chunk packet class/trailing");
   var encoded=Unpooled.buffer();protocol.codec().encode(encoded,packet);byte[] back=new byte[encoded.readableBytes()];encoded.readBytes(back);if(!java.util.Arrays.equals(body,back))throw new AssertionError("chunk reencode");
   var disk=NbtIo.read(new DataInputStream(Files.newInputStream(Path.of(file.toString().replace(".wire",".nbt")))),NbtAccounter.create(2097152));
   var diskSections=disk.getListOrEmpty("sections");var network=c.chunkData().getReadBuffer();
   for(int sy=-4;sy<20;sy++){
    var section=new LevelChunkSection(factory);section.read(network);
    net.minecraft.world.level.chunk.PalettedContainer<net.minecraft.world.level.block.state.BlockState> source=null;
    for(int i=0;i<diskSections.size();i++){var t=diskSections.getCompoundOrEmpty(i);if(t.getByteOr("Y",(byte)-99)==sy){var blocks=t.getCompound("block_states");if(blocks.isPresent())source=factory.blockStatesContainerCodec().parse(NbtOps.INSTANCE,blocks.get()).getOrThrow();}}
    for(int y=0;y<16;y++)for(int z=0;z<16;z++)for(int x=0;x<16;x++){
      int absolute=sy*16+y;int expected=source!=null?Block.getId(source.get(x,y,z)):0;if(!supported.contains(expected))expected=0;
      if(Block.getId(section.getBlockState(x,y,z))!=expected)throw new AssertionError("disk/network cell mismatch "+file+" "+sy+" "+x+","+y+","+z);cells++;
    }
   }
   if(network.isReadable())throw new AssertionError("section buffer trailing");chunks++;
  }
  for(var file:Files.list(Path.of(args[0])).filter(p->p.toString().endsWith(".packet")).toList()){
   byte[] body=Files.readAllBytes(file);var buf=Unpooled.wrappedBuffer(body);var packet=protocol.codec().decode(buf);String name=file.getFileName().toString();String expected=name.startsWith("light")?"ClientboundLightUpdatePacket":name.startsWith("cracks")?"ClientboundBlockDestructionPacket":"ClientboundGameEventPacket";
   if(!packet.getClass().getSimpleName().equals(expected)||buf.isReadable())throw new AssertionError("extra packet");var encoded=Unpooled.buffer();protocol.codec().encode(encoded,packet);byte[] back=new byte[encoded.readableBytes()];encoded.readBytes(back);if(!java.util.Arrays.equals(body,back))throw new AssertionError("extra packet reencode");System.out.println("PASS actual 26.3 "+expected);
  }
  for(int i=0;i<4;i++){var buf=new FriendlyByteBuf(Unpooled.wrappedBuffer(Files.readAllBytes(Path.of(args[0]).resolve("dig"+i+".server"))));if(buf.readVarInt()!=0x29)throw new AssertionError("dig ID");var packet=ServerboundPlayerActionPacket.STREAM_CODEC.decode(buf);var names=new String[]{"START_DESTROY_BLOCK","CHANGE_DESTROY_DIRECTION","ABORT_DESTROY_BLOCK","STOP_DESTROY_BLOCK"};if(!packet.getAction().name().equals(names[i])||buf.isReadable())throw new AssertionError("dig enum "+i);}
  System.out.println("PASS actual 26.3 mining start/direction/abort/finish enums 0/1/2/3");
  if(chunks==0)throw new AssertionError("empty corpus");System.out.println("PASS actual 26.3 disk + wire palettes: "+chunks+" chunks, "+cells+" cells, exact packet re-encode");
 }
}
