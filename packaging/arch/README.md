# Arch Linux package

`PKGBUILD.template` is maintained release input, not a GitHub release artifact.
After portable Linux archives exist, release tooling replaces the version and both
archive checksums, generates `.SRCINFO`, and submits the reviewed recipe to AUR.

Render it with:

```bash
cargo run -p xtask -- arch-render --version 1.2.3 \
  --amd64 dist/packages/vcms-1.2.3-linux-amd64.tar.gz \
  --arm64 dist/packages/vcms-1.2.3-linux-arm64.tar.gz \
  --out dist/arch
```

Validate generated recipes in an Arch container with `makepkg --verifysource`,
`makepkg`, and `namcap`. AUR publication remains manual until dedicated credentials
and repository ownership are configured.
