import static dev.smolmc.MinecraftRustServer.*;

import dev.smolmc.MinecraftRustServer;
import java.nio.file.Path;
import java.util.*;
import java.util.concurrent.*;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.concurrent.atomic.AtomicReference;

/**
 * Tiny number-guessing minigame: /arena guess N (1..10). A correct guess sends the winner to the
 * separate B lobby; the empty A arena resets for the next round. /arena a and /arena b move between
 * arenas. Operator-only /arena reset rejects occupied games. Includes a buried 1000-block marker.
 * No Rust changes. Run: java -cp target/java/classes TinyArena target/release/smolmc Launch logs
 * (including the game address) remain in the printed directory.
 */
public final class TinyArena {
  private TinyArena() {}

  public static void main(String[] args) throws Exception {
    try (var launched = MinecraftRustServer.launch(Path.of(args[0]))) {
      var s = launched.server();
      var template = s.templates().load("stone_flat").get(5, TimeUnit.SECONDS);
      var a =
          new AtomicReference<>(
              s.worlds().create(template, Profile.EDITABLE).get(5, TimeUnit.SECONDS));
      var b = s.worlds().cloneWorld(a.get()).get(5, TimeUnit.SECONDS);
      var answer = new AtomicInteger(ThreadLocalRandom.current().nextInt(1, 11));
      List<BlockEdit> marker = new ArrayList<>();
      for (int i = 0; i < 1000; i++)
        marker.add(new BlockEdit(new BlockPos(i % 32, 0, i / 32), Material.AIR));
      s.worlds().editBatch(a.get(), marker).get(5, TimeUnit.SECONDS);
      try (var events =
          s.subscribe(
                  EnumSet.of(
                      EventKind.PLAYER_JOINED,
                      EventKind.PLAYER_WORLD_CHANGED,
                      EventKind.PLAYER_LEFT,
                      EventKind.COMMAND),
                  event -> {
                    CompletableFuture<?> action = CompletableFuture.completedFuture(null);
                    if (event.kind() == EventKind.PLAYER_JOINED) {
                      action =
                          s.players()
                              .teleportToSpawn(event.player().handle(), a.get())
                              .thenCompose(
                                  v ->
                                      s.players()
                                          .message(
                                              event.player().handle(),
                                              "[Arena] Guess 1..10 with /arena guess N; a correct"
                                                  + " guess wins."));
                    }
                    if (event.kind() == EventKind.PLAYER_WORLD_CHANGED
                        || event.kind() == EventKind.PLAYER_LEFT) {
                      WorldHandle round = a.get();
                      action =
                          s.worlds()
                              .resources(round)
                              .thenCompose(
                                  resources -> {
                                    if (resources.players() != 0)
                                      return CompletableFuture.completedFuture(null);
                                    return s.worlds()
                                        .reset(round)
                                        .thenAccept(
                                            fresh -> {
                                              a.set(fresh);
                                              answer.set(
                                                  ThreadLocalRandom.current().nextInt(1, 11));
                                            });
                                  });
                    }
                    if (event.kind() == EventKind.COMMAND) {
                      if (event.text().equals("arena reset") && event.player() == null)
                        action = s.worlds().reset(a.get()).thenAccept(a::set);
                      else if (event.player() != null) {
                        var player = event.player().handle();
                        if (event.text().startsWith("arena guess ")
                            && event.world().equals(a.get())) {
                          int guess;
                          try {
                            guess = Integer.parseInt(event.text().substring(12));
                          } catch (NumberFormatException ex) {
                            guess = -1;
                          }
                          if (guess == answer.get())
                            action =
                                s.players()
                                    .message(player, "[Arena] You won! Moving to the B lobby.")
                                    .thenCompose(v -> s.players().teleportToSpawn(player, b));
                          else
                            action =
                                s.players()
                                    .message(player, "[Arena] Try another number from 1..10.");
                        } else
                          action =
                              s.players()
                                  .teleportToSpawn(
                                      player, event.text().equals("arena b") ? b : a.get());
                      }
                    }
                    action.exceptionally(
                        error -> {
                          System.err.println("Arena operation rejected: " + error.getMessage());
                          return null;
                        });
                  })
              .get(5, TimeUnit.SECONDS)) {
        s.commands().register("arena").get(5, TimeUnit.SECONDS);
        System.out.println(
            "TinyArena ready. Logs: " + launched.directory() + "; press Enter to stop.");
        System.in.read();
        s.commands().unregister("arena").get(5, TimeUnit.SECONDS);
        events.unsubscribe().get(5, TimeUnit.SECONDS);
      }
      // Players are disconnected by the owning child shutdown. Occupied worlds
      // cannot be reset/destroyed; ownership is explicit and never auto-evicts.
    }
  }
}
