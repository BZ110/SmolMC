package dev.smolmc;

import static dev.smolmc.MinecraftRustServer.*;

import java.nio.file.Path;
import java.util.*;
import java.util.concurrent.*;

/** Separately killable/freezeable actual JVM sidecar for failure isolation tests. */
public final class FaultClient {
  private FaultClient() {}

  public static void main(String[] args) throws Exception {
    try (var s = MinecraftRustServer.attach(Path.of(args[0]))) {
      IntegrationTest.get(s.commands().register("owned"));
      if (args[1].equals("slow")) {
        var latch = new CountDownLatch(1);
        IntegrationTest.get(
            s.subscribe(
                EnumSet.of(EventKind.WORLD_PARKED),
                e -> {
                  try {
                    latch.await();
                  } catch (InterruptedException ex) {
                    Thread.currentThread().interrupt();
                  }
                }));
      }
      System.out.println("READY");
      System.out.flush();
      if (args[1].equals("slow")) {
        var w = IntegrationTest.get(s.worlds().defaultWorld());
        for (int i = 0; i < 180; i++) {
          try {
            IntegrationTest.get(s.worlds().park(w));
          } catch (ExecutionException e) {
            break;
          }
        }
        Code reason = s.disconnected().get(8, TimeUnit.SECONDS);
        IntegrationTest.check(
            reason == Code.BACKPRESSURE,
            "slow callback closes with explicit backpressure: " + reason);
        System.out.println("PASS bounded slow callback: " + reason);
      } else System.in.read();
    }
  }
}
