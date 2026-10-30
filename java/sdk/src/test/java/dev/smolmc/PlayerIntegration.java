package dev.smolmc;

import static dev.smolmc.MinecraftRustServer.*;

import java.nio.file.Path;
import java.util.*;
import java.util.concurrent.*;
import java.util.concurrent.atomic.AtomicInteger;

/** Runs against a cfg(test)-only TCP/AES/compression Play fixture. */
public final class PlayerIntegration {
  private PlayerIntegration() {}

  public static void main(String[] args) throws Exception {
    try (var s = MinecraftRustServer.attach(Path.of(args[0]))) {
      var events = new ArrayBlockingQueue<Event>(128);
      var parks = new AtomicInteger();
      var sub =
          IntegrationTest.get(
              s.subscribe(
                  EnumSet.allOf(EventKind.class),
                  event -> {
                    if (event.kind() == EventKind.WORLD_PARKED) parks.incrementAndGet();
                    else events.add(event);
                  }));
      var players = IntegrationTest.get(s.players().list());
      IntegrationTest.check(players.size() == 2, "two players");
      var a = players.stream().filter(p -> p.handle().entityId() == 1).findFirst().orElseThrow();
      var b = players.stream().filter(p -> p.handle().entityId() == 2).findFirst().orElseThrow();
      IntegrationTest.error(Code.OCCUPIED, s.worlds().reset(a.world()));
      IntegrationTest.error(
          Code.STALE_HANDLE,
          s.players()
              .message(
                  new PlayerHandle(s.serverId(), a.handle().entityId(), new UUID(0, 0)), "spoof"));
      IntegrationTest.get(s.commands().register("arena"));
      boolean command = false;
      long end = System.nanoTime() + TimeUnit.SECONDS.toNanos(5);
      while (System.nanoTime() < end && !command) {
        Event e = events.poll(1, TimeUnit.SECONDS);
        if (e != null && e.kind() == EventKind.COMMAND) {
          IntegrationTest.check(
              e.player() != null && e.text().equals("arena b"), "real player command");
          command = true;
        }
      }
      IntegrationTest.check(command, "player command arrived");
      IntegrationTest.get(s.players().teleportToSpawn(a.handle(), b.world()));
      boolean switched = false;
      end = System.nanoTime() + TimeUnit.SECONDS.toNanos(5);
      while (System.nanoTime() < end && !switched) {
        Event e = events.poll(1, TimeUnit.SECONDS);
        switched =
            e != null
                && e.kind() == EventKind.PLAYER_WORLD_CHANGED
                && e.player().handle().equals(a.handle())
                && e.world().equals(b.world());
      }
      IntegrationTest.check(switched, "transfer committed event");
      IntegrationTest.get(s.players().message(a.handle(), "Java arena message"));
      IntegrationTest.get(s.players().message(b.handle(), "Java arena message"));
      IntegrationTest.get(
          s.worlds()
              .editBatch(b.world(), List.of(new BlockEdit(new BlockPos(8, 62, 8), Material.AIR))));
      IntegrationTest.check(
          IntegrationTest.get(s.worlds().readBlocks(a.world(), List.of(new BlockPos(8, 62, 8))))
                  .get(0)
              == Material.STONE,
          "old world isolation");
      var fresh = IntegrationTest.get(s.worlds().reset(a.world()));
      IntegrationTest.error(Code.STALE_HANDLE, s.worlds().resources(a.world()));
      IntegrationTest.get(s.worlds().destroy(fresh));
      // Sustained cold-event traffic alongside two 20Hz moving encrypted clients.
      // One admitted request at a time; Java's ring/callback queues stay bounded.
      var flood =
          IntegrationTest.get(
              s.worlds()
                  .create(
                      IntegrationTest.get(s.templates().load("stone_flat")), Profile.READ_ONLY));
      long floodStart = System.nanoTime();
      for (int i = 0; i < 2000; i++) {
        IntegrationTest.get(s.worlds().park(flood));
        Thread.sleep(1);
      }
      long deliveredDeadline = System.nanoTime() + TimeUnit.SECONDS.toNanos(2);
      while (parks.get() < 2000 && System.nanoTime() < deliveredDeadline) Thread.sleep(1);
      IntegrationTest.check(parks.get() == 2000, "event flood delivered without gaps");
      System.out.println(
          "event_flood_count="
              + parks.get()
              + " seconds="
              + ((System.nanoTime() - floodStart) / 1e9));
      IntegrationTest.get(s.worlds().destroy(flood));
      IntegrationTest.get(s.commands().unregister("arena"));
      IntegrationTest.get(sub.unsubscribe());
    }
    System.out.println(
        "PASS Java player integration: real encrypted Play, slash command, accepted transfer/commit"
            + " event, teleport/chunk/entity reconciliation, messages, stale session, occupied"
            + " rejection, isolated edits");
  }
}
