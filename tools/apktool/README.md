# Apktool

Project-local Docker image with Java 21 and the official Apktool 3.0.3 JAR,
verified against its release SHA-256. Requires Docker with daemon access.

From the repository root:

```bash
tools/apktool/apktool build-image
tools/apktool/apktool --version
mkdir -p scratch/apk-input
cp /path/to/app.apk scratch/apk-input/app.apk
tools/apktool/apktool d /input/app.apk -o /output/decoded -p /output/framework
```

Read decoded files in `scratch/apk-output/decoded`. Choose a new output name
for each decode. Input files are mounted read-only; output files use your UID.
The container has no network access during analysis. Generated inputs and outputs
are ignored by Git.

Apktool decodes manifests, resources and smali; use JADX for Java decompilation.
This CLI setup does not register REA MCP tools.

Release: <https://github.com/iBotPeaches/Apktool/releases/tag/v3.0.3>
