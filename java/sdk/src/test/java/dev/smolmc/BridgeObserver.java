package dev.smolmc;

import static dev.smolmc.MinecraftRustServer.*;

import java.nio.file.Path;
import java.util.EnumSet;

public final class BridgeObserver {
  private BridgeObserver() {}

  public static void main(String[] args) throws Exception {
    try (var s = MinecraftRustServer.attach(Path.of(args[0]))) {
      IntegrationTest.get(s.subscribe(EnumSet.allOf(EventKind.class), event -> {}));
      System.out.println("READY");
      System.out.flush();
      System.in.read();
    }
  }
}
