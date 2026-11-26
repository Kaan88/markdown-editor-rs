<#
.SYNOPSIS
  Makes the portable zip of the release: mdedit-<version>-windows-x86_64.zip, one folder holding
  mdedit.exe, mdview-engine.exe (the preview pane's formulas and diagrams), mdedit-ai.exe (AI
  rewriting), LICENSE and README.md. Unzip it anywhere and run mdedit.exe; it updates itself in
  place like the installed copy (the folder must be writable).
.NOTES
  pwsh packaging\windows\build-zip.ps1 [-Exe path\to\mdedit.exe] [-Engine path\to\mdview-engine.exe] [-OutDir dir]
  Defaults: target\release\mdedit.exe, mdview-engine.exe beside it, target\dist\<version>. Run after tools\release.ps1.
#>
param([string]$Exe = '', [string]$Engine = '', [string]$Ai = '', [string]$OutDir = '')
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
function Fail($msg) { Write-Error $msg; exit 1 }

$root = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$toml = Get-Content -LiteralPath (Join-Path $root 'Cargo.toml') -Raw
$m = [regex]::Match($toml, '(?ms)^\[workspace\.package\]\s*(?:(?!^\[).)*?^version\s*=\s*"([^"]+)"')
if (-not $m.Success) { Fail 'Could not read [workspace.package] version from Cargo.toml.' }
$version = $m.Groups[1].Value
if (-not $Exe) { $Exe = Join-Path $root 'target\release\mdedit.exe' }
if (-not $OutDir) { $OutDir = Join-Path $root "target\dist\$version" }
if (-not $Ai) { $Ai = Join-Path $root 'target\release\mdedit-ai.exe' }
if (-not (Test-Path -LiteralPath $Ai)) { Fail "$Ai is missing." }
if (-not (Test-Path -LiteralPath $Exe)) { Fail "$Exe is missing." }
if (-not $Engine) { $Engine = Join-Path (Split-Path -Parent (Resolve-Path -LiteralPath $Exe).Path) 'mdview-engine.exe' }
if (-not (Test-Path -LiteralPath $Engine)) { Fail "$Engine is missing." }
New-Item -ItemType Directory -Force $OutDir | Out-Null

$name = "mdedit-$version-windows-x86_64"
$stage = Join-Path ([System.IO.Path]::GetTempPath()) ("zip-" + [guid]::NewGuid().ToString('N'))
$folder = Join-Path $stage $name
New-Item -ItemType Directory -Force $folder | Out-Null
Copy-Item -LiteralPath $Exe -Destination (Join-Path $folder 'mdedit.exe')
Copy-Item -LiteralPath $Engine -Destination (Join-Path $folder 'mdview-engine.exe')
Copy-Item -LiteralPath $Ai -Destination (Join-Path $folder 'mdedit-ai.exe')
Copy-Item -LiteralPath (Join-Path $root 'LICENSE') -Destination $folder
Copy-Item -LiteralPath (Join-Path $root 'README.md') -Destination $folder

$zip = Join-Path $OutDir "$name.zip"
if (Test-Path -LiteralPath $zip) { Remove-Item -LiteralPath $zip -Force }
# Entries with forward slashes, as the zip format wants; Compress-Archive of Windows PowerShell 5
# writes backslashes, so the archive is built by hand.
Add-Type -AssemblyName System.IO.Compression
Add-Type -AssemblyName System.IO.Compression.FileSystem
$archive = [System.IO.Compression.ZipFile]::Open($zip, 'Create')
try {
  foreach ($f in Get-ChildItem -LiteralPath $folder -File | Sort-Object Name) {
    [void][System.IO.Compression.ZipFileExtensions]::CreateEntryFromFile($archive, $f.FullName, "$name/$($f.Name)", 'Optimal')
  }
} finally { $archive.Dispose() }
Remove-Item -LiteralPath $stage -Recurse -Force
Write-Host "built $zip ($((Get-Item $zip).Length) bytes)"
