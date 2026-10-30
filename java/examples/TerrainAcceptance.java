import static dev.smolmc.MinecraftRustServer.*;
import dev.smolmc.MinecraftRustServer;
import java.nio.file.Path;
import java.util.EnumSet;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicReference;
/** Local imported-map acceptance controls; no plugin lifecycle or credentials logged. */
public final class TerrainAcceptance {
 private TerrainAcceptance() {}
 public static void main(String[] args)throws Exception {
  try(var server=MinecraftRustServer.attach(Path.of(args[0]))) {
   var home=new AtomicReference<>(server.worlds().defaultWorld().get(5,TimeUnit.SECONDS));
   var template=server.templates().configured().get(5,TimeUnit.SECONDS);
   var isolated=server.worlds().create(template,Profile.BUILD).get(5,TimeUnit.SECONDS);
   var readonly=server.worlds().create(template,Profile.READ_ONLY).get(5,TimeUnit.SECONDS);
   var sub=server.subscribe(EnumSet.of(EventKind.PLAYER_JOINED,EventKind.COMMAND),event->{
    if(event.player()==null)return;
    var player=event.player().handle();
    if(event.kind()==EventKind.PLAYER_JOINED){server.players().teleportToSpawn(player,home.get());return;}
    CompletableFuture<?> action;
    switch(event.text()){
     case "together" -> action=server.players().teleportToSpawn(player,home.get());
     case "isolate" -> action=server.players().teleportToSpawn(player,isolated);
     case "readonly" -> action=server.players().teleportToSpawn(player,readonly);
     case "fresh" -> action=server.worlds().create(template,Profile.BUILD).thenCompose(w->server.players().teleportToSpawn(player,w));
     case "reset_arena" -> action=server.worlds().reset(home.get()).thenAccept(home::set);
     case "arena_status" -> action=server.players().combat(player).thenCompose(s->server.players().message(player,"[Arena] health="+s.health()+" main="+s.mainHandItemId()+" world="+event.world().id()));
     default -> {return;}
    }
    action.thenCompose(v->server.players().message(player,"[Arena] Done: "+event.text())).exceptionally(e->{server.players().message(player,"[Arena] Rejected (reset requires both players outside home): "+event.text());return null;});
   }).get(5,TimeUnit.SECONDS);
   for(String name:new String[]{"together","isolate","readonly","fresh","reset_arena","arena_status"})server.commands().register(name).get(5,TimeUnit.SECONDS);
   System.out.println("READY imported terrain acceptance helper");System.out.flush();
   server.disconnected().get();sub.close();
  }
 }
}
