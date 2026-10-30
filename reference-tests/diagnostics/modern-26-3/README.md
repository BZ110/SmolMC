# Historical 26.3 landing diagnostic

This original adapter helped investigate the user-rejected modern knockback
profile. It is **not** the final classic-profile acceptance gate. It executes
vanilla Player.aiStep, incoming movement and knockback primitives, explicitly
models listener accepted-position restoration, and prints a paired trajectory at
10–13 tick attack cadences. It does not instantiate a ServerPlayer, real network
listener, render interpolation, plugins or full server tick. The world delegates
stone/air collision to vanilla; audio/vibration sinks intentionally do nothing.

With the installed 26.3 JAR/library classpath in `VANILLA_CP`, first run the normal
codec check (which compiles its shared registry helper), then:

```sh
javac -cp "target/reference-check:$VANILLA_CP" -d target/reference-check \
  reference-tests/diagnostics/modern-26-3/*.java
java -Xmx512m -cp "target/reference-check:$VANILLA_CP" FlatMotionReferenceCheck
```

No Mojang bodies, JARs or assets are included. Its primitive reproduction does not
establish how every stock/Paper server/client combination feels. The final target
was explicitly changed by the user to 1.19.4-based knockback; contact settling is
an honest local arena policy. See the M3 report and marked numeric trace.
