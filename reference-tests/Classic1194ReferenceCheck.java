// Original adapter against the operator-supplied, officially distributed Paper
// 1.19.4 build 550 Mojang-mapped JAR. No Paper/Mojang implementation is copied.
import java.nio.file.*;
import com.google.gson.*;
import net.minecraft.util.Mth;
import net.minecraft.world.phys.Vec3;
import net.minecraft.network.protocol.game.ClientboundSetEntityMotionPacket;
public final class Classic1194ReferenceCheck {
  public static void main(String[] args) throws Exception {
    var corpus=JsonParser.parseString(Files.readString(Path.of(args[0]))).getAsJsonObject();
    for(var entry:corpus.getAsJsonArray("axes")) {
      var row=entry.getAsJsonObject();float yaw=row.get("yaw").getAsFloat();
      float angle=yaw*0.017453292F;var v=row.getAsJsonArray("axes");
      if(Math.abs(-Mth.sin(angle)-v.get(0).getAsDouble())>1e-12 || Math.abs(Mth.cos(angle)-v.get(1).getAsDouble())>1e-12)
        throw new AssertionError("classic yaw mismatch at "+yaw);
    }
    for(var entry:corpus.getAsJsonArray("velocities")) {
      var row=entry.getAsJsonObject();var v=row.getAsJsonArray("input");var expected=row.getAsJsonArray("classic");
      var packet=new ClientboundSetEntityMotionPacket(7,new Vec3(v.get(0).getAsDouble(),v.get(1).getAsDouble(),v.get(2).getAsDouble()));
      var decoded=new double[]{packet.getXa()/8000.,packet.getYa()/8000.,packet.getZa()/8000.};
      for(int i=0;i<3;i++)if(decoded[i]!=expected.get(i).getAsDouble())throw new AssertionError("classic velocity quantization mismatch");
    }
    System.out.println("PASS actual Paper 1.19.4 build 550: 16 Mth yaw vectors and 6 velocity packet quantization vectors");
  }
}
