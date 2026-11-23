# Checks the version details Windows shows for kstar.exe, kstar-lsp.exe and
# kstar-setup-x64.exe (Properties > Details): the product name, the product
# and file version and the company. Code signing (CODE_SIGNING_POLICY.md)
# signs only files whose product name and version match, so CI checks them
# before anything is signed or released:
#
#   installer\check-version-info.ps1 -Version 0.8.1 -Files target\debug\kstar.exe, target\debug\kstar-lsp.exe
param(
    [Parameter(Mandatory = $true)][string]$Version,
    [Parameter(Mandatory = $true)][string[]]$Files
)
$ErrorActionPreference = "Stop"
$wrong = @()
foreach ($file in $Files) {
    $info = (Get-Item $file).VersionInfo
    $expected = [ordered]@{
        ProductName    = "Kleene Star"
        ProductVersion = $Version
        FileVersion    = $Version
        CompanyName    = "Kleene Star"
    }
    foreach ($name in $expected.Keys) {
        $actual = $info.$name
        if ($actual -ne $expected[$name]) {
            $wrong += "${file}: $name is '$actual', not '$($expected[$name])'"
        }
    }
    if (-not $info.LegalCopyright) { $wrong += "${file}: there is no copyright line" }
    $signature = (Get-AuthenticodeSignature $file).Status
    Write-Host "${file}: $($info.ProductName) $($info.ProductVersion), file $($info.FileVersion), $($info.CompanyName), $($info.LegalCopyright); signature: $signature"
}
if ($wrong.Count -gt 0) {
    $wrong | ForEach-Object { Write-Host "::error::$_" }
    throw "the version details are wrong"
}
