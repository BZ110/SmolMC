#!/bin/sh
set -eu
mkdir -p target/java/classes
javac --release 17 -Xlint:all -Werror -d target/java/classes $(find java/sdk/src/main/java java/sdk/src/test/java java/examples -name '*.java')
mkdir -p target/java/sdk
javac --release 17 -Xlint:all -Werror -d target/java/sdk $(find java/sdk/src/main/java -name '*.java')
jar --create --file target/java/smolmc-sdk-0.3.0.jar -C target/java/sdk dev/smolmc
java -ea -cp target/java/classes dev.smolmc.IntegrationTest "${1:-target/release/smolmc}"
