# Native packaging

This directory is source of truth for service and installer definitions. Keep native
files reviewable here; `xtask` only stages them, substitutes release metadata, invokes
platform tools, and writes artifact manifests/checksums.

- `linux/`: shared systemd unit
- `debian/`: Debian metadata and maintainer scripts
- `rpm/`: RPM spec template
- `macos/`: launchd definition and installer scripts
- `windows/`: WiX source template
- `arch/`: post-release AUR recipe template

GitHub releases contain portable archives plus DEB, RPM, MSI, and macOS PKG artifacts.
Arch uses the same Linux archives through a rendered, checksummed PKGBUILD submitted to
AUR. It is intentionally not represented by a `PKGBUILD.tar.gz` release attachment.
