# Building Limusic (Windows)

Limusic is built and shipped for **Windows only**.

## Prerequisites

- Rust (stable, MSVC toolchain) + Visual Studio Build Tools with the
  "Desktop development with C++" workload (MSVC + Windows SDK).
- Node.js 22+ and pnpm 10.
- [7-Zip](https://www.7-zip.org/) (`7z` on PATH) to unpack the libmpv dev package.
- WiX (for the `.msi`) and NSIS (for the `-setup.exe`) — `cargo tauri build` offers
  to install both automatically the first time.

## 1. Get the libmpv dev package

The player links libmpv directly, so you need the DLL plus an import library.
The pinned build is the same one CI uses:

```
https://github.com/shinchiro/mpv-winbuild-cmake/releases/download/20260610/mpv-dev-x86_64-20260610-git-304426c.7z
```

Unpack it into `.libmpv\` at the repo root. The dev package ships a MinGW import
lib (`libmpv.dll.a`) that the MSVC linker cannot consume, and no `.def` file — so
synthesise a real `mpv.lib` from the DLL's export table:

```powershell
cd .libmpv
$vswhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
$vs = & $vswhere -latest -products * -property installationPath
Import-Module "$vs\Common7\Tools\Microsoft.VisualStudio.DevShell.dll"
Enter-VsDevShell -VsInstallPath $vs -SkipAutomaticLocation -DevCmdArguments '-arch=x64 -host_arch=x64'

$names = & dumpbin /exports libmpv-2.dll |
  Select-String -Pattern '^\s+\d+\s+[0-9A-Fa-f]+\s+[0-9A-Fa-f]+\s+(\w+)' |
  ForEach-Object { $_.Matches[0].Groups[1].Value }
@("EXPORTS") + ($names | ForEach-Object { "    $_" }) | Set-Content -Path mpv.def -Encoding ascii
& lib /def:mpv.def /name:libmpv-2.dll /out:mpv.lib /machine:x64
cd ..
```

(The Rust side only emits `cargo:rustc-link-lib=mpv` via pregenerated bindings — no
headers needed.)

## 2. Point the linker at it

```powershell
$env:RUSTFLAGS = "-L native=$PWD\.libmpv"
```

Also copy `libmpv-2.dll` next to the produced exe (or into `src-tauri\` for `tauri dev`)
so the app can start.

## 3. Frontend deps

```powershell
cd ui
pnpm install
cd ..
```

## 4. Run / build

```powershell
cargo tauri dev     # development
cargo tauri build   # release: NSIS setup.exe + MSI under target/release/bundle/
```

## Validation checklist

- `cargo test` (with RUSTFLAGS set) — full unit suite
- `cargo clippy --workspace --all-targets`
- `cargo fmt --all --check`
- `cd ui && pnpm check`
- `node --experimental-strip-types ui/src/lib/<name>.check.ts` for each of the UI assertion scripts
