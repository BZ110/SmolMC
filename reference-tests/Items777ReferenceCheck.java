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

public final class Items777ReferenceCheck {
  static class TestPlayer extends net.minecraft.world.entity.player.Player {
    TestPlayer(FlatLevel level){super(level,new com.mojang.authlib.GameProfile(java.util.UUID.randomUUID(),"items-fixture"));}
    public net.minecraft.world.level.GameType gameMode(){return net.minecraft.world.level.GameType.SURVIVAL;}
  }
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
    var biomes=materialize(net.minecraft.core.registries.Registries.BIOME,lookup);
    var extra =
        java.util.stream.Stream.of(
            new RegistryAccess.RegistryEntry<>(net.minecraft.core.registries.Registries.BIOME,biomes),
            new RegistryAccess.RegistryEntry<>(
                net.minecraft.core.registries.Registries.DIMENSION_TYPE, dimensions),
            new RegistryAccess.RegistryEntry<>(
                net.minecraft.core.registries.Registries.DAMAGE_TYPE, damages));
    var access =
        new RegistryAccess.ImmutableRegistryAccess(
            java.util.stream.Stream.concat(base.registries(), extra));
    if(BuiltInRegistries.ATTRIBUTE.getId(net.minecraft.world.entity.ai.attributes.Attributes.ARMOR.value())!=1||BuiltInRegistries.ATTRIBUTE.getId(net.minecraft.world.entity.ai.attributes.Attributes.ARMOR_TOUGHNESS.value())!=2)throw new AssertionError("armor attribute IDs");
    var protocol =
        GameProtocols.CLIENTBOUND_TEMPLATE.bind(RegistryFriendlyByteBuf.decorator(access));
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
        if(decoded instanceof net.minecraft.network.protocol.game.ClientboundContainerSetContentPacket inv) {
          if(inv.items().get(36).getItem()!=Items.IRON_PICKAXE||inv.items().get(36).getDamageValue()!=249||inv.items().get(9).getCount()!=64||inv.carriedItem().getDamageValue()!=17)throw new AssertionError("stack count/damage semantics");
        }
        if(decoded instanceof net.minecraft.network.protocol.game.ClientboundAddEntityPacket add && add.getType()!=net.minecraft.world.entity.EntityTypes.ITEM)throw new AssertionError("item entity type");
        if(decoded instanceof net.minecraft.network.protocol.game.ClientboundSetEntityDataPacket meta) {
          var value=meta.packedItems().getFirst();if(value.id()!=8||!(value.value() instanceof net.minecraft.world.item.ItemStack stack)||stack.isEmpty())throw new AssertionError("item metadata semantics");
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
    var source=new net.minecraft.world.damagesource.DamageSource(damages.getOrThrow(net.minecraft.world.damagesource.DamageTypes.GENERIC));
    int checks=0;
    var corpus=com.google.gson.JsonParser.parseString(Files.readString(Path.of(args[1]))).getAsJsonObject();
    for(var value:corpus.getAsJsonArray("armor")) {
      var r=value.getAsJsonObject();float actual=net.minecraft.world.damagesource.CombatRules.getDamageAfterAbsorb(null,r.get("damage").getAsFloat(),source,r.get("armor").getAsFloat(),r.get("toughness").getAsFloat());
      if(Float.floatToIntBits(r.get("result").getAsFloat())!=Float.floatToIntBits(actual))throw new AssertionError("Rust armor mismatch "+r+" actual="+actual);checks++;
    }
    for(var value:corpus.getAsJsonArray("gear")) {
      var r=value.getAsJsonObject();var stack=new net.minecraft.world.item.ItemStack(BuiltInRegistries.ITEM.byIdOrThrow(r.get("id").getAsInt()));
      if(stack.getMaxDamage()!=r.get("max").getAsInt()||stack.getMaxStackSize()!=1)throw new AssertionError("gear durability/stack bounds");
      double resistance=stack.get(net.minecraft.core.component.DataComponents.ATTRIBUTE_MODIFIERS).modifiers().stream().filter(e->e.attribute().is(net.minecraft.world.entity.ai.attributes.Attributes.KNOCKBACK_RESISTANCE)).mapToDouble(e->e.modifier().amount()).sum();
      if(Math.abs(resistance-r.get("resistance").getAsDouble())>1e-15)throw new AssertionError("gear resistance");
      var tool=stack.get(net.minecraft.core.component.DataComponents.TOOL);var weapon=stack.get(net.minecraft.core.component.DataComponents.WEAPON);
      if((tool==null?0:tool.damagePerBlock())!=r.get("mine").getAsInt()||(weapon==null?0:weapon.itemDamagePerAttack())!=r.get("attack").getAsInt())throw new AssertionError("gear wear costs");
      if(!r.get("broken").getAsBoolean()||!r.get("empty").getAsBoolean()||r.get("before").getAsInt()!=stack.getMaxDamage()-1)throw new AssertionError("Rust break result");
    }
    var level=new FlatLevel(access,dimensions.getOrThrow(net.minecraft.world.level.dimension.BuiltinDimensionTypes.OVERWORLD));var player=new TestPlayer(level);
    for(var value:corpus.getAsJsonArray("knockback")) {
      var r=value.getAsJsonObject();player.getAttribute(net.minecraft.world.entity.ai.attributes.Attributes.KNOCKBACK_RESISTANCE).setBaseValue(r.get("resistance").getAsDouble());
      var v=r.getAsJsonArray("velocity");player.setDeltaMovement(v.get(0).getAsDouble(),v.get(1).getAsDouble(),v.get(2).getAsDouble());player.setOnGround(r.get("ground").getAsBoolean());
      player.knockback(r.get("power").getAsDouble(),0,-1,source,6,false);var actual=player.getDeltaMovement();var expected=r.getAsJsonArray("result");
      for(int i=0;i<3;i++)if(Math.abs(new double[]{actual.x,actual.y,actual.z}[i]-expected.get(i).getAsDouble())>1e-12)throw new AssertionError("resistance knockback mismatch "+r+" actual="+actual);
    }
    for(var value:corpus.getAsJsonArray("replaceable")){var r=value.getAsJsonObject();if(net.minecraft.world.level.block.Block.stateById(r.get("state").getAsInt()).canBeReplaced()!=r.get("replaceable").getAsBoolean())throw new AssertionError("replaceable flag");}
    System.out.println("PASS actual LivingEntity knockback resistance matches 12 Rust vectors; five grass/solid replacement flags");
    var apply=net.minecraft.world.item.ItemStack.class.getDeclaredMethod("applyDamage",int.class,net.minecraft.server.level.ServerPlayer.class,java.util.function.Consumer.class);apply.setAccessible(true);
    for(var item:new net.minecraft.world.item.Item[]{Items.IRON_PICKAXE,Items.IRON_SWORD,Items.DIAMOND_SWORD,Items.LEATHER_HELMET,Items.IRON_CHESTPLATE,Items.DIAMOND_LEGGINGS,Items.NETHERITE_BOOTS}) {
      var stack=item.getDefaultInstance();int max=stack.getMaxDamage();apply.invoke(stack,max-1,null,(java.util.function.Consumer<net.minecraft.world.item.ItemStack>)(x)->{});
      if(stack.isEmpty()||stack.getDamageValue()!=max-1)throw new AssertionError("durability before break");
      apply.invoke(stack,max,null,(java.util.function.Consumer<net.minecraft.world.item.ItemStack>)(x)->{});if(!stack.isEmpty())throw new AssertionError("break");
    }
    System.out.println("PASS actual armor mitigation "+checks+" vectors and seven actual durability/break transitions");
  }
}
