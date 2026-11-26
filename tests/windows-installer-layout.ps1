# Regression test for build-installer's preflight, without installing or invoking Inno Setup.
$ErrorActionPreference = 'Stop'
$root = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$scratch = Join-Path ([System.IO.Path]::GetTempPath()) ("mdedit-installer-" + [guid]::NewGuid())
$oldCompiler = $env:ISCC
try {
  $packaging = Join-Path $scratch 'packaging\windows'
  $release = Join-Path $scratch 'target\release'
  New-Item -ItemType Directory $packaging, $release | Out-Null
  Copy-Item (Join-Path $root 'Cargo.toml') $scratch
  Copy-Item -LiteralPath @(
    (Join-Path $root 'packaging\windows\build-installer.ps1'),
    (Join-Path $root 'packaging\windows\mdedit.iss')
  ) -Destination $packaging
  foreach ($name in 'mdedit.exe', 'mdedit-ai.exe', 'mdview-engine.exe') {
    Set-Content -LiteralPath (Join-Path $release $name) -Value "stub $name"
  }
  $env:ISCC = Join-Path $scratch 'fake-iscc.ps1'
  @'
$ErrorActionPreference = 'Stop'
$version = $args[0] -replace '^/DAppVersion=', ''
$script = $args[1]
$source = [regex]::Match((Get-Content -LiteralPath $script -Raw), '(?m)^Source: "([^"]*mdview-engine\.exe)"')
if (-not $source.Success -or -not (Test-Path -LiteralPath (Join-Path (Split-Path $script) $source.Groups[1].Value) -PathType Leaf)) {
  throw 'Inno Setup source is missing.'
}
$out = Join-Path $PSScriptRoot 'target\installer'
New-Item -ItemType Directory $out | Out-Null
Set-Content -LiteralPath (Join-Path $out "mdedit-setup-$version.exe") -Value 'stub installer'
exit 0
'@ | Set-Content -LiteralPath $env:ISCC
  if (Test-Path -LiteralPath (Join-Path $scratch 'target\engine')) { throw 'Fixture is not clean.' }
  & (Join-Path $packaging 'build-installer.ps1') -SkipBuild
  if (-not (Get-ChildItem -LiteralPath (Join-Path $scratch 'target\installer') -Filter '*.exe.sha256')) {
    throw 'Installer preflight did not reach the compiler.'
  }
} finally {
  $env:ISCC = $oldCompiler
  if (Test-Path -LiteralPath $scratch) { Remove-Item -LiteralPath $scratch -Recurse -Force }
}
