# Installation and Removal

Build with the pinned toolchain and locked dependencies:

```text
cargo build --release --locked --offline
target/release/anasemble install /opt/anasemble-0.0.1
```

The prefix must not already exist. Installation stages and syncs a Rust binary, compatibility manifest, and default operations configuration, then atomically renames the complete prefix into place. It never edits `PATH`, shell profiles, launch agents, system services, Kubernetes resources, or operator data.

Verify `bin/anasemble operations-status <operations-root>` against a disposable operations root before changing any executable reference. Keep the prior prefix intact through the disaster drill and acceptance window.

Remove an inactive prefix with:

```text
/path/to/active/anasemble uninstall /opt/anasemble-0.0.1
```

Uninstallation parses the exact install manifest, verifies every installed file digest, and removes only those four files and the now-empty owned directories. It refuses modified files, unknown manifests, links, extra files that make a directory non-empty, or a missing prefix. Operations roots, keys, evidence, state, OCI artifacts, Kubernetes resources, and backend snapshots are never removed by uninstall.
