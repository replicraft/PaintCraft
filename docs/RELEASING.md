# Release builds

The [release workflow](../.github/workflows/release.yml) follows the native runner,
portable archive and complete-release checks used by
[EarthCraft](https://github.com/therichronicles/EarthCraft/blob/main/.github/workflows/release.yml).
PaintCraft retains GPU rendering in every desktop build. It uses the locked Cargo
dependencies and Rust 1.99.0 from `rust-toolchain.toml`.

## Downloads

Names start with `paintcraft-<version>-`. For version 0.1.0:

| Platform | Architectures | Suffixes |
| --- | --- | --- |
| Windows | `windows-x64`, `windows-arm64` | `-portable.zip`, `.exe`, `.msi` |
| macOS | `macos-arm64`, `macos-x64` | `.dmg`, `-update.zip` |
| Linux | `linux-x64`, `linux-arm64` | `.deb`, `.rpm`, `.AppImage`, `.tar.gz` |
| FreeBSD | `freebsd-x64` | `.tar.gz` |

The assembled release has 19 downloads plus `SHA256SUMS.txt`. Each portable ZIP
or tar archive includes the application, its license and bundled font notices.
Fonts and interface assets are embedded in the executable. There are no source trees or build tools
inside the downloads. The checksums describe the final downloadable bytes.

## Build and publish

1. Set `[workspace.package].version` in the root `Cargo.toml` and update the lockfile.
2. Run `python3 -m unittest discover -s scripts/tests -p 'test_package_*.py'`.
3. Use **Actions → Release → Run workflow** to build and assemble every platform
   without publishing. Download `paintcraft-release-<version>` from that run.
4. Check installation, document opening, rendering, file dialogs, upgrades and
   uninstall on the actual target systems. Automated build and installer checks
   do not establish interactive compatibility.
5. Push a matching `v<version>` tag when ready to publish. A tag that disagrees
   with Cargo fails before building. The tag run builds all platforms again,
   checks every asset and SHA-256 digest, creates/uploads a draft and then
   publishes it. An existing published release is refused. Prerelease versions
   produce prerelease downloads; MSI and Apple bundle metadata use the numeric
   major/minor/patch version. Use a new numeric version when upgrading an MSI.

Pull requests changing packaging also build the release matrix. Native build
artifacts and the assembled set are retained for seven days. No release is
published from a pull request or a manual workflow run. No credentials are needed
for local packaging; tag publication uses the job's repository token.

The `assemble` job refuses a missing architecture, extra/stale asset, empty file,
symlink or mismatched per-asset checksum. Only after all 19 checks succeed does
it write the sorted, LF-delimited `SHA256SUMS.txt` and remove intermediate checksum
sidecars. It runs `sha256sum --check` independently before uploading the final set.

## Platform details

Windows builds run natively on Windows x64 and ARM64. The VC runtime is linked
statically, so portable downloads do not require a separate VC redistributable.
The executable contains the PaintCraft icon and version resources. WiX 5.0.2
builds both MSI architectures from one template, with a stable upgrade identity,
Program Files installation and a Start menu shortcut. The MSI registers the
`.pcnd` extension, `PaintCraft.Document` ProgID, icon, Open With and Default Apps
capabilities. Its open command quotes both executable and document paths.
It does not write the user's protected `UserChoice` preference. Existing explicit
default-app choices remain controlled by Windows. CI installs, repairs and
uninstalls each MSI and checks the owned registry entries and installed binary.
Windows packages are not Authenticode-signed.

macOS requires 13.0 or newer. Apple Silicon and Intel use separate thin Rust
binaries and matching Swift launchers on native runners. Both retain the existing
Finder document-event launcher and `.pcnd` Launch Services declarations. DMGs
contain `PaintCraft.app` and an Applications shortcut. The `-update.zip` contains
that exact app, including its helper app, icon, metadata, signatures and executable
permissions. It uses forward-slash archive paths on every host. Both bundles are
ad-hoc signed and checked with `codesign --verify --deep --strict`; DMGs are checked
with `hdiutil verify`. These are not Developer ID signed or notarized. The release
verifier also mounts each DMG read-only, compares every app file against the
extracted ZIP and verifies both executable architectures and ZIP permissions.
No application is launched during these checks. The updater archive is ready
for a future updater consumer; this packaging work does not add
an in-app update service.

Linux packages build on Ubuntu 22.04 for glibc 2.35 or newer, with X11/Wayland
and Vulkan/OpenGL driver support. DEB and RPM include desktop, icon, AppStream,
license and `.pcnd` MIME metadata. Package-manager MIME/desktop triggers refresh
the relevant caches; no global default image handler is changed. Native dialogs
use the host's XDG desktop portal and require a portal backend suitable for the
desktop. AppImages include the X11, XKB and Wayland libraries loaded at runtime
in addition to discovered ELF dependencies, but still need a suitable graphics
driver. They can use FUSE or `APPIMAGE_EXTRACT_AND_RUN=1`; make the downloaded
AppImage executable before running it. Portable tar archives require host runtime
libraries. AppImages and portable archives do not install host MIME associations.
Package generation uses pinned cargo-packager 0.11.8 and cargo-generate-rpm 0.21.0.

FreeBSD x86-64 builds inside a FreeBSD 14.5 VM and produces a portable tar archive.
The build installs `libxkbcommon`, `wayland`, `libX11`, `libXcursor`, `libXrandr`,
`libXi`, `libxcb`, `mesa-libs` and `vulkan-loader`. Equivalent runtime graphics
libraries and a working desktop/display are required when running the archive.
Native dialogs also need a functioning XDG portal service/backend. CI inspects
the resulting executable's shared-library dependencies.

## Local packaging

Build the native release executable with the appropriate Rust target first:

```sh
cargo build --locked --release -p paintcraft --target aarch64-apple-darwin
python3 scripts/package_release.py package --version 0.1.0 \
  --target aarch64-apple-darwin \
  --binary target/aarch64-apple-darwin/release/paintcraft --output dist
```

Use `x86_64-apple-darwin` for Intel. The packaging script checks the executable
header rather than assuming its filename describes its architecture. DMG
creation requires macOS, Xcode command-line tools and disk-image access. It does
not launch, install or register the app.

For Linux, run `scripts/package_release.py package` for the tar archive, then
`scripts/package_linux.py` with the same `--version`, `--target`, `--binary` and
`--output`. Install the pinned tools from the release-tools action first.
Windows uses the archive command followed by `scripts/package_windows.py` after
installing WiX 5.0.2. Windows release builds must use
`RUSTFLAGS=-C target-feature=+crt-static`, as in CI.

Once every platform's files and `.sha256` sidecars are together:

```sh
python3 scripts/package_release.py finalize --version 0.1.0 --output dist
python3 scripts/package_release.py verify --version 0.1.0 --output dist
cd dist
sha256sum --check SHA256SUMS.txt
```
