# Installs Kleene Star (kstar and kstar-lsp) on Windows for the current user,
# without administrator rights:
#
#   powershell -c "irm https://raw.githubusercontent.com/Pratham71/kleene-star/main/install.ps1 | iex"
#
# While the repository is private, that URL needs signing in, so use the
# GitHub CLI (after `gh auth login`) instead:
#
#   powershell -c "gh api repos/Pratham71/kleene-star/contents/install.ps1 -H 'Accept: application/vnd.github.raw' | Out-String | iex"
#
# It downloads the latest release for this machine, checks its SHA-256
# checksum, puts the programs in %LOCALAPPDATA%\Programs\Kleene Star\bin and
# adds that folder to your PATH. Settings, all optional, as environment
# variables:
#
#   KSTAR_VERSION       a release tag such as v0.2.0 (default: the latest)
#   KSTAR_INSTALL_DIR   where to install (default: %LOCALAPPDATA%\Programs\Kleene Star)
#   KSTAR_REPO          the GitHub repository (default: Pratham71/kleene-star)
#   KSTAR_INSTALL_FROM  a folder that already holds the release files; used
#                       to test this script without downloading anything
#   KSTAR_NO_PATH       set to 1 to leave the PATH alone
#
# The installer, kstar-setup-x64.exe from the releases page, does the same
# and also associates .kstar files; use one or the other.

function Install-KleeneStar {
    $ErrorActionPreference = "Stop"
    $ProgressPreference = "SilentlyContinue"

    $repo = if ($env:KSTAR_REPO) { $env:KSTAR_REPO } else { "Pratham71/kleene-star" }
    $tag = $env:KSTAR_VERSION
    $dir = if ($env:KSTAR_INSTALL_DIR) { $env:KSTAR_INSTALL_DIR } else { Join-Path $env:LOCALAPPDATA "Programs\Kleene Star" }
    $bin = Join-Path $dir "bin"

    $arch = $env:PROCESSOR_ARCHITECTURE
    if ($env:PROCESSOR_ARCHITEW6432) { $arch = $env:PROCESSOR_ARCHITEW6432 }
    if ($arch -ne "AMD64" -and $arch -ne "ARM64") {
        throw "Kleene Star is only built for 64-bit Windows (this machine is $arch)."
    }
    # On ARM64 Windows the x64 build runs under emulation.
    $asset = "kstar-x86_64-pc-windows-msvc.zip"

    $work = Join-Path ([System.IO.Path]::GetTempPath()) ("kstar-install-" + [guid]::NewGuid())
    New-Item -ItemType Directory -Path $work | Out-Null
    try {
        if ($env:KSTAR_INSTALL_FROM) {
            Write-Host "Using the release files in $env:KSTAR_INSTALL_FROM"
            Copy-Item (Join-Path $env:KSTAR_INSTALL_FROM $asset) $work
            Copy-Item (Join-Path $env:KSTAR_INSTALL_FROM "$asset.sha256") $work
        } elseif (Get-Command gh -ErrorAction SilentlyContinue) {
            Write-Host "Downloading $asset from $repo $(if ($tag) { $tag } else { '(latest release)' }) with the GitHub CLI"
            $ghArgs = @("release", "download") + $(if ($tag) { @($tag) } else { @() }) + @("-R", $repo, "-p", $asset, "-p", "$asset.sha256", "-D", $work)
            & gh @ghArgs
            if ($LASTEXITCODE -ne 0) {
                throw "gh could not download the release. Is it signed in (gh auth login), and can this account see $repo?"
            }
        } else {
            $base = if ($tag) { "https://github.com/$repo/releases/download/$tag" } else { "https://github.com/$repo/releases/latest/download" }
            Write-Host "Downloading $base/$asset"
            try {
                Invoke-WebRequest -UseBasicParsing "$base/$asset" -OutFile (Join-Path $work $asset)
                Invoke-WebRequest -UseBasicParsing "$base/$asset.sha256" -OutFile (Join-Path $work "$asset.sha256")
            } catch {
                throw "Could not download $asset ($($_.Exception.Message)). While $repo is private, install the GitHub CLI, run 'gh auth login' and try again."
            }
        }

        $expected = ((Get-Content (Join-Path $work "$asset.sha256") -Raw).Trim() -split "\s+")[0].ToLower()
        $actual = (Get-FileHash (Join-Path $work $asset) -Algorithm SHA256).Hash.ToLower()
        if ($expected -ne $actual) {
            throw "The checksum of $asset does not match (expected $expected, got $actual). Nothing was installed."
        }
        Write-Host "Checksum verified."

        $unpacked = Join-Path $work "unpacked"
        Expand-Archive -Path (Join-Path $work $asset) -DestinationPath $unpacked
        New-Item -ItemType Directory -Force -Path $bin | Out-Null
        foreach ($program in "kstar.exe", "kstar-lsp.exe") {
            $from = Get-ChildItem -Path $unpacked -Recurse -Filter $program | Select-Object -First 1
            if (-not $from) { throw "$program is missing from $asset." }
            Copy-Item $from.FullName (Join-Path $bin $program) -Force
        }
    } finally {
        Remove-Item -Recurse -Force $work -ErrorAction SilentlyContinue
    }

    $kstar = Join-Path $bin "kstar.exe"
    Write-Host ""
    & $kstar version
    Write-Host ""
    Write-Host "Installed kstar and kstar-lsp in $bin"

    if ($env:KSTAR_NO_PATH -ne "1") {
        $userPath = [Environment]::GetEnvironmentVariable("Path", "User")
        $entries = @($userPath -split ";" | Where-Object { $_ -ne "" })
        if (-not ($entries | Where-Object { $_.TrimEnd("\") -ieq $bin.TrimEnd("\") })) {
            [Environment]::SetEnvironmentVariable("Path", (($entries + $bin) -join ";"), "User")
            Write-Host "Added $bin to your PATH. Open a new terminal for it to take effect."
        } else {
            Write-Host "$bin is already on your PATH."
        }
        if (-not (($env:Path -split ";") | Where-Object { $_.TrimEnd("\") -ieq $bin.TrimEnd("\") })) {
            $env:Path = "$env:Path;$bin"
        }
    }

    Write-Host ""
    Write-Host "To uninstall: delete $dir and remove $bin from your user PATH"
    Write-Host "(Settings > System > About > Advanced system settings > Environment Variables)."
}

Install-KleeneStar
