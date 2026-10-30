import static dev.smolmc.MinecraftRustServer.*;

import dev.smolmc.MinecraftRustServer;
import java.nio.file.Path;
import java.util.EnumSet;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.TimeUnit;

/**
 * Local operator test helper, not a plugin loader. Start after a PvP server with an explicitly
 * enabled control bridge. /isolate moves the invoking player to another arena; /together returns to
 * the initial arena; /arena_status reports authoritative health/equipment. No credentials or player
 * names are logged.
 */
public final class PvpAcceptance {
  private PvpAcceptance() {}

  public static void main(String[] args) throws Exception {
    try (var server = MinecraftRustServer.attach(Path.of(args[0]))) {
      var home = server.worlds().defaultWorld().get(5, TimeUnit.SECONDS);
      var isolated =
          server
              .worlds()
              .create(
                  server.templates().load("stone_flat").get(5, TimeUnit.SECONDS), Profile.READ_ONLY)
              .get(5, TimeUnit.SECONDS);
      var sub =
          server
              .subscribe(
                  EnumSet.of(
                      EventKind.PLAYER_JOINED,
                      EventKind.COMMAND,
                      EventKind.PLAYER_DAMAGED,
                      EventKind.PLAYER_DIED,
                      EventKind.PLAYER_RESPAWNED),
                  event -> {
                    if (event.kind() == EventKind.PLAYER_JOINED) {
                      server
                          .players()
                          .teleportToSpawn(event.player().handle(), home)
                          .exceptionally(
                              error -> {
                                System.err.println("Join routing rejected: " + error.getMessage());
                                return null;
                              });
                      return;
                    }
                    if (event.kind() != EventKind.COMMAND) {
                      System.out.println(
                          "event="
                              + event.kind()
                              + " entity="
                              + event.player().handle().entityId()
                              + " health="
                              + event.combat().state().health()
                              + " damage="
                              + event.combat().damage());
                      return;
                    }
                    if (event.player() == null) return;
                    var player = event.player().handle();
                    CompletableFuture<?> action;
                    switch (event.text()) {
                      case "isolate" -> action = server.players().teleportToSpawn(player, isolated);
                      case "together" -> action = server.players().teleportToSpawn(player, home);
                      case "arena_status" ->
                          action =
                              server
                                  .players()
                                  .combat(player)
                                  .thenCompose(
                                      state ->
                                          server
                                              .players()
                                              .message(
                                                  player,
                                                  "[Arena] health="
                                                      + state.health()
                                                      + " main="
                                                      + state.mainHandItemId()
                                                      + " off="
                                                      + state.offHandItemId()));
                      default -> {
                        return;
                      }
                    }
                    action.exceptionally(
                        error -> {
                          System.err.println("Rejected test action: " + error.getMessage());
                          return null;
                        });
                  })
              .get(5, TimeUnit.SECONDS);
      for (String command : new String[] {"isolate", "together", "arena_status"})
        server.commands().register(command).get(5, TimeUnit.SECONDS);
      System.out.println("READY PvP acceptance helper");
      System.out.flush();
      server.disconnected().get();
      sub.close();
    }
  }
}
