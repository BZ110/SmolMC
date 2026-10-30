package dev.smolmc;

import static dev.smolmc.MinecraftRustServer.*;

import java.nio.file.Path;
import java.util.*;
import java.util.concurrent.*;

/** Short loopback control workload; println JSON followed by idle until stdin. */
public final class BridgeBenchmark {
  private BridgeBenchmark() {}

  static double percentile(long[] a, double q) {
    long[] b = a.clone();
    Arrays.sort(b);
    return b[(int) Math.ceil(b.length * q) - 1] / 1000.;
  }

  static String distribution(long[] a) {
    return "{\"p50_us\":"
        + percentile(a, .5)
        + ",\"p95_us\":"
        + percentile(a, .95)
        + ",\"p99_us\":"
        + percentile(a, .99)
        + "}";
  }

  public static void main(String[] args) throws Exception {
    try (var s = MinecraftRustServer.attach(Path.of(args[0]))) {
      var world = IntegrationTest.get(s.worlds().defaultWorld());
      for (int i = 0; i < 100; i++) IntegrationTest.get(s.worlds().resources(world));
      long[] requests = new long[200];
      for (int i = 0; i < requests.length; i++) {
        long t = System.nanoTime();
        IntegrationTest.get(s.worlds().resources(world));
        requests[i] = System.nanoTime() - t;
      }
      var delivered = new ArrayBlockingQueue<Long>(128);
      var sub =
          IntegrationTest.get(
              s.subscribe(
                  EnumSet.of(EventKind.WORLD_PARKED), e -> delivered.add(System.nanoTime())));
      long[] events = new long[100];
      for (int i = 0; i < events.length; i++) {
        long t = System.nanoTime();
        IntegrationTest.get(s.worlds().park(world));
        Long at = delivered.poll(2, TimeUnit.SECONDS);
        if (at == null) throw new AssertionError("event lost");
        events[i] = at - t;
      }
      IntegrationTest.get(sub.unsubscribe());
      long start = System.nanoTime();
      int successes = 0, rejected = 0;
      for (int round = 0; round < 32; round++) {
        List<CompletableFuture<WorldResources>> batch = new ArrayList<>();
        for (int i = 0; i < 32; i++) batch.add(s.worlds().resources(world));
        for (var f : batch) {
          try {
            IntegrationTest.get(f);
            successes++;
          } catch (ExecutionException e) {
            Throwable t = e.getCause();
            if (t instanceof BridgeException b && b.code() == Code.BACKPRESSURE) rejected++;
            else throw e;
          }
        }
      }
      double seconds = (System.nanoTime() - start) / 1e9;
      System.out.println(
          "{\"pid\":"
              + ProcessHandle.current().pid()
              + ",\"requests\":"
              + distribution(requests)
              + ",\"submit_to_event\":"
              + distribution(events)
              + ",\"throughput_successes\":"
              + successes
              + ",\"throughput_rejected\":"
              + rejected
              + ",\"throughput_seconds\":"
              + seconds
              + ",\"successful_requests_per_second\":"
              + (successes / seconds)
              + "}");
      System.out.flush();
      System.in.read();
    }
  }
}
