package dev.smolmc;

import java.io.*;
import java.net.*;
import java.nio.*;
import java.nio.channels.*;
import java.nio.charset.*;
import java.nio.file.*;
import java.nio.file.attribute.PosixFilePermission;
import java.time.Duration;
import java.util.*;
import java.util.concurrent.*;
import java.util.concurrent.atomic.*;
import java.util.function.*;

/**
 * Optional local SmolMC SDK 0.2.0 (schema 1.1), Java 17+. Every operation is async. close()
 * disconnects this control peer; launched process ownership is separate. Dependent actions may run
 * on bounded completion workers or the attaching caller (standard CompletableFuture behavior).
 * Never block waiting for another SDK future inside such an action; compose with thenCompose
 * instead.
 */
public final class MinecraftRustServer implements AutoCloseable {
  public static final String API_VERSION = "0.2.0";
  static final int MAX_FRAME = 131072, MAX_PENDING = 64;

  public enum Profile {
    READ_ONLY,
    EDITABLE
  }

  public enum Material {
    AIR,
    STONE
  }

  public enum WorldState {
    Ready,
    Active,
    Parked
  }

  public enum Code {
    INVALID_REQUEST,
    VERSION_MISMATCH,
    PERMISSION_DENIED,
    STALE_HANDLE,
    NOT_FOUND,
    OCCUPIED,
    BUDGET_EXCEEDED,
    UNSUPPORTED_FEATURE,
    BACKPRESSURE,
    CONFLICT,
    WORLD_NOT_READY,
    TIMEOUT,
    DISCONNECTED
  }

  public static final class BridgeException extends RuntimeException {
    private static final long serialVersionUID = 1L;
    private final Code code;

    public BridgeException(Code code, String message) {
      super(message);
      this.code = code;
    }

    public Code code() {
      return code;
    }
  }

  public record Template(UUID serverId, long id, String name) {}

  public record WorldHandle(UUID serverId, long id, long generation) {
    public WorldHandle {
      Objects.requireNonNull(serverId);
      if (id <= 0 || generation <= 0) throw new IllegalArgumentException("invalid world handle");
    }
  }

  /** Session identity, not a persistent player proxy; reconnect gets another entity ID. */
  public record PlayerHandle(UUID serverId, int entityId, UUID uuid) {
    public PlayerHandle {
      Objects.requireNonNull(serverId);
      Objects.requireNonNull(uuid);
      if (entityId <= 0) throw new IllegalArgumentException("invalid entity ID");
    }
  }

  public record Player(PlayerHandle handle, String name, WorldHandle world) {}

  public record BlockPos(int x, int y, int z) {}

  public record BlockEdit(BlockPos position, Material material) {
    public BlockEdit {
      Objects.requireNonNull(position);
      Objects.requireNonNull(material);
    }
  }

  public record WorldResources(
      WorldState state,
      boolean editable,
      long players,
      long edits,
      long modifiedChunks,
      long overlayChargedBytes,
      long ownedTasks,
      long ownedTimers) {}

  public enum EventKind {
    WORLD_CREATED,
    WORLD_CLONED,
    WORLD_RESET,
    WORLD_DESTROYED,
    BLOCKS_EDITED,
    WORLD_PARKED,
    PLAYER_JOINED,
    PLAYER_LEFT,
    PLAYER_WORLD_CHANGED,
    COMMAND,
    PLAYER_DAMAGED,
    PLAYER_DIED,
    PLAYER_RESPAWNED
  }

  /**
   * Immutable cold snapshot. world is null for operator commands; player is null for world/operator
   * events.
   */
  public record Event(
      long sequence,
      EventKind kind,
      WorldHandle world,
      Player player,
      String text,
      CombatChange combat) {
    public Event(long sequence, EventKind kind, WorldHandle world, Player player, String text) {
      this(sequence, kind, world, player, text, null);
    }
  }

  public record CombatState(
      float health, boolean dead, int selectedSlot, int mainHandItemId, int offHandItemId) {}

  public record CombatChange(CombatState state, int sourceEntityId, float damage) {}

  private record Pending(
      int operation,
      CompletableFuture<byte[]> future,
      ScheduledFuture<?> deadline,
      AtomicBoolean received) {}

  private final SocketChannel channel;
  private final UUID serverId;
  private final boolean control;
  private final ConcurrentHashMap<Long, Pending> pending = new ConcurrentHashMap<>();
  private final Semaphore slots = new Semaphore(MAX_PENDING);
  private final ArrayBlockingQueue<ByteBuffer> outbound = new ArrayBlockingQueue<>(MAX_PENDING);
  private final AtomicLong sequence = new AtomicLong(1);
  private final AtomicBoolean closed = new AtomicBoolean();
  private final Object submitLock = new Object();
  private final ThreadPoolExecutor completions = executor(2, 128, "smolmc-complete");
  private final ThreadPoolExecutor callbacks = executor(1, 128, "smolmc-event");
  private final ScheduledThreadPoolExecutor deadlines =
      new ScheduledThreadPoolExecutor(1, factory("smolmc-deadline"));
  private final CompletableFuture<Code> disconnected = new CompletableFuture<>();
  private final AtomicReference<Subscription> subscription = new AtomicReference<>();
  private final Thread reader, writer;

  private static ThreadFactory factory(String name) {
    return r -> {
      Thread t = new Thread(r, name);
      t.setDaemon(true);
      return t;
    };
  }

  private static ThreadPoolExecutor executor(int threads, int queue, String name) {
    return new ThreadPoolExecutor(
        threads,
        threads,
        0,
        TimeUnit.MILLISECONDS,
        new ArrayBlockingQueue<>(queue),
        factory(name),
        new ThreadPoolExecutor.AbortPolicy());
  }

  private MinecraftRustServer(SocketChannel channel, UUID id, boolean control) {
    this.channel = channel;
    this.serverId = id;
    this.control = control;
    deadlines.setRemoveOnCancelPolicy(true);
    reader = factory("smolmc-reader").newThread(this::readLoop);
    writer = factory("smolmc-writer").newThread(this::writeLoop);
    reader.start();
    writer.start();
  }

  /**
   * Blocking startup only (max 5 seconds). Caller must protect the private credential directory.
   */
  public static MinecraftRustServer attach(Path directory) throws IOException {
    Set<PosixFilePermission> perms = Files.getPosixFilePermissions(directory);
    if (perms.stream()
        .anyMatch(p -> p.name().startsWith("GROUP_") || p.name().startsWith("OTHERS_")))
      throw new IOException("bridge directory must be private (0700)");
    Path tokenPath = directory.resolve("token");
    if (!Files.isRegularFile(tokenPath, LinkOption.NOFOLLOW_LINKS)
        || Files.size(tokenPath) != 32
        || Files.isSymbolicLink(tokenPath)
        || Files.getPosixFilePermissions(tokenPath).stream()
            .anyMatch(p -> p.name().startsWith("GROUP_") || p.name().startsWith("OTHERS_")))
      throw new IOException("token must be a private regular file");
    byte[] token;
    try (InputStream input = Files.newInputStream(tokenPath)) {
      token = input.readNBytes(33);
    }
    if (token.length != 32) {
      Arrays.fill(token, (byte) 0);
      throw new IOException("invalid bootstrap credential");
    }
    SocketChannel socket = SocketChannel.open(StandardProtocolFamily.UNIX);
    ScheduledExecutorService timeout =
        Executors.newSingleThreadScheduledExecutor(factory("smolmc-attach-deadline"));
    ScheduledFuture<?> expiry =
        timeout.schedule(
            () -> {
              try {
                socket.close();
              } catch (IOException ignored) {
              }
            },
            5,
            TimeUnit.SECONDS);
    try {
      socket.connect(UnixDomainSocketAddress.of(directory.resolve("socket")));
      byte[] hello =
          encode(
              d -> {
                d.writeByte(1);
                d.writeLong(1);
                d.writeByte(0);
                d.writeShort(1);
                d.writeShort(1);
                d.write(token);
              });
      try {
        writeFrame(socket, ByteBuffer.wrap(hello));
      } finally {
        Arrays.fill(hello, (byte) 0);
        Arrays.fill(token, (byte) 0);
      }
      Wire r = new Wire(readFrame(socket));
      if (r.u8() != 2 || r.i64() != 1 || r.u8() != 0)
        throw new IOException("invalid bridge handshake");
      int status = r.u16();
      if (status != 0) throw error(status, r.string(1024));
      UUID id = r.uuid();
      if (r.u16() != 1 || r.u16() != 1 || !r.string(32).equals(API_VERSION))
        throw new IOException("unsupported SDK/schema version");
      boolean control = r.u8() == 1;
      r.end();
      return new MinecraftRustServer(socket, id, control);
    } catch (IOException | RuntimeException e) {
      socket.close();
      throw e;
    } finally {
      Arrays.fill(token, (byte) 0);
      expiry.cancel(false);
      timeout.shutdownNow();
    }
  }

  /** A launch owns exactly one Rust child, never one process per world. */
  public static LaunchedServer launch(Path executable) throws IOException {
    Path parent = Files.createTempDirectory("smolmc-java-");
    Path directory = parent.resolve("ipc");
    Process child =
        new ProcessBuilder(
                executable.toAbsolutePath().toString(),
                "--bind",
                "127.0.0.1:0",
                "--bridge-dir",
                directory.toString(),
                "--bridge-access",
                "control")
            .redirectOutput(ProcessBuilder.Redirect.DISCARD)
            .redirectError(parent.resolve("server.log").toFile())
            .start();
    try {
      long limit = System.nanoTime() + Duration.ofSeconds(10).toNanos();
      while (!Files.exists(directory.resolve("socket"))) {
        if (!child.isAlive() || System.nanoTime() > limit)
          throw new IOException("Rust startup failed; inspect " + parent.resolve("server.log"));
        try {
          Thread.sleep(10);
        } catch (InterruptedException e) {
          Thread.currentThread().interrupt();
          throw new IOException(e);
        }
      }
      return new LaunchedServer(attach(directory), child, parent);
    } catch (IOException | RuntimeException e) {
      child.destroyForcibly();
      throw e;
    }
  }

  public static final class LaunchedServer implements AutoCloseable {
    private final MinecraftRustServer server;
    private final Process process;
    private final Path directory;

    private LaunchedServer(MinecraftRustServer s, Process p, Path d) {
      server = s;
      process = p;
      directory = d;
    }

    public MinecraftRustServer server() {
      return server;
    }

    public Process process() {
      return process;
    }

    /**
     * Log directory remains available after close for diagnostics; contains no credential after
     * clean shutdown.
     */
    public Path directory() {
      return directory;
    }

    @Override
    public void close() {
      server.close();
      process.destroy();
      try {
        if (!process.waitFor(5, TimeUnit.SECONDS)) {
          process.destroyForcibly();
          process.waitFor(2, TimeUnit.SECONDS);
        }
      } catch (InterruptedException e) {
        Thread.currentThread().interrupt();
        process.destroyForcibly();
      }
    }
  }

  public UUID serverId() {
    return serverId;
  }

  public boolean hasControlGrant() {
    return control;
  }

  public CompletableFuture<Code> disconnected() {
    return disconnected.copy();
  }

  public Templates templates() {
    return new Templates();
  }

  public Worlds worlds() {
    return new Worlds();
  }

  public Players players() {
    return new Players();
  }

  public Commands commands() {
    return new Commands();
  }

  public final class Templates {
    /** Binds the sole built-in immutable descriptor. File/import formats are deferred. */
    public CompletableFuture<Template> load(String name) {
      if (!"stone_flat".equals(name))
        return CompletableFuture.failedFuture(
            new BridgeException(Code.UNSUPPORTED_FEATURE, "only built-in stone_flat is available"));
      return CompletableFuture.completedFuture(new Template(serverId, 1, name));
    }
  }

  public final class Worlds {
    public CompletableFuture<WorldHandle> defaultWorld() {
      return request(1, d -> {}, MinecraftRustServer.this::world);
    }

    public CompletableFuture<WorldHandle> create(Template template, Profile profile) {
      sameServer(template.serverId());
      Objects.requireNonNull(profile);
      return request(
          2,
          d -> {
            d.writeLong(template.id());
            d.writeByte(profile.ordinal());
          },
          MinecraftRustServer.this::world);
    }

    public CompletableFuture<WorldHandle> cloneWorld(WorldHandle w) {
      return request(3, d -> putWorld(d, w), MinecraftRustServer.this::world);
    }

    /** Rejects occupied worlds. Returned generation replaces the old handle. */
    public CompletableFuture<WorldHandle> reset(WorldHandle w) {
      return request(4, d -> putWorld(d, w), MinecraftRustServer.this::world);
    }

    public CompletableFuture<Void> destroy(WorldHandle w) {
      return empty(5, d -> putWorld(d, w));
    }

    public CompletableFuture<Void> park(WorldHandle w) {
      return empty(6, d -> putWorld(d, w));
    }

    public CompletableFuture<WorldResources> resources(WorldHandle w) {
      return request(
          7,
          d -> putWorld(d, w),
          r ->
              new WorldResources(
                  WorldState.valueOf(r.string(32)),
                  r.u8() != 0,
                  r.i64(),
                  r.i64(),
                  r.i64(),
                  r.i64(),
                  r.i64(),
                  r.i64()));
    }

    /** Transactional, at most 4096 buried Air/Stone edits (-64 <= y < 63). */
    public CompletableFuture<Void> editBatch(WorldHandle w, List<BlockEdit> edits) {
      bound(edits.size());
      List<BlockEdit> copy = List.copyOf(edits);
      bound(copy.size());
      return empty(
          8,
          d -> {
            putWorld(d, w);
            d.writeShort(copy.size());
            for (BlockEdit e : copy) {
              putPos(d, e.position());
              d.writeByte(e.material().ordinal());
            }
          });
    }

    public CompletableFuture<List<Material>> readBlocks(WorldHandle w, List<BlockPos> positions) {
      bound(positions.size());
      List<BlockPos> copy = List.copyOf(positions);
      bound(copy.size());
      return request(
          9,
          d -> {
            putWorld(d, w);
            d.writeShort(copy.size());
            for (BlockPos p : copy) putPos(d, p);
          },
          r -> {
            int n = r.count(4096);
            List<Material> result = new ArrayList<>(n);
            for (int i = 0; i < n; i++) {
              int m = r.u8();
              if (m > 1) throw new IOException("invalid material");
              result.add(Material.values()[m]);
            }
            return List.copyOf(result);
          });
    }

    public CompletableFuture<List<WorldHandle>> list() {
      return request(
          18,
          d -> {},
          r -> {
            int n = r.count(4096);
            List<WorldHandle> result = new ArrayList<>(n);
            for (int i = 0; i < n; i++) result.add(world(r));
            return List.copyOf(result);
          });
    }
  }

  public final class Players {
    /** Authoritative cold snapshot. PvP disabled, missing and stale sessions fail explicitly. */
    public CompletableFuture<CombatState> combat(PlayerHandle player) {
      return request(19, d -> putPlayer(d, player), MinecraftRustServer::combatState);
    }

    public CompletableFuture<List<Player>> list() {
      return request(
          10,
          d -> {},
          r -> {
            int n = r.count(512);
            List<Player> result = new ArrayList<>(n);
            for (int i = 0; i < n; i++) result.add(player(r));
            return List.copyOf(result);
          });
    }

    /**
     * Receipt means accepted/coalesced; PLAYER_WORLD_CHANGED means membership committed.
     * Position/terrain delivery and the client ACK follow. A subsequently invalidated target safely
     * closes the player session.
     */
    public CompletableFuture<Void> teleportToSpawn(PlayerHandle p, WorldHandle w) {
      return empty(
          11,
          d -> {
            putPlayer(d, p);
            putWorld(d, w);
          });
    }

    /** Bounded accepted message; receipt is not a client-rendering barrier. */
    public CompletableFuture<Void> message(PlayerHandle p, String text) {
      return empty(
          12,
          d -> {
            putPlayer(d, p);
            putString(d, text, 1024);
          });
    }
  }

  public final class Commands {
    /** Literal name [a-z0-9_]{1,32}, optional greedy arguments; owned by this connection. */
    public CompletableFuture<Void> register(String name) {
      return empty(15, d -> putString(d, name, 32));
    }

    public CompletableFuture<Void> unregister(String name) {
      return empty(16, d -> putString(d, name, 32));
    }

    /**
     * Operator-origin invocation. Player invocations arrive through the unsigned slash-command
     * packet.
     */
    public CompletableFuture<Void> invoke(String command) {
      return empty(17, d -> putString(d, command, 256));
    }
  }

  /**
   * One explicit subscription per SDK connection; use one callback to dispatch kinds. Sequence is
   * global and may skip filtered/other-peer events. Ring lag or callback saturation disconnects
   * visibly.
   */
  public CompletableFuture<Subscription> subscribe(Set<EventKind> kinds, Consumer<Event> listener) {
    Objects.requireNonNull(listener);
    Set<EventKind> copy = Set.copyOf(kinds);
    Subscription sub = new Subscription(copy, listener);
    if (!subscription.compareAndSet(null, sub))
      return CompletableFuture.failedFuture(new IllegalStateException("already subscribed"));
    int mask = 0;
    for (EventKind kind : copy) mask |= 1 << (kind.ordinal() + 1);
    final int value = mask;
    return empty(13, d -> d.writeInt(value))
        .handle(
            (v, e) -> {
              if (e != null) {
                subscription.compareAndSet(sub, null);
                throw new CompletionException(e);
              }
              return sub;
            });
  }

  public final class Subscription implements AutoCloseable {
    private final Set<EventKind> kinds;
    private final Consumer<Event> listener;
    private final AtomicBoolean active = new AtomicBoolean(true);

    private Subscription(Set<EventKind> k, Consumer<Event> l) {
      kinds = k;
      listener = l;
    }

    public CompletableFuture<Void> unsubscribe() {
      if (!active.compareAndSet(true, false)) return CompletableFuture.completedFuture(null);
      return empty(14, d -> {}).thenRun(() -> subscription.compareAndSet(this, null));
    }

    @Override
    public void close() {
      unsubscribe();
    }
  }

  @FunctionalInterface
  private interface Encoder {
    void write(DataOutputStream d) throws IOException;
  }

  @FunctionalInterface
  private interface Decoder<T> {
    T read(Wire r) throws IOException;
  }

  private static byte[] encode(Encoder encoder) throws IOException {
    ByteArrayOutputStream bytes = new ByteArrayOutputStream();
    try (DataOutputStream d = new DataOutputStream(bytes)) {
      encoder.write(d);
    }
    return bytes.toByteArray();
  }

  private CompletableFuture<Void> empty(int operation, Encoder encoder) {
    return request(operation, encoder, r -> null);
  }

  private <T> CompletableFuture<T> request(int operation, Encoder encoder, Decoder<T> decoder) {
    CompletableFuture<byte[]> result = new CompletableFuture<>();
    synchronized (submitLock) {
      if (closed.get())
        return CompletableFuture.failedFuture(
            new BridgeException(Code.DISCONNECTED, "bridge closed"));
      if (!slots.tryAcquire())
        return CompletableFuture.failedFuture(
            new BridgeException(Code.BACKPRESSURE, "64 pending calls limit"));
      long id = sequence.incrementAndGet();
      try {
        byte[] body =
            encode(
                d -> {
                  d.writeByte(1);
                  d.writeLong(id);
                  d.writeByte(operation);
                  putUuid(d, serverId);
                  encoder.write(d);
                });
        if (body.length > MAX_FRAME) throw new IllegalArgumentException("frame too large");
        ScheduledFuture<?> expiry =
            deadlines.schedule(
                () ->
                    fail(
                        new BridgeException(
                            Code.TIMEOUT,
                            "request expired; outcome may have committed; resnapshot")),
                5,
                TimeUnit.SECONDS);
        pending.put(id, new Pending(operation, result, expiry, new AtomicBoolean()));
        if (!outbound.offer(ByteBuffer.wrap(body)))
          throw new IllegalStateException("outbound queue full");
      } catch (IOException | RuntimeException e) {
        Pending p = pending.remove(id);
        if (p != null) p.deadline().cancel(false);
        slots.release();
        return CompletableFuture.failedFuture(e);
      }
    }
    return result.thenApply(
        bytes -> {
          try {
            Wire r = new Wire(bytes);
            T value = decoder.read(r);
            r.end();
            return value;
          } catch (IOException | RuntimeException e) {
            fail(new BridgeException(Code.DISCONNECTED, "invalid response schema"));
            throw new CompletionException(e);
          }
        });
  }

  private void writeLoop() {
    try {
      while (!closed.get()) writeFrame(channel, outbound.take());
    } catch (IOException | InterruptedException e) {
      if (e instanceof InterruptedException) Thread.currentThread().interrupt();
      fail(new BridgeException(Code.DISCONNECTED, "bridge write closed"));
    }
  }

  private void readLoop() {
    try {
      while (!closed.get()) {
        Wire r = new Wire(readFrame(channel));
        int kind = r.u8();
        long id = r.i64();
        int op = r.u8();
        if (kind == 2) {
          Pending p = pending.get(id);
          if (p == null || p.operation() != op || !p.received().compareAndSet(false, true))
            throw new IOException("unknown response");
          p.deadline().cancel(false);
          int status = r.u16();
          byte[] payload = r.rest();
          try {
            completions.execute(
                () -> {
                  try {
                    if (status == 0) p.future().complete(payload);
                    else {
                      try {
                        Wire error = new Wire(payload);
                        String message = error.string(1024);
                        error.end();
                        p.future().completeExceptionally(error(status, message));
                      } catch (IOException e) {
                        p.future().completeExceptionally(e);
                      }
                    }
                  } finally {
                    if (pending.remove(id, p)) slots.release();
                  }
                });
          } catch (RejectedExecutionException e) {
            throw e;
          }
        } else if (kind == 3) {
          if (op < 1 || op > 13) throw new IOException("invalid event kind");
          WorldHandle w = optionalWorld(r);
          Player p = r.u8() != 0 ? player(r) : null;
          String text = r.string(256);
          CombatChange change =
              op >= 11 ? new CombatChange(combatState(r), r.i32(), r.f32()) : null;
          Event event = new Event(id, EventKind.values()[op - 1], w, p, text, change);
          r.end();
          Subscription sub = subscription.get();
          if (sub != null && sub.active.get() && sub.kinds.contains(event.kind()))
            callbacks.execute(
                () -> {
                  if (sub.active.get()) {
                    try {
                      sub.listener.accept(event);
                    } catch (RuntimeException e) {
                      fail(new BridgeException(Code.DISCONNECTED, "event callback failed"));
                    }
                  }
                });
        } else throw new IOException("invalid frame kind");
      }
    } catch (IOException | RuntimeException e) {
      fail(
          new BridgeException(
              e instanceof RejectedExecutionException ? Code.BACKPRESSURE : Code.DISCONNECTED,
              "bridge read closed or event gap; resnapshot"));
    }
  }

  private void fail(BridgeException error) {
    synchronized (submitLock) {
      if (!closed.compareAndSet(false, true)) return;
      try {
        channel.close();
      } catch (IOException ignored) {
      }
      writer.interrupt();
      reader.interrupt();
      outbound.clear();
      deadlines.shutdownNow();
      callbacks.shutdownNow();
      // The 64 end-to-end slots include responses awaiting Java completion.
      // At most 64 normal plus 64 terminal tasks fit the completion queue.
      for (Map.Entry<Long, Pending> entry : pending.entrySet()) {
        entry.getValue().deadline().cancel(false);
        completions.execute(
            () -> {
              try {
                entry.getValue().future().completeExceptionally(error);
              } finally {
                if (pending.remove(entry.getKey(), entry.getValue())) slots.release();
              }
            });
      }
      completions.execute(() -> disconnected.complete(error.code()));
      completions.shutdown();
    }
  }

  @Override
  public void close() {
    fail(new BridgeException(Code.DISCONNECTED, "bridge closed"));
  }

  private void sameServer(UUID id) {
    if (!serverId.equals(id))
      throw new BridgeException(Code.STALE_HANDLE, "handle belongs to another server incarnation");
  }

  private void putWorld(DataOutputStream d, WorldHandle w) throws IOException {
    sameServer(w.serverId());
    d.writeLong(w.id());
    d.writeLong(w.generation());
  }

  private void putPlayer(DataOutputStream d, PlayerHandle p) throws IOException {
    sameServer(p.serverId());
    d.writeInt(p.entityId());
    putUuid(d, p.uuid());
  }

  private static void putPos(DataOutputStream d, BlockPos p) throws IOException {
    d.writeInt(p.x());
    d.writeInt(p.y());
    d.writeInt(p.z());
  }

  private static void putUuid(DataOutputStream d, UUID u) throws IOException {
    d.writeLong(u.getMostSignificantBits());
    d.writeLong(u.getLeastSignificantBits());
  }

  private static void putString(DataOutputStream d, String s, int max) throws IOException {
    if (s.length() > max)
      throw new IllegalArgumentException("text exceeds " + max + " UTF-8 bytes");
    byte[] b = s.getBytes(StandardCharsets.UTF_8);
    if (b.length > max) throw new IllegalArgumentException("text exceeds " + max + " UTF-8 bytes");
    d.writeShort(b.length);
    d.write(b);
  }

  private static void bound(int n) {
    if (n > 4096) throw new IllegalArgumentException("batch exceeds 4096");
  }

  private static CombatState combatState(Wire r) throws IOException {
    float health = r.f32();
    int dead = r.u8(), selected = r.u8();
    int main = r.i32(), off = r.i32();
    if (!Float.isFinite(health) || health < 0 || health > 20 || dead > 1 || selected > 8)
      throw new IOException("invalid combat snapshot");
    return new CombatState(health, dead != 0, selected, main, off);
  }

  private WorldHandle world(Wire r) throws IOException {
    return new WorldHandle(serverId, r.i64(), r.i64());
  }

  private WorldHandle optionalWorld(Wire r) throws IOException {
    long id = r.i64(), generation = r.i64();
    return id == 0 && generation == 0 ? null : new WorldHandle(serverId, id, generation);
  }

  private Player player(Wire r) throws IOException {
    return new Player(new PlayerHandle(serverId, r.i32(), r.uuid()), r.string(64), world(r));
  }

  private static BridgeException error(int status, String message) {
    if (status < 1 || status > 11) return new BridgeException(Code.DISCONNECTED, "unknown status");
    return new BridgeException(Code.values()[status - 1], message);
  }

  private static void writeFrame(SocketChannel socket, ByteBuffer body) throws IOException {
    ByteBuffer prefix = ByteBuffer.allocate(4).putInt(body.remaining());
    prefix.flip();
    while (prefix.hasRemaining()) socket.write(prefix);
    while (body.hasRemaining()) socket.write(body);
  }

  private static byte[] readFrame(SocketChannel socket) throws IOException {
    ByteBuffer prefix = ByteBuffer.allocate(4);
    readFully(socket, prefix);
    int n = prefix.flip().getInt();
    if (n < 10 || n > MAX_FRAME) throw new IOException("invalid frame length");
    ByteBuffer body = ByteBuffer.allocate(n);
    readFully(socket, body);
    return body.array();
  }

  private static void readFully(SocketChannel socket, ByteBuffer buffer) throws IOException {
    while (buffer.hasRemaining()) if (socket.read(buffer) < 0) throw new EOFException();
  }

  static final class Wire {
    private final DataInputStream data;

    Wire(byte[] body) {
      data = new DataInputStream(new ByteArrayInputStream(body));
    }

    float f32() throws IOException {
      return data.readFloat();
    }

    int u8() throws IOException {
      return data.readUnsignedByte();
    }

    int u16() throws IOException {
      return data.readUnsignedShort();
    }

    int i32() throws IOException {
      return data.readInt();
    }

    long i64() throws IOException {
      return data.readLong();
    }

    UUID uuid() throws IOException {
      return new UUID(i64(), i64());
    }

    int count(int max) throws IOException {
      int n = u16();
      if (n > max) throw new IOException("collection too large");
      return n;
    }

    String string(int max) throws IOException {
      int n = count(max);
      byte[] b = data.readNBytes(n);
      if (b.length != n) throw new EOFException();
      try {
        return StandardCharsets.UTF_8
            .newDecoder()
            .onMalformedInput(CodingErrorAction.REPORT)
            .decode(ByteBuffer.wrap(b))
            .toString();
      } catch (CharacterCodingException e) {
        throw new IOException(e);
      }
    }

    byte[] rest() throws IOException {
      return data.readAllBytes();
    }

    void end() throws IOException {
      if (data.available() != 0) throw new IOException("trailing bytes");
    }
  }
}
