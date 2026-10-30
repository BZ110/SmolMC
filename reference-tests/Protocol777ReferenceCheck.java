// Original interoperability harness. Uses an operator-owned official client JAR;
// no Mojang implementation, JAR or assets are included in this repository.
import io.netty.buffer.Unpooled;
import java.io.*;
import java.nio.file.*;
import net.minecraft.SharedConstants;
import net.minecraft.core.RegistryAccess;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.network.RegistryFriendlyByteBuf;
import net.minecraft.network.protocol.game.GameProtocols;
import net.minecraft.server.Bootstrap;
import net.minecraft.world.item.Items;

public final class Protocol777ReferenceCheck {
  static <T> net.minecraft.core.Registry<T> materialize(
      net.minecraft.resources.ResourceKey<? extends net.minecraft.core.Registry<T>> key,
      net.minecraft.core.HolderLookup.Provider provider) {
    var registry =
        new net.minecraft.core.MappedRegistry<T>(key, com.mojang.serialization.Lifecycle.stable());
    provider
        .lookupOrThrow(key)
        .listElements()
        .sorted(java.util.Comparator.comparing(h -> h.key().identifier().toString()))
        .forEach(
            h ->
                registry.register(
                    h.key(), h.value(), net.minecraft.core.RegistrationInfo.BUILT_IN));
    return registry.freeze();
  }

  public static void main(String[] args) throws Exception {
    SharedConstants.tryDetectVersion();
    Bootstrap.bootStrap();
    var lookup = net.minecraft.data.registries.VanillaRegistries.createWorldLookup();
    BuiltInRegistries.DATA_COMPONENT_INITIALIZERS
        .build(lookup)
        .forEach(net.minecraft.core.component.DataComponentInitializers.PendingComponents::apply);
    var base = RegistryAccess.fromRegistryOfRegistries(BuiltInRegistries.REGISTRY);
    var dimensions = materialize(net.minecraft.core.registries.Registries.DIMENSION_TYPE, lookup);
    var damages = materialize(net.minecraft.core.registries.Registries.DAMAGE_TYPE, lookup);
    var extra =
        java.util.stream.Stream.of(
            new RegistryAccess.RegistryEntry<>(
                net.minecraft.core.registries.Registries.DIMENSION_TYPE, dimensions),
            new RegistryAccess.RegistryEntry<>(
                net.minecraft.core.registries.Registries.DAMAGE_TYPE, damages));
    var access =
        new RegistryAccess.ImmutableRegistryAccess(
            java.util.stream.Stream.concat(base.registries(), extra));
    var protocol =
        GameProtocols.CLIENTBOUND_TEMPLATE.bind(RegistryFriendlyByteBuf.decorator(access));
    if (BuiltInRegistries.ATTRIBUTE.getId(
            net.minecraft.world.entity.ai.attributes.Attributes.ATTACK_SPEED.value())
        != 5) throw new AssertionError("attack-speed attribute ID mismatch");
    if (BuiltInRegistries.ITEM.getId(Items.IRON_SWORD) != 1045
        || BuiltInRegistries.ITEM.getId(Items.DIAMOND_SWORD) != 1050)
      throw new AssertionError("item registry identity mismatch");
    System.out.println("PASS actual 26.3 item IDs iron_sword=1045 diamond_sword=1050");
    if (BuiltInRegistries.PARTICLE_TYPE.getId(
                net.minecraft.core.particles.ParticleTypes.SWEEP_ATTACK)
            != 77
        || BuiltInRegistries.PARTICLE_TYPE.getId(
                net.minecraft.core.particles.ParticleTypes.DAMAGE_INDICATOR)
            != 14) throw new AssertionError("particle registry mismatch");
    System.out.println("PASS actual particle IDs sweep_attack=77 damage_indicator=14");
    try (var in = new DataInputStream(Files.newInputStream(Path.of(args[0])))) {
      while (in.available() > 0) {
        String expected = in.readUTF();
        int n = in.readInt();
        byte[] body = in.readNBytes(n);

        var buffer = Unpooled.wrappedBuffer(body);
        if (expected.startsWith("Serverbound")) {
          var input = new net.minecraft.network.FriendlyByteBuf(buffer);
          if (input.readVarInt() != 0x29) throw new AssertionError("player action packet ID");
          var decoded =
              net.minecraft.network.protocol.game.ServerboundPlayerActionPacket.STREAM_CODEC.decode(
                  input);
          if (decoded.getAction()
                  != net.minecraft.network.protocol.game.ServerboundPlayerActionPacket.Action
                      .SWAP_ITEM_WITH_OFFHAND
              || buffer.isReadable()) throw new AssertionError("off-hand enum/layout mismatch");
          System.out.println("PASS ServerboundPlayerActionPacket vanilla off-hand action 7");
          continue;
        }
        var decoded = protocol.codec().decode(buffer);
        if (!decoded.getClass().getSimpleName().equals(expected))
          throw new AssertionError(expected + " decoded as " + decoded.getClass().getSimpleName());
        if (buffer.isReadable())
          throw new AssertionError(expected + " trailing bytes " + buffer.readableBytes());
        if (decoded
            instanceof
            net.minecraft.network.protocol.game.ClientboundContainerSetContentPacket inventory) {
          if (inventory.items().size() != 46
              || inventory.items().get(36).getItem() != Items.IRON_SWORD
              || inventory.items().get(9).getItem() != Items.DIAMOND_SWORD
              || inventory.items().get(36).getCount() != 1)
            throw new AssertionError("inventory item/slot mismatch");
        }
        if (decoded instanceof net.minecraft.network.protocol.game.ClientboundSetEntityMotionPacket motion) {
          var expectedMotion = new net.minecraft.world.phys.Vec3(.4,.4,-.4);
          if (motion.movement().subtract(expectedMotion).lengthSqr() > 3e-9)
            throw new AssertionError("velocity semantic decode mismatch: " + motion.movement());
        }
        var encoded = Unpooled.buffer();
        protocol.codec().encode(encoded, decoded);
        byte[] back = new byte[encoded.readableBytes()];
        encoded.readBytes(back);
        if (!java.util.Arrays.equals(body, back))
          throw new AssertionError(expected + " reference re-encode mismatch");
        System.out.println("PASS " + expected + " exact vanilla decode/re-encode");
      }
    }
  }
}
