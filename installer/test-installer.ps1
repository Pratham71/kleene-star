# Tests kstar-setup-x64.exe on a Windows machine, as the release workflow
# does before publishing it: a silent per-user install with both tasks, then
# checks that kstar runs, that its folder is on the PATH and that .kstar is
# associated; then a silent uninstall, and checks that all three are gone.
#
#   powershell -File installer\test-installer.ps1 -Setup dist\kstar-setup-x64.exe
#
# It changes the current user's PATH and file associations while it runs,
# so it is meant for a throwaway CI machine.

param(
    [Parameter(Mandatory = $true)][string]$Setup,
    # Fail if VS Code is not installed, instead of skipping the extension checks.
    [switch]$ExpectVSCode
)

$ErrorActionPreference = "Stop"
$dir = Join-Path $env:LOCALAPPDATA "Programs\Kleene Star"
$bin = Join-Path $dir "bin"

function Fail($message) {
    Write-Host "FAILED: $message"
    exit 1
}

function UserPath {
    (Get-ItemProperty -Path HKCU:\Environment -Name Path -ErrorAction SilentlyContinue).Path
}

function OnPath {
    $entries = (UserPath) -split ";" | ForEach-Object { $_.TrimEnd("\") }
    $entries -contains $bin
}

# VS Code's command line, found the way the installer finds it.
$code = @(
    (Get-Command code.cmd -ErrorAction SilentlyContinue).Source,
    (Join-Path $env:LOCALAPPDATA "Programs\Microsoft VS Code\bin\code.cmd"),
    (Join-Path $env:ProgramFiles "Microsoft VS Code\bin\code.cmd")
) | Where-Object { $_ -and (Test-Path $_) } | Select-Object -First 1
if (-not $code -and $ExpectVSCode) { Fail "VS Code is not installed" }
if ($code) { Write-Host "VS Code: $code" } else { Write-Host "VS Code is not installed; skipping the extension checks" }

function HasExtension {
    (& $code --list-extensions 2>$null) -contains "kleene-star.kleene-star"
}

if ($code -and (HasExtension)) { Fail "the extension is installed before the test starts" }

Write-Host "Installing $Setup"
$install = Start-Process -FilePath $Setup -Wait -PassThru -ArgumentList `
    "/VERYSILENT", "/SUPPRESSMSGBOXES", "/NORESTART", "/CURRENTUSER", "/TASKS=addtopath,associate,vscode", "/LOG=install.log"
if ($install.ExitCode -ne 0) { Get-Content install.log; Fail "the installer exited with $($install.ExitCode)" }

$kstar = Join-Path $bin "kstar.exe"
if (-not (Test-Path $kstar)) { Fail "$kstar was not installed" }
if (-not (Test-Path (Join-Path $bin "kstar-lsp.exe"))) { Fail "kstar-lsp.exe was not installed" }
$version = & $kstar version
Write-Host $version
if ($LASTEXITCODE -ne 0 -or -not ($version -match "^kstar \d")) { Fail "kstar version did not work" }
if (-not (OnPath)) { Fail "$bin is not on the user's PATH: $(UserPath)" }
$association = (Get-ItemProperty -Path "HKCU:\Software\Classes\.kstar" -ErrorAction SilentlyContinue)."(default)"
if ($association -ne "KleeneStar.Program") { Fail ".kstar is not associated (found '$association')" }
$icon = (Get-ItemProperty -Path "HKCU:\Software\Classes\KleeneStar.Program\DefaultIcon")."(default)"
if (-not (Test-Path $icon)) { Fail "the file icon $icon does not exist" }
Write-Host "Installed: kstar runs, the PATH and the association are set."
if ($code) {
    if (-not (HasExtension)) { Get-Content install.log | Select-String "VS Code"; Fail "the extension was not installed in VS Code" }
    Write-Host "Installed: the extension is in VS Code."
}

# A second install over the first must not add the folder to the PATH twice.
Start-Process -FilePath $Setup -Wait -ArgumentList "/VERYSILENT", "/SUPPRESSMSGBOXES", "/NORESTART", "/CURRENTUSER", "/TASKS=addtopath,associate,vscode"
$count = ((UserPath) -split ";" | Where-Object { $_.TrimEnd("\") -eq $bin }).Count
if ($count -ne 1) { Fail "after reinstalling, the PATH holds the folder $count times" }

Write-Host "Uninstalling"
Start-Process -FilePath (Join-Path $dir "unins000.exe") -Wait -ArgumentList "/VERYSILENT", "/SUPPRESSMSGBOXES", "/NORESTART"
# The uninstaller hands over to a copy of itself and returns at once, so
# wait for it to finish removing the folder.
for ($i = 0; $i -lt 120 -and (Test-Path $kstar); $i++) { Start-Sleep -Milliseconds 500 }
if (Test-Path $kstar) { Fail "kstar.exe is still there after uninstalling" }
if (OnPath) { Fail "$bin is still on the PATH after uninstalling" }
if (Test-Path "HKCU:\Software\Classes\KleeneStar.Program") { Fail "the KleeneStar.Program key is still there" }
$association = (Get-ItemProperty -Path "HKCU:\Software\Classes\.kstar" -ErrorAction SilentlyContinue)."(default)"
if ($association) { Fail ".kstar is still associated with '$association'" }
if ($code -and (HasExtension)) { Fail "the extension is still in VS Code after uninstalling" }
Write-Host "Uninstalled: the files, the PATH entry, the association and the extension are gone."
