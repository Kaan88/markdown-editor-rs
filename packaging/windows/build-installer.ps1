<#
.SYNOPSIS
  Builds the Inno Setup installer for mdedit from the release exe (target\release\mdedit.exe),
  and writes its .sha256 file. Modelled on the viewer's packaging\windows\build-installer.ps1.
.NOTES
  pwsh packaging\windows\build-installer.ps1 [-SkipBuild]
  Output: target\installer\mdedit-setup-<version>.exe and .exe.sha256.
  tools\release.ps1 calls this after it has built the exe, with -SkipBuild.
#>
param([switch]$SkipBuild)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
function Fail($msg) { Write-Error $msg; exit 1 }

$root = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$toml = Get-Content -LiteralPath (Join-Path $root 'Cargo.toml') -Raw
$m = [regex]::Match($toml, '(?ms)^\[workspace\.package\]\s*(?:(?!^\[).)*?^version\s*=\s*"([^"]+)"')
if (-not $m.Success) { Fail 'Could not read [workspace.package] version from Cargo.toml.' }
$version = $m.Groups[1].Value

$iscc = $null; $cmd = Get-Command ISCC.exe -ErrorAction SilentlyContinue; if ($cmd) { $iscc = $cmd.Source }
if (-not $iscc) {
  foreach ($c in "$env:LOCALAPPDATA\Programs\Inno Setup 7\ISCC.exe", "${env:ProgramFiles(x86)}\Inno Setup 7\ISCC.exe",
                 "$env:ProgramFiles\Inno Setup 7\ISCC.exe", "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe") {
    if (Test-Path -LiteralPath $c) { $iscc = $c; break }
  }
}
if (-not $iscc) { Fail 'ISCC.exe not found.' }

if (-not $SkipBuild) {
  $tmp = Join-Path $root 'target\tmp'; New-Item -ItemType Directory -Force $tmp | Out-Null
  $env:TEMP = $tmp; $env:TMP = $tmp
  Push-Location $root
  try { cargo build --release -p mdedit } finally { Pop-Location }
  if ($LASTEXITCODE -ne 0) { Fail 'cargo build failed.' }
}
if (-not (Test-Path -LiteralPath (Join-Path $root 'target\release\mdedit.exe'))) { Fail 'target\release\mdedit.exe is missing.' }

& $iscc "/DAppVersion=$version" (Join-Path $PSScriptRoot 'mdedit.iss')
if ($LASTEXITCODE -ne 0) { Fail 'ISCC failed.' }
$setup = Join-Path $root "target\installer\mdedit-setup-$version.exe"
$hash = (Get-FileHash -LiteralPath $setup -Algorithm SHA256).Hash.ToLowerInvariant()
[System.IO.File]::WriteAllText("$setup.sha256", "$hash  mdedit-setup-$version.exe`n", [System.Text.UTF8Encoding]::new($false))
Write-Host "Built $setup"
Write-Host "SHA256 $hash"
