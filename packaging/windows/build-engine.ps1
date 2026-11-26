<#
.SYNOPSIS
  Builds mdview-engine.exe, the helper process that draws the preview pane's formulas and
  diagrams, from Kaan88/markdown-shared-rs at the renderer revision resolved in mdedit's
  Cargo.lock, with the toolkit's lockfile and release profile and the static C runtime.
  The PowerShell twin of packaging/engine-source.sh.
.NOTES
  pwsh packaging\windows\build-engine.ps1
  Output: target\release\mdview-engine.exe (the checkout is target\engine-src, the build
  target\engine-build). Needs git and the network.
#>
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
function Fail($msg) { Write-Error $msg; exit 1 }

$root = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$lock = Get-Content -LiteralPath (Join-Path $root 'Cargo.lock') -Raw
$matchesAt = [regex]::Matches($lock, '(?m)^name = "mdshared-md-render"\r?\nversion = "[^"]+"\r?\nsource = "git\+https://github\.com/Kaan88/markdown-shared-rs\?rev=[0-9a-f]+#([0-9a-f]{40})"\r?$')
if ($matchesAt.Count -ne 1) { Fail 'No unique shared renderer revision in Cargo.lock.' }
$rev = $matchesAt[0].Groups[1].Value
$src = Join-Path $root 'target\engine-src'
$head = if (Test-Path -LiteralPath (Join-Path $src '.git')) { (& git -C $src rev-parse HEAD 2>$null) } else { '' }
if ($head -ne $rev) {
  if (Test-Path -LiteralPath $src) { Remove-Item -LiteralPath $src -Recurse -Force }
  & git init -q $src; if ($LASTEXITCODE -ne 0) { Fail 'git init failed' }
  & git -C $src fetch -q --depth 1 https://github.com/Kaan88/markdown-shared-rs $rev; if ($LASTEXITCODE -ne 0) { Fail 'git fetch failed' }
  & git -C $src -c advice.detachedHead=false checkout -q FETCH_HEAD; if ($LASTEXITCODE -ne 0) { Fail 'git checkout failed' }
}
Write-Host "engine source: markdown-shared-rs $rev"
$env:RUSTFLAGS = '-C target-feature=+crt-static'
try {
  & cargo build --release --locked -p mdshared-md-engine --bin mdview-engine --manifest-path (Join-Path $src 'Cargo.toml') --target-dir (Join-Path $root 'target\engine-build')
} finally { Remove-Item Env:RUSTFLAGS }
if ($LASTEXITCODE -ne 0) { Fail 'cargo build of mdview-engine failed' }
$out = Join-Path $root 'target\release'
New-Item -ItemType Directory -Force $out | Out-Null
Copy-Item -LiteralPath (Join-Path $root 'target\engine-build\release\mdview-engine.exe') -Destination $out -Force
Write-Host "built $(Join-Path $out 'mdview-engine.exe') ($((Get-Item (Join-Path $out 'mdview-engine.exe')).Length) bytes)"
