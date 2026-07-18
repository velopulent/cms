# `xtask`: build and package Velopulent CMS

`xtask` contains repository automation for producing Velopulent CMS release packages.
It builds the dashboard and backend, stages the package payload, invokes the native
packaging tool, and writes release metadata.

Run every command from the repository root:

```console
cargo run -p xtask -- help
```

Generated packages are written to `dist/packages/`. Temporary staging files are
written to `target/package/`.

## Supported outputs

| Target OS | `--kind` | Output |
| --- | --- | --- |
| Linux | `portable` | `vcms-<version>-linux-<arch>.tar.gz` |
| Linux | `deb` | `vcms_<version>_<deb-arch>.deb` |
| Linux | `rpm` | `vcms-<version>-1.<rpm-arch>.rpm` |
| macOS | `portable` | `vcms-<version>-macos-<arch>.tar.gz` |
| macOS | `pkg` | `vcms-<version>-macos-<arch>.pkg` |
| Windows | `portable` | `vcms-<version>-windows-<arch>.exe` |
| Windows | `msi` | `vcms-<version>-windows-<arch>.msi` |

Every package run also creates:

- `vcms-<version>-<os>-<arch>-manifest.json`
- `vcms-<version>-<os>-<arch>-SHA256SUMS`

`host` and `all` currently mean the same thing: build the portable artifact plus
all native package formats supported by the selected host OS.

## Build packages

Build every package supported by the current machine:

```console
cargo run -p xtask --release -- package --kind host
```

Build one format:

```console
# Linux
cargo run -p xtask --release -- package --kind portable
cargo run -p xtask --release -- package --kind deb
cargo run -p xtask --release -- package --kind rpm

# macOS
cargo run -p xtask --release -- package --kind pkg

# Windows
cargo run -p xtask --release -- package --kind msi
```

Specify release version and architecture when needed:

```console
cargo run -p xtask --release -- package \
  --kind all \
  --version 1.2.3 \
  --target-os linux \
  --arch amd64
```

PowerShell equivalent:

```powershell
cargo run -p xtask --release -- package `
  --kind all `
  --version 1.2.3 `
  --target-os windows `
  --arch amd64
```

If `--version` is omitted, `xtask` uses `GITHUB_REF_NAME` with a leading `v`
removed, or falls back to the version in `apps/backend/Cargo.toml`.

If `--target-os` or `--arch` is omitted, current host values are used. Accepted
architecture aliases include `amd64`, `x86_64`, `x64`, `arm64`, and `aarch64`.
`darwin` is accepted as an alias for `macos`.

## Build prerequisites

All real package builds require Rust, Cargo, Bun, and project dependencies. By
default, `xtask` runs:

1. `bun run build:dashboard`
2. `cargo build --release --locked` for the backend
3. selected package builder

Native package tools are also required:

| Format | Required tool | Build host |
| --- | --- | --- |
| Portable Unix archive | `tar` | Linux or macOS |
| DEB | `dpkg-deb` | Linux |
| RPM | `rpmbuild` and `tar` | Linux |
| macOS PKG | `pkgbuild` and `productbuild` | macOS |
| MSI | WiX v7 `wix` CLI | Windows |

MSI automation uses WiX v7's direct `-acceptEula wix7` build switch. Running an
MSI build therefore accepts the WiX v7 OSMF EULA for that invocation.
Install the matching UI and Util extensions once before local MSI builds:

```console
wix eula accept wix7
wix extension add --global WixToolset.UI.wixext/7.0.0
wix extension add --global WixToolset.Util.wixext/7.0.0
```

Real native packages must be built on their matching OS. `--target-os` selects and
labels the target; it does not cross-compile the backend binary.

## Reuse an existing release binary

Use `--skip-build` when dashboard and backend were already built. `xtask` expects
the binary at `target/release/vcms` or `target/release/vcms.exe`:

```console
bun run build:dashboard
cargo build --release --locked --manifest-path apps/backend/Cargo.toml
cargo run -p xtask --release -- package --kind all --skip-build
```

`--skip-build` does not skip native packaging tools.

## Deterministic dry-runs

Dry-runs stage templates and create deterministic placeholder artifacts without
building the application or invoking external packaging tools. They are useful for
CI and package-definition validation:

```console
cargo run -p xtask -- package-dry-run \
  --kind all \
  --version 1.2.3 \
  --target-os linux \
  --arch amd64
```

These two forms are equivalent:

```console
cargo run -p xtask -- package-dry-run --kind all
cargo run -p xtask -- package --kind all --dry-run
```

Dry-run files in `dist/packages/` are placeholders, not installable packages.

## Render the Arch Linux package recipe

Arch support is distributed as a `PKGBUILD` consuming published Linux portable
archives. First produce or download both Linux archives, then render the recipe:

```console
cargo run -p xtask -- arch-render \
  --version 1.2.3 \
  --amd64 dist/packages/vcms-1.2.3-linux-amd64.tar.gz \
  --arm64 dist/packages/vcms-1.2.3-linux-arm64.tar.gz \
  --out dist/arch
```

This writes:

```text
dist/arch/PKGBUILD
dist/arch/vcms.service
```

The renderer calculates SHA-256 hashes from the supplied archives and inserts them
into `PKGBUILD`. The rendered recipe is intended for the AUR workflow; it is not a
release archive itself.

## Test `xtask`

```console
cargo test -p xtask --locked
cargo fmt --all -- --check
```

Package and service source files live in [`../packaging`](../packaging/README.md).
Keep native definitions there; keep `xtask` focused on validation, staging,
orchestration, artifact naming, manifests, and checksums.
