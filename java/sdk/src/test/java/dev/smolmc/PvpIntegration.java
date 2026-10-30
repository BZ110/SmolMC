package dev.smolmc;

import static dev.smolmc.MinecraftRustServer.*;

import java.nio.file.*;
import java.util.*;
import java.util.concurrent.*;

/** Actual optional SDK against encrypted Play combat, not mocked health/events. */
public final class PvpIntegration {
  private PvpIntegration() {}

  public static void main(String[] args) throws Exception {
    try (var server = MinecraftRustServer.attach(Path.of(args[0]))) {
      var queue = new ArrayBlockingQueue<Event>(32);
      var sub =
          IntegrationTest.get(
              server.subscribe(
                  EnumSet.of(
                      EventKind.PLAYER_DAMAGED, EventKind.PLAYER_DIED, EventKind.PLAYER_RESPAWNED),
                  queue::add));
      var players = IntegrationTest.get(server.players().list());
      IntegrationTest.check(players.size() == 3, "three fixture players");
      var target =
          players.stream().filter(p -> p.handle().entityId() == 3).findFirst().orElseThrow();
      var initial = IntegrationTest.get(server.players().combat(target.handle()));
      IntegrationTest.check(
          initial.health() == 20 && initial.mainHandItemId() == 1045 && !initial.dead(),
          "initial combat snapshot");
      Files.writeString(Path.of(args[1]), "ready");
      boolean hurt = false, dead = false, respawn = false;
      long deadline = System.nanoTime() + TimeUnit.SECONDS.toNanos(15);
      while (System.nanoTime() < deadline && !respawn) {
        var event = queue.poll(1, TimeUnit.SECONDS);
        if (event == null) continue;
        IntegrationTest.check(
            event.combat() != null && event.player() != null, "typed combat event");
        if (!event.player().handle().equals(target.handle())) continue;
        if (event.kind() == EventKind.PLAYER_DAMAGED) {
          hurt = true;
          IntegrationTest.check(
              event.combat().damage() > 0 && event.combat().sourceEntityId() == 1,
              "Rust authoritative damage");
        }
        if (event.kind() == EventKind.PLAYER_DIED) {
          IntegrationTest.check(
              hurt && event.combat().state().dead() && event.combat().state().health() == 0,
              "death snapshot");
          dead = true;
        }
        if (event.kind() == EventKind.PLAYER_RESPAWNED) {
          IntegrationTest.check(
              dead && event.combat().state().health() == 20 && !event.combat().state().dead(),
              "respawn snapshot");
          respawn = true;
        }
      }
      IntegrationTest.check(hurt && dead && respawn, "all gameplay events received");
      var fresh = IntegrationTest.get(server.players().combat(target.handle()));
      IntegrationTest.check(fresh.health() == 20 && fresh.mainHandItemId() == 1045, "fresh state");
      IntegrationTest.get(sub.unsubscribe());
    }
    System.out.println(
        "PASS Java PvP: authoritative health/equipment snapshots, damage/death/respawn events;"
            + " combat progresses without blocking Java");
  }
}
