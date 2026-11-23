# Code signing policy

Free code signing provided by [SignPath.io](https://about.signpath.io), certificate by [SignPath Foundation](https://signpath.org)

This page says which Kleene Star files are signed, how they are built, who
can have them signed, and what the signed programs do on your computer.

> **Status:** the release workflow is ready for signing, and the project is
> applying to SignPath Foundation. Until signing is switched on, Windows
> releases are unsigned, as before. Each release's notes say whether its
> Windows files are signed; you can also check a file's *Properties >
> Digital Signatures* tab.

## What is signed

On Windows, every release's:

- `kstar.exe`, the `kstar` program (inside `kstar-x86_64-pc-windows-msvc.zip`
  and installed by `kstar-setup-x64.exe`);
- `kstar-lsp.exe`, the language server (in the same places);
- `kstar-setup-x64.exe`, the installer.

Nothing else is signed: not the Linux or macOS programs, the VS Code
extension, the install scripts or any file built outside the release
workflow.

## How they are built

The signed files are built only by the
[release workflow](.github/workflows/release.yml), on GitHub-hosted runners,
from the source in this public repository at the release's tag. No one
builds or signs them on their own machine. The workflow:

1. runs the tests and builds `kstar.exe` and `kstar-lsp.exe`;
2. checks their version details (product name *Kleene Star*, the version
   of the release) and sends them to SignPath to be signed;
3. packages the signed programs in the `.zip` and builds the installer from
   them;
4. checks and signs the installer the same way;
5. computes the SHA-256 checksums of the signed files and publishes the
   release.

SignPath signs a file only if it comes from this repository's release
workflow and its product name and version match, and only after an
approver has approved that signing request by hand.

## Roles

| Role | Who | What they do |
|---|---|---|
| Authors (committers) | [Pratham71](https://github.com/Pratham71) | Write and commit the source. |
| Reviewers | [Pratham71](https://github.com/Pratham71) | Review every change before it is merged into `main`, including changes from other contributors. |
| Approvers | [Pratham71](https://github.com/Pratham71) | Approve every signing request by hand, after checking that it comes from a release tag of this repository. |

All three roles are currently held by the maintainer. Contributions from
others come in through pull requests, which the maintainer reviews before
they are merged; nothing is signed from a branch or a pull request.

## Privacy

This program will not transfer any information to other networked systems unless specifically requested by the user or the person installing or operating it.

In detail:

- `kstar` connects to the network only when you run `kstar update` (or
  `kstar update --check`). It then asks GitHub's API for the latest release
  of this repository and, if you agree to update, downloads that release's
  files from GitHub. It sends nothing about you or your programs: only the
  requests for those files, with `kstar/<version>` as the user agent and,
  if you set `GITHUB_TOKEN`, that token. It never checks for updates on its
  own.
- `kstar-lsp`, the language server, makes no network connections. The VS
  Code extension compares its own version with the server's on your
  computer and, if they differ, offers a button that runs `kstar update`
  only when you click it.
- Running, checking, tracing and drawing programs happens on your
  computer. Programs can read and write files only in their own folder and
  the folders you allow with `--allow-path`.
- The installer downloads nothing: it installs the files it carries.

## What the installer changes

`kstar-setup-x64.exe` installs for the current user, without administrator
rights (or for all users, if you choose so and are an administrator). It:

- copies `kstar.exe` and `kstar-lsp.exe` to
  `%LOCALAPPDATA%\Programs\Kleene Star\bin` (for all users:
  `Program Files\Kleene Star\bin`), with the README, the licence and the
  icon in the folder above;
- adds a *Kleene Star* folder to the Start Menu with the REPL, the README
  and the uninstaller;
- registers itself in *Settings > Apps* so it can be uninstalled;
- if you keep these options ticked: adds the `bin` folder to your PATH,
  associates `.kstar` files with Kleene Star (with the K* icon), and, if VS
  Code is installed, installs the Kleene Star extension in it.

`kstar update` replaces `kstar.exe` and `kstar-lsp.exe` in the same folder
(the old ones are renamed to `.old` and removed the next time `kstar`
starts). The install script `install.ps1` copies the same two programs and
adds the folder to your user PATH.

## Uninstalling

Uninstall Kleene Star from *Settings > Apps > Installed apps*, or with
*Uninstall Kleene Star* in its Start Menu folder. The uninstaller removes
the programs and the folder, the Start Menu folder, the PATH entry, the
`.kstar` association and, if the installer put it there, the VS Code
extension. If you installed with `install.ps1`, delete the install folder
and remove its `bin` folder from your user PATH, as the
[installation guide](docs/install.md#uninstalling) says.

## Reporting a problem

If a signed file behaves in a way this page does not describe, or you find
a file signed for Kleene Star that did not come from this repository's
releases, please report it as [SECURITY.md](SECURITY.md) describes.
