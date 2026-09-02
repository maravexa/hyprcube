# Release guide

HyprCube uses one version for every workspace crate. The current version is
defined by `[workspace.package]` in the root `Cargo.toml`; crate manifests and
the CLI inherit it. Do not lower an existing version.

## Preflight

1. Move completed changes from `Unreleased` in `CHANGELOG.md` into a dated
   version section.
2. Update the workspace version and every versioned internal dependency in the
   root `Cargo.toml`; update `pkgver` in `PKGBUILD`.
3. Run the commands in `AGENTS.md`, then run `cargo package --workspace` and
   inspect each generated package.
4. Commit the version and lockfile before creating `v<version>`.

## crates.io

On the initial publish, publish in dependency order and wait for each package
to become available before continuing:

```sh
cargo publish -p hyprcube-core
cargo publish -p hyprcube-hyprconf
cargo publish -p hyprcube-panels
cargo publish -p hyprcube
```

Use `--dry-run` first. Later releases use the same order.

## GitHub and AUR

Push the signed `v<version>` tag only after the version commit is on `main`.
The release workflow verifies the tag, builds `.deb`, `.rpm`, and `.tar.zst`
artifacts, and creates the GitHub release.

Once GitHub serves the tag archive, run `updpkgsums` and `makepkg --printsrcinfo
> .SRCINFO` in a copy of the AUR package repository. Commit both files there;
the in-repository `PKGBUILD` is the maintained source for that packaging.
