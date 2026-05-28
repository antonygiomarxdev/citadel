# Distribution: self-contained bundles

Citadel ships a **vendored Node runtime** alongside the app and its **native Rust
NAPI addon** (the `.node` file compiled from `citadel-napi/`).

Because we have native code (NAPI), each platform bundle must be built on its
target architecture — unlike the upstream codegraph which has zero native addons
and can cross-pack all platforms from a single runner.

## What's in a bundle

Built by [`scripts/build-bundle.sh`](scripts/build-bundle.sh) — one archive per
platform:

```
citadel-<target>/
  node | node.exe          # official Node runtime for <target>
  citadel-native.*.node    # prebuilt NAPI addon for <target>
  lib/
    dist/                  # compiled app (+ tree-sitter .wasm grammars, schema.sql)
    node_modules/          # production deps only (pure JS / wasm — portable)
  bin/
    citadel | citadel.cmd  # launcher → runs the bundled Node with the app
```

Targets: `darwin-arm64`, `darwin-x64`, `linux-x64`, `linux-arm64`, `win32-x64`,
`win32-arm64`.

## Release pipeline

[`.github/workflows/release.yml`](.github/workflows/release.yml) — manually
triggered. The pipeline:

1. **Matrix build**: builds the NAPI `.node` binary for each target platform
   (ubuntu-latest, macos-latest, windows-latest) via `napi build --target`
2. **TypeScript build**: compiles the TS app + copies assets (one runner)
3. **Release job**: collects all platform binaries + TS dist, creates the GitHub
   Release (notes from CHANGELOG.md), publishes to npm
