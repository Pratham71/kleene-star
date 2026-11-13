# Installing Kleene Star

Kleene Star is two programs: `kstar`, the command you run, and `kstar-lsp`,
the language server that editors use. Every way of installing below gives
you both. They are self-contained: you do not need Rust, and on Windows,
from 0.2.5-beta on, you do not need the Visual C++ Redistributable.

| You are on | The quickest way |
|---|---|
| Windows | [The installer](#windows-the-installer), or [`install.ps1`](#windows-installps1) |
| Linux (x64) | [`install.sh`](#linux-and-macos-installsh) |
| macOS (Intel or Apple Silicon) | [`install.sh`](#linux-and-macos-installsh) |
| Anything else, such as ARM Linux | [Build from source](#building-from-source) |

Then, if you use VS Code, add [the extension](#the-vs-code-extension).

## Windows: the installer

1. Open the [latest release](https://github.com/Pratham71/kleene-star/releases/latest)
   and download `kstar-setup-x64.exe`.
2. Run it. It installs for your user only, so it does not ask for
   administrator rights. Windows may warn that the publisher is unknown:
   see [SmartScreen](#windows-protected-your-pc) below.
3. Keep the options you want. All are ticked by default:
   - **Add Kleene Star to the PATH**, so `kstar` works in any terminal.
   - **Associate `.kstar` files** with Kleene Star, with the K* icon.
   - **Install the Kleene Star extension in VS Code**. This option is only
     shown when VS Code is installed.
4. Open a **new** terminal and check:

   ```bash
   kstar version
   ```

The programs go in `%LOCALAPPDATA%\Programs\Kleene Star\bin`. The Start
Menu gets a Kleene Star folder with the REPL, the README and the
uninstaller.

## Windows: install.ps1

In PowerShell:

```bash
powershell -c "irm https://raw.githubusercontent.com/Pratham71/kleene-star/main/install.ps1 | iex"
```

The script downloads the build for Windows from the latest release, checks
its SHA-256 checksum, puts `kstar.exe` and `kstar-lsp.exe` in
`%LOCALAPPDATA%\Programs\Kleene Star\bin` (the same folder as the
installer, so use one or the other) and adds that folder to your user PATH.
It does not associate `.kstar` files or install the VS Code extension.

It reads these settings from the environment, all optional:

| Variable | Meaning |
|---|---|
| `KSTAR_VERSION` | The release to install, such as `v0.2.5-beta`. Default: the latest stable release. |
| `KSTAR_INSTALL_DIR` | Where to install. Default: `%LOCALAPPDATA%\Programs\Kleene Star`. |
| `KSTAR_NO_PATH` | `1` leaves your PATH alone. |

Set them in the same PowerShell window as the script. For example, to
install the 0.2.5-beta pre-release:

```bash
$env:KSTAR_VERSION = "v0.2.5-beta"; irm https://raw.githubusercontent.com/Pratham71/kleene-star/main/install.ps1 | iex
```

## Linux and macOS: install.sh

```bash
curl -fsSL https://raw.githubusercontent.com/Pratham71/kleene-star/main/install.sh | sh
```

The script picks the build for your system (Linux x64, macOS Intel or
macOS Apple Silicon), checks its SHA-256 checksum, puts `kstar` and
`kstar-lsp` in `~/.kstar/bin`, and adds that folder to your PATH by adding
one line to `~/.profile`, and to `~/.bashrc` and `~/.zshrc` if you have
them. Running it again does not add the line twice.

It takes the same settings as `install.ps1`, given to `sh`:

```bash
curl -fsSL https://raw.githubusercontent.com/Pratham71/kleene-star/main/install.sh | KSTAR_VERSION=v0.2.5-beta sh
```

`KSTAR_INSTALL_DIR` defaults to `~/.kstar`, and `KSTAR_NO_PATH=1` leaves
your shell's startup files alone.

There is no build for ARM Linux yet; [build from source](#building-from-source)
there.

## Downloading by hand

Every [release](https://github.com/Pratham71/kleene-star/releases) has
these files:

| File | What it is |
|---|---|
| `kstar-setup-x64.exe` | The Windows installer |
| `kstar-x86_64-pc-windows-msvc.zip` | Windows x64 |
| `kstar-x86_64-unknown-linux-gnu.tar.gz` | Linux x64 |
| `kstar-x86_64-apple-darwin.tar.gz` | macOS, Intel |
| `kstar-aarch64-apple-darwin.tar.gz` | macOS, Apple Silicon |
| `kleene-star-<version>.vsix` | The VS Code extension |
| `SHA256SUMS` | The checksums of all of the above |

Each file also has its own checksum beside it, such as
`kstar-x86_64-unknown-linux-gnu.tar.gz.sha256`.

**Check the download** before you unpack it. On Linux, with
`SHA256SUMS` in the same folder as the files you downloaded:

```bash
sha256sum -c SHA256SUMS --ignore-missing
```

On macOS, use `shasum -a 256 -c` with the file's own `.sha256`:

```bash
shasum -a 256 -c kstar-aarch64-apple-darwin.tar.gz.sha256
```

Each file you downloaded should be reported as `OK`. On Windows, in
PowerShell, compare the file's hash with the one in its `.sha256` file:

```bash
(Get-FileHash kstar-x86_64-pc-windows-msvc.zip).Hash -eq (Get-Content kstar-x86_64-pc-windows-msvc.zip.sha256).Split(" ")[0]
```

That prints `True` when they match.

**Unpack it.** Each archive holds `kstar`, `kstar-lsp`, `LICENSE` and
`README.md` (`kstar.exe` and `kstar-lsp.exe` on Windows). Put the two
programs in a folder on your PATH, such as `~/.local/bin`. On macOS, see
[Gatekeeper](#macos-cannot-be-opened-because-the-developer-cannot-be-verified)
below.

## Building from source

You need [Rust](https://rustup.rs), stable. Then:

```bash
git clone https://github.com/Pratham71/kleene-star.git
```

```bash
cd kleene-star
```

```bash
cargo install --path . --locked
```

That builds both programs and puts them in `~/.cargo/bin`, which `rustup`
already added to your PATH. This works on any system Rust supports,
including ARM Linux. `main` may be ahead of the latest release; check out a
tag such as `v0.2.3` first to build a release.

## The VS Code extension

On Windows, the installer can install the extension for you (see above).
Otherwise:

1. Install Kleene Star itself first, by any of the ways above: the
   extension needs `kstar-lsp`.
2. Download `kleene-star-<version>.vsix` from the same release.
3. Install it:

   ```bash
   code --install-extension kleene-star-0.2.3.vsix
   ```

   or, in VS Code, *Extensions*, then `...`, then *Install from VSIX...*.

The extension looks for `kstar-lsp` and `kstar` on the PATH, then in
`~/.cargo/bin`, `~/.kstar/bin` and, on Windows,
`%LOCALAPPDATA%\Programs\Kleene Star\bin`. It is not on the VS Code
Marketplace. See [its README](../editors/vscode/README.md) for what it
does and its settings.

## Updating

```bash
kstar update --check
```

says whether a newer stable release exists. `kstar update` says the same
and prints the command to install it; it does not replace itself, since a
running program cannot be overwritten on Windows. To update, install again
the way you installed: run the new `kstar-setup-x64.exe`, run the install
script again (it replaces the old programs), or run `cargo install` again
in an updated clone. Install the matching `.vsix` too.

`kstar version` shows what you have.

## Choosing stable or beta

Releases come in two kinds, both listed on the
[releases page](https://github.com/Pratham71/kleene-star/releases):

- **Stable** releases, such as 0.2.3, have a plain version number. The
  newest one is marked *Latest*. The install scripts and `kstar update`
  use it unless told otherwise.
- **Beta** releases, such as 0.2.5-beta, are marked *Pre-release*. They
  bring new features early (0.2.4-beta added imports, `std.math` and
  `kstar export`) and may still change before the next stable release.
  See [the changelog](../CHANGELOG.md) for what each one has.

To install a beta, download its files from its release page, or give its
tag to a script with `KSTAR_VERSION` as shown above. To go back to stable,
run the script again without `KSTAR_VERSION`, or run the latest stable
installer.

`kstar update --check` only compares with the latest stable release, so on
a beta it says you are up to date until a newer stable release comes out.

## Uninstalling

**Windows, installed with the installer:** open *Settings*, *Apps*, find
Kleene Star and uninstall it. That removes the programs, the PATH entry,
the `.kstar` association and, if the installer added it, the VS Code
extension.

**Windows, installed with `install.ps1`:** delete the folder
`%LOCALAPPDATA%\Programs\Kleene Star`, then remove
`%LOCALAPPDATA%\Programs\Kleene Star\bin` from your user PATH (*Settings*,
*System*, *About*, *Advanced system settings*, *Environment Variables*).

**Linux and macOS, installed with `install.sh`:**

```bash
rm -rf ~/.kstar
```

Then delete the line `export PATH="$HOME/.kstar/bin:$PATH"` (written with
your home folder spelled out) and the comment `# Added by the Kleene Star
installer` above it from `~/.profile`, `~/.bashrc` and `~/.zshrc`.

**Built from source:**

```bash
cargo uninstall kstar
```

**The VS Code extension:**

```bash
code --uninstall-extension kleene-star.kleene-star
```

## Troubleshooting

### `kstar` is not found right after installing

A terminal reads the PATH when it starts. Close it and open a new one (on
Windows, restart VS Code too, since its terminals inherit its PATH). On
Linux and macOS you can instead run the `export PATH=...` line the script
printed. If it is still not found, check that the folder is on your PATH:
`%LOCALAPPDATA%\Programs\Kleene Star\bin` on Windows, `~/.kstar/bin`
otherwise.

### "Windows protected your PC"

The installer and the programs are not code-signed, so Microsoft Defender
SmartScreen does not recognise the publisher and may stop the installer
from starting. If you downloaded it from this project's releases page and
its [checksum matches](#downloading-by-hand), click *More info*, then *Run
anyway*. The install scripts download with PowerShell, which does not mark
files this way, so they do not show this warning.

### macOS: "cannot be opened because the developer cannot be verified"

macOS Gatekeeper blocks programs from unidentified developers that were
downloaded with a web browser, which marks them as quarantined. The
install script downloads with `curl`, which does not, so this only happens
when you download an archive by hand. Remove the mark from the two
programs (adjust the folder to where you put them):

```bash
xattr -d com.apple.quarantine ~/.local/bin/kstar ~/.local/bin/kstar-lsp
```

### VS Code: "cannot find `kstar-lsp`"

The extension shows this when it cannot find the language server. Install
Kleene Star itself (the extension does not include it), then restart VS
Code so it sees the new PATH. If the programs are somewhere unusual, set
`kstar.serverPath` (and `kstar.path`, for Run, Check and Trace) to their
full paths: the message's *Open Settings* button goes there. `kstar-lsp`
is always installed next to `kstar`, so if `kstar version` works in a new
terminal, the folder it is in is the one to use.

### Something else

Run `kstar version` and include its output in
[an issue](https://github.com/Pratham71/kleene-star/issues/new/choose).
