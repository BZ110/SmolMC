package dev.smolmc;
import static dev.smolmc.MinecraftRustServer.*;
import java.nio.file.Path;
import java.util.List;
import java.util.concurrent.TimeUnit;
/** Real Java terrain controls against an imported-template fixture. */
public final class TerrainIntegration {
 private TerrainIntegration() {}
 public static void main(String[] args)throws Exception {
  try(var s=MinecraftRustServer.attach(Path.of(args[0]))){
   var template=s.templates().configured().get(5,TimeUnit.SECONDS);if(!template.name().equals("imported"))throw new AssertionError("template");
   var a=s.worlds().defaultWorld().get(5,TimeUnit.SECONDS);var b=s.worlds().create(template,Profile.BUILD).get(5,TimeUnit.SECONDS);
   var pos=new BlockPos(0,64,0);if(!s.worlds().state(1).get(5,TimeUnit.SECONDS).name().equals("minecraft:stone"))throw new AssertionError("state");
   s.worlds().editStates(a,List.of(new StateEdit(pos,1))).get(5,TimeUnit.SECONDS);
   if(!s.worlds().readStates(a,List.of(pos)).get(5,TimeUnit.SECONDS).equals(List.of(1)))throw new AssertionError("edit");
   if(!s.worlds().readStates(b,List.of(pos)).get(5,TimeUnit.SECONDS).equals(List.of(0)))throw new AssertionError("isolation");
   var c=s.worlds().cloneWorld(a).get(5,TimeUnit.SECONDS);s.worlds().editStates(c,List.of(new StateEdit(pos,0))).get(5,TimeUnit.SECONDS);
   if(!s.worlds().readStates(a,List.of(pos)).get(5,TimeUnit.SECONDS).equals(List.of(1)))throw new AssertionError("clone COW");
   var current=s.worlds().reset(a).get(5,TimeUnit.SECONDS);
   if(!s.worlds().readStates(current,List.of(pos)).get(5,TimeUnit.SECONDS).equals(List.of(0)))throw new AssertionError("reset");
   try{s.worlds().readStates(a,List.of(pos)).get(5,TimeUnit.SECONDS);throw new AssertionError("stale accepted");}catch(java.util.concurrent.ExecutionException expected){if(!(expected.getCause()instanceof BridgeException e)||e.code()!=Code.STALE_HANDLE)throw expected;}
   s.worlds().destroy(b).get(5,TimeUnit.SECONDS);s.worlds().destroy(c).get(5,TimeUnit.SECONDS);
   System.out.println("PASS real Java terrain: configured template, typed states/edits, isolation, clone COW, reset, stale generation, cleanup");
  }
 }
}
