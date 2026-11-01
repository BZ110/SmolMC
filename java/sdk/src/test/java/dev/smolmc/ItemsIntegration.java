package dev.smolmc;
import java.nio.file.Path;
import static dev.smolmc.MinecraftRustServer.*;
/** Reads a real encrypted fixture's mined tool and combat-worn armor. */
public final class ItemsIntegration {
  private ItemsIntegration() {}
  public static void main(String[] args)throws Exception {
    try(var server=MinecraftRustServer.attach(Path.of(args[0]))) {
      var player=IntegrationTest.get(server.players().list()).stream().filter(p->p.handle().entityId()==1).findFirst().orElseThrow();
      var state=IntegrationTest.get(server.players().inventory(player.handle()));
      IntegrationTest.check(state.slots().size()==46&&state.slots().get(37).itemId()==1047&&state.slots().get(37).damage()==1,"real mining durability snapshot");
      IntegrationTest.check(state.slots().get(5).damage()>0&&state.armor()==2&&state.toughness()==0,"combat-worn equipped armor snapshot");
      IntegrationTest.check(state.slots().get(37).maxDamage()==250,"reference maximum durability");
      boolean immutable=false;try{state.slots().clear();}catch(UnsupportedOperationException expected){immutable=true;}
      IntegrationTest.check(immutable,"immutable inventory slots");
      var stale=new PlayerHandle(player.handle().serverId(),player.handle().entityId(),new java.util.UUID(0,0));
      IntegrationTest.error(Code.STALE_HANDLE,server.players().inventory(stale));
    }
    System.out.println("PASS Java items: real counted/component inventory, armor/durability, immutable snapshots and stale identity rejection");
  }
}
