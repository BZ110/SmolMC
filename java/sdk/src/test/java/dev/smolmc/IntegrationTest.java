package dev.smolmc;

import static dev.smolmc.MinecraftRustServer.*;

import java.nio.file.*;
import java.util.*;
import java.util.concurrent.*;
import java.util.concurrent.atomic.*;

/** Real JVM -> real Rust integration. No mock implementation or account secrets. */
public final class IntegrationTest {
  private IntegrationTest() {}

  static <T> T get(CompletableFuture<T> f) throws Exception {
    return f.get(8, TimeUnit.SECONDS);
  }

  static void check(boolean value, String message) {
    if (!value) throw new AssertionError(message);
  }

  static void error(Code code, CompletableFuture<?> future) throws Exception {
    try {
      get(future);
      throw new AssertionError("expected " + code);
    } catch (ExecutionException e) {
      Throwable t = e.getCause();
      while (t instanceof CompletionException) t = t.getCause();
      check(
          t instanceof BridgeException && ((BridgeException) t).code() == code, "wrong error " + t);
    }
  }

  public static void main(String[] args) throws Exception {
    WorldHandle old;
    Path parent;
    try (var launched = MinecraftRustServer.launch(Path.of(args[0]))) {
      var s = launched.server();
      parent = launched.directory();
      check(launched.process().isAlive(), "launch");
      check(s.hasControlGrant(), "grant");
      var t = get(s.templates().load("stone_flat"));
      error(Code.UNSUPPORTED_FEATURE, s.templates().load("anvil"));
      var received = new ArrayBlockingQueue<Event>(128);
      var sub =
          get(
              s.subscribe(
                  EnumSet.allOf(EventKind.class),
                  event -> {
                    if (!received.offer(event)) throw new AssertionError("test queue full");
                  }));
      var a = get(s.worlds().create(t, Profile.EDITABLE));
      old = a;
      var b = get(s.worlds().cloneWorld(a));
      List<BlockEdit> edits = new ArrayList<>();
      List<BlockPos> positions = new ArrayList<>();
      for (int i = 0; i < 1000; i++) {
        var p = new BlockPos(i % 32, 0, i / 32);
        positions.add(p);
        edits.add(new BlockEdit(p, Material.AIR));
      }
      get(s.worlds().editBatch(a, edits));
      check(
          get(s.worlds().readBlocks(a, positions)).stream().allMatch(m -> m == Material.AIR),
          "A edits");
      check(
          get(s.worlds().readBlocks(b, positions)).stream().allMatch(m -> m == Material.STONE),
          "B isolation");
      var c = get(s.worlds().cloneWorld(a));
      get(s.worlds().editBatch(c, List.of(new BlockEdit(positions.get(0), Material.STONE))));
      check(
          get(s.worlds().readBlocks(a, List.of(positions.get(0)))).get(0) == Material.AIR,
          "COW source");
      check(get(s.worlds().resources(c)).edits() == 999, "COW clone");
      get(s.worlds().park(b));
      check(get(s.worlds().resources(b)).ownedTimers() == 0, "no idle timer");
      var fresh = get(s.worlds().reset(a));
      error(Code.STALE_HANDLE, s.worlds().resources(a));
      check(get(s.worlds().resources(fresh)).edits() == 0, "reset");
      error(
          Code.INVALID_REQUEST,
          s.worlds().editBatch(b, List.of(new BlockEdit(new BlockPos(0, 63, 0), Material.AIR))));
      var readOnly = get(s.worlds().create(t, Profile.READ_ONLY));
      error(Code.UNSUPPORTED_FEATURE, s.worlds().editBatch(readOnly, edits));
      get(s.commands().register("arena"));
      error(Code.CONFLICT, s.commands().register("arena"));
      get(s.commands().invoke("arena reset"));
      boolean command = false;
      long previous = 0;
      for (Event e; (e = received.poll(1, TimeUnit.SECONDS)) != null; ) {
        check(e.sequence() > previous, "event order");
        previous = e.sequence();
        if (e.kind() == EventKind.COMMAND) command = e.text().equals("arena reset");
      }
      check(command, "command delivered");
      get(sub.unsubscribe());
      get(s.commands().invoke("arena"));
      Thread.sleep(50);
      check(received.isEmpty(), "unsubscribe");
      get(s.commands().unregister("arena"));
      error(Code.INVALID_REQUEST, s.commands().invoke("arena"));
      CompletableFuture<?>[] calls = new CompletableFuture<?>[32];
      for (int i = 0; i < calls.length; i++) calls[i] = s.worlds().resources(b);
      get(CompletableFuture.allOf(calls));
      // A frozen core guarantees outstanding calls cannot complete while we
      // prove the SDK's end-to-end 64-call admission bound.
      List<CompletableFuture<?>> stalled = new ArrayList<>();
      int rejected = 0;
      check(
          new ProcessBuilder("kill", "-STOP", Long.toString(launched.process().pid()))
                  .start()
                  .waitFor()
              == 0,
          "freeze test child");
      try {
        for (int i = 0; i < 100; i++) {
          var f = s.worlds().resources(b);
          if (f.isCompletedExceptionally()) {
            error(Code.BACKPRESSURE, f);
            rejected++;
          } else stalled.add(f);
        }
        check(rejected >= 36, "bounded pending calls");
      } finally {
        check(
            new ProcessBuilder("kill", "-CONT", Long.toString(launched.process().pid()))
                    .start()
                    .waitFor()
                == 0,
            "resume test child");
      }
      get(CompletableFuture.allOf(stalled.toArray(CompletableFuture<?>[]::new)));
      error(
          Code.NOT_FOUND,
          s.players().message(new PlayerHandle(s.serverId(), 123, new UUID(0, 1)), "hello"));
      get(s.worlds().destroy(fresh));
      get(s.worlds().destroy(b));
      get(s.worlds().destroy(c));
      get(s.worlds().destroy(readOnly));
      error(Code.NOT_FOUND, s.worlds().resources(b));
      // Sidecar connection loss releases command ownership, not Rust worlds.
      try (var second = MinecraftRustServer.attach(parent.resolve("ipc"))) {
        get(second.commands().register("recovery"));
      }
      Thread.sleep(50);
      get(s.commands().register("recovery"));
      get(s.commands().unregister("recovery"));
      check(get(s.worlds().list()).size() == 1, "cleanup");
    }
    check(!Files.exists(parent.resolve("ipc")), "bridge files cleaned");
    try (var restart = MinecraftRustServer.launch(Path.of(args[0]))) {
      try {
        get(restart.server().worlds().resources(old));
        throw new AssertionError("old incarnation");
      } catch (BridgeException e) {
        check(e.code() == Code.STALE_HANDLE, "incarnation");
      } catch (ExecutionException e) {
        check(e.getCause() instanceof BridgeException, "incarnation future");
      }
    }
    Thread.sleep(200);
    check(
        Thread.getAllStackTraces().keySet().stream()
            .noneMatch(t -> t.isAlive() && t.getName().startsWith("smolmc-")),
        "owned SDK workers retired");
    System.out.println(
        "PASS Java integration: launch/attach, isolation/1000 edits/COW, lifecycle/stale IDs,"
            + " errors, events/commands, concurrent requests, reconnect, shutdown/restart");
  }
}
