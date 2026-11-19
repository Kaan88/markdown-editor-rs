<#
  Builds the release executable, writes the update manifest, builds the Inno Setup installer
  (packaging\windows\mdedit.iss), and with -Publish creates the GitHub release v<version>
  (targeting the current commit) with the three assets.

  tools\release.ps1 -Notes 'One short sentence.'             build and check only
  tools\release.ps1 -Notes 'One short sentence.' -Publish    also publish

  Output: target\dist\<version>\mdedit-<version>-windows-x86_64.exe, mdview-engine.exe (the
  preview pane's engine, packaging\windows\build-engine.ps1), mdedit-ai.exe (the AI engine),
  mdedit-update.txt (format 1, read by 1.0.6 and older, without mdedit-ai), mdedit-update-2.txt
  (format 2, read by 1.1 and later, with it) and mdedit-setup-<version>.exe
  Changes under tools\ and docs\ do not count as a dirty tree (the exe does not depend on them).
#>
param(
  [string]$Notes = 'A new version of mdedit.',
  [string]$ReleaseNotes = 'mdedit is a light native text editor, the companion of the Markdown viewer mdview.',
  [switch]$Publish
)
$ErrorActionPreference = 'Stop'
$repo = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
Set-Location $repo
$slug = 'Kaan88/markdown-editor-rs'

function Fail($m) { Write-Error $m; exit 1 }
function RunGit { $out = & git @args; if ($LASTEXITCODE -ne 0) { Fail "git $args failed" }; $out }

if ($Notes -match '[\x00-\x1f]' -or $Notes.Length -gt 1000) { Fail 'Notes: one line, no control characters.' }

# Version from Cargo metadata.
$meta = (& cargo metadata --no-deps --format-version 1) | ConvertFrom-Json
if ($LASTEXITCODE -ne 0) { Fail 'cargo metadata failed' }
$version = ($meta.packages | Where-Object { $_.name -eq 'mdedit' }).version
if (-not $version) { Fail 'package mdedit not found' }

# Clean tree (tools/ and docs/ excepted) and pushed HEAD.
$dirty = RunGit status --porcelain -- . ':(exclude)tools' ':(exclude)docs'
if ($dirty) { Fail "Working tree is dirty:`n$dirty" }
$sha = (RunGit rev-parse HEAD).Trim()
RunGit fetch --quiet origin
$remote = RunGit branch -r --contains $sha
if (-not $remote) { Fail "HEAD $sha is not on any remote branch; push it first." }

# Build with the temp dir inside the repo's target folder.
$tmp = Join-Path $repo 'target\tmp'
New-Item -ItemType Directory -Force $tmp | Out-Null
$env:TEMP = $tmp
$env:TMP = $tmp
& cargo build --release -p mdedit -p mdedit-ai
if ($LASTEXITCODE -ne 0) { Fail 'cargo build failed' }
& (Join-Path $repo 'packaging\windows\build-engine.ps1')
if ($LASTEXITCODE -ne 0) { Fail 'engine build failed' }

$dist = Join-Path $repo "target\dist\$version"
if (Test-Path $dist) { Remove-Item -Recurse -Force $dist }
New-Item -ItemType Directory -Force $dist | Out-Null
$asset = "mdedit-$version-windows-x86_64.exe"
$exe = Join-Path $dist $asset
Copy-Item (Join-Path $repo 'target\release\mdedit.exe') $exe
$hash = (Get-FileHash -LiteralPath $exe -Algorithm SHA256).Hash.ToLowerInvariant()
$engine = Join-Path $dist 'mdview-engine.exe'
Copy-Item (Join-Path $repo 'target\engine\mdview-engine.exe') $engine
$engineHash = (Get-FileHash -LiteralPath $engine -Algorithm SHA256).Hash.ToLowerInvariant()
$ai = Join-Path $dist 'mdedit-ai.exe'
Copy-Item (Join-Path $repo 'target\release\mdedit-ai.exe') $ai
$aiHash = (Get-FileHash -LiteralPath $ai -Algorithm SHA256).Hash.ToLowerInvariant()
$aiUrl = "https://github.com/$slug/releases/download/v$version/mdedit-ai.exe"

# The executable and the engine beside it: the platform entry and its engine keys.
$url = "https://github.com/$slug/releases/download/v$version/$asset"
$engineUrl = "https://github.com/$slug/releases/download/v$version/mdview-engine.exe"
$manifest = Join-Path $dist 'mdedit-update.txt'
$lines = @(
  'manifest: 1',
  "version: $version",
  "notes: $Notes",
  "windows-x86_64.asset: $asset",
  "windows-x86_64.url: $url",
  "windows-x86_64.sha256: $hash",
  "windows-x86_64.engine.url: $engineUrl",
  "windows-x86_64.engine.sha256: $engineHash"
)
[System.IO.File]::WriteAllText($manifest, (($lines -join "`n") + "`n"), [System.Text.UTF8Encoding]::new($false))
# Format 2, read by 1.1 and later: the same, and the AI engine beside the exe.
$manifest2 = Join-Path $dist 'mdedit-update-2.txt'
$lines2 = @('manifest: 2') + $lines[1..($lines.Count - 1)] + @(
  "windows-x86_64.mdedit-ai.url: $aiUrl",
  "windows-x86_64.mdedit-ai.sha256: $aiHash"
)
[System.IO.File]::WriteAllText($manifest2, (($lines2 -join "`n") + "`n"), [System.Text.UTF8Encoding]::new($false))
if ((Get-Content -LiteralPath $manifest2)[-1] -ne "windows-x86_64.mdedit-ai.sha256: $aiHash") { Fail 'manifest 2 does not match' }
if ((Get-FileHash -LiteralPath $ai -Algorithm SHA256).Hash.ToLowerInvariant() -ne $aiHash) { Fail 'AI engine hash mismatch' }

# Re-read the manifest and compare with the file.
$back = Get-Content -LiteralPath $manifest
$want = @{ 'version' = $version; 'windows-x86_64.asset' = $asset; 'windows-x86_64.url' = $url; 'windows-x86_64.sha256' = $hash
           'windows-x86_64.engine.url' = $engineUrl; 'windows-x86_64.engine.sha256' = $engineHash }
if ($back[0] -ne 'manifest: 1') { Fail 'manifest header wrong' }
foreach ($k in $want.Keys) {
  $line = $back | Where-Object { $_.StartsWith("${k}: ") }
  if (-not $line -or $line.Substring($k.Length + 2) -ne $want[$k]) { Fail "manifest key $k does not match" }
}
if ((Get-FileHash -LiteralPath $exe -Algorithm SHA256).Hash.ToLowerInvariant() -ne $hash) { Fail 'hash mismatch' }
if ((Get-FileHash -LiteralPath $engine -Algorithm SHA256).Hash.ToLowerInvariant() -ne $engineHash) { Fail 'engine hash mismatch' }
if ((Get-Item $manifest).Length -gt 8192) { Fail 'manifest too large' }
# The installer, an extra asset: the manifest names the bare exe and the engine, which is what the updater installs.
& (Join-Path $repo 'packaging\windows\build-installer.ps1') -SkipBuild
if ($LASTEXITCODE -ne 0) { Fail 'installer build failed' }
$setupName = "mdedit-setup-$version.exe"
$setup = Join-Path $dist $setupName
Copy-Item (Join-Path $repo "target\installer\$setupName") $setup
Write-Host "built $setupName ($((Get-Item $setup).Length) bytes)"
Write-Host "built $asset ($((Get-Item $exe).Length) bytes) sha256 $hash from $sha"

if (-not $Publish) { Write-Host "not published (no -Publish). Files are in $dist"; exit 0 }

& gh release create "v$version" $exe $engine $ai $manifest $manifest2 $setup --repo $slug --target $sha --title "mdedit $version" --notes $ReleaseNotes --latest
if ($LASTEXITCODE -ne 0) { Fail 'gh release create failed' }
Write-Host "published https://github.com/$slug/releases/tag/v$version"
