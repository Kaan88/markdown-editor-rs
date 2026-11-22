# Copied from the Hub repository (local, tools/write-hub-txt.ps1) at Hub commit 8caeef1; the script itself last changed in 26be12a. Do not edit here: change it in Hub and copy it again.
<#
.SYNOPSIS
  Writes hub.txt, the one manifest every app's release carries for Hub (docs/launcher-plan.md 3.4,
  grammar in crates/hub-core/src/manifest.rs). A release workflow calls this once, after the
  release files exist, and uploads the result as an asset named hub.txt.

.EXAMPLE
  pwsh tools/write-hub-txt.ps1 -App sprawl -Repo sprawl -Version 1.0.5 -Notes "Faster scans." `
    -Kind setup -Exe sprawl.exe -Uninstall Sprawl_is1 `
    -Files @{ Name='Sprawl-Setup-1.0.5.exe'; Path='dist/Sprawl-Setup-1.0.5.exe'; Role='setup';
              Silent='/VERYSILENT /SUPPRESSMSGBOXES /NORESTART'; Scope='machine' } `
    -Out dist/hub.txt

  Each file is a hashtable (or, with -FilesJson, a JSON array of objects) with:
    Name    the file name Hub places or runs: letters, digits . _ - only
    Path    the local file: its SHA-256 and size are computed from it
    Role    'file' (a placed executable; up to 9) or 'setup' (the installer; at most one) or 'zip'
    Asset   optional: the release asset's name when it differs from Name (default Name)
    Silent  setup only: the silent arguments; characters A-Z a-z 0-9 space / = _ . : -
    Scope   setup only: 'user' or 'machine'
    Sha256, Size   optional overrides, used only when Path is absent (samples, tests)

  The URL of a file is https://github.com/Kaan88/<Repo>/releases/download/v<Version>/<Asset>.
  Exits 1 with a message on stderr, writing nothing, when anything would not satisfy Hub's parser.
  Works in Windows PowerShell 5.1 and PowerShell 7.
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory)][ValidatePattern('^[a-z][a-z0-9-]*$')][string]$App,
    [Parameter(Mandatory)][string]$Repo,
    [Parameter(Mandatory)][string]$Version,
    [string]$Notes = '',
    [string]$NotesFile = '',
    [string]$Date = '',
    [ValidateSet('windows-x86_64', 'windows-aarch64', 'linux-x86_64', 'linux-aarch64', 'macos-aarch64', 'macos-x86_64')]
    [string]$Platform = 'windows-x86_64',
    [ValidateSet('setup', 'files', 'zip')][string]$Kind = 'files',
    [string]$Exe = '',
    [string]$Uninstall = '',
    [string]$Min = '',
    [string[]]$Wait = @(),
    [object[]]$Files = @(),
    [string]$FilesJson = '',
    [Parameter(Mandatory)][string]$Out
)
$ErrorActionPreference = 'Stop'

function Fail([string]$m) { [Console]::Error.WriteLine("write-hub-txt: $m"); exit 1 }

# Hub's own checks, repeated here so a bad release fails in the workflow and not on a user's PC.
function Test-Name([string]$v) {
    $v -match '^[A-Za-z0-9._-]{1,128}$' -and -not $v.StartsWith('.') -and -not $v.EndsWith('.') -and
        ($v.Split('.')[0].ToUpperInvariant() -notmatch '^(CON|PRN|AUX|NUL|COM[0-9]|LPT[0-9])$')
}
function Test-Rel([string]$v) {
    $v -ne '' -and $v.Length -le 128 -and $v -match '^[A-Za-z0-9._ \\/-]+$' -and $v -notmatch '(^|[\\/])\.\.([\\/]|$)' -and
        $v -notmatch '^[\\/]' -and $v -notmatch '^[A-Za-z]:'
}

if ($Version -notmatch '^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.]+)?$') { Fail "version '$Version' is not X.Y.Z[-pre]" }
if ($Date -eq '') { $Date = [DateTime]::UtcNow.ToString('yyyy-MM-dd') }
if ($Date -notmatch '^\d{4}-(0[1-9]|1[0-2])-(0[1-9]|[12]\d|3[01])$') { Fail "date '$Date' is not YYYY-MM-DD" }
$Notes = ($Notes -replace '[\p{Cc}]+', ' ').Trim()
if ($Notes.Length -gt 1024) { $Notes = $Notes.Substring(0, 1021) + '...' }
if ($NotesFile -ne '' -and -not (Test-Name $NotesFile)) { Fail "notes file name '$NotesFile' is not a plain file name" }
if ($Uninstall -ne '' -and $Uninstall -notmatch '^[A-Za-z0-9{}._-]{1,80}$') { Fail "uninstall key '$Uninstall' has characters Hub refuses" }
if ($Min -ne '' -and $Min -notmatch '^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.]+)?$') { Fail "min '$Min' is not a version" }
if ($Exe -ne '' -and -not (Test-Rel $Exe)) { Fail "exe '$Exe' is not a plain relative path" }
$waitList = @($Wait | ForEach-Object { $_ -split ',' } | ForEach-Object { $_.Trim() } | Where-Object { $_ -ne '' })
if ($waitList.Count -gt 8) { Fail 'wait lists at most 8 executables' }
foreach ($w in $waitList) { if (-not (Test-Rel $w)) { Fail "wait entry '$w' is not a plain relative path" } }

$list = @($Files)
if ($FilesJson -ne '') {
    $json = if (Test-Path -LiteralPath $FilesJson -PathType Leaf) { Get-Content -LiteralPath $FilesJson -Raw } else { $FilesJson }
    $list = @(ConvertFrom-Json -InputObject $json)
}
if ($list.Count -eq 0) { Fail 'no files' }

function Get-Field($o, [string]$k) {
    if ($o -is [System.Collections.IDictionary]) { if ($o.Contains($k)) { return $o[$k] } else { return $null } }
    $p = $o.PSObject.Properties[$k]
    if ($p) { return $p.Value } else { return $null }
}

$base = "https://github.com/Kaan88/$Repo/releases/download/v$Version"
$sb = New-Object System.Text.StringBuilder
$null = $sb.Append("hub: 1`n")
$null = $sb.Append("app: $App`n")
$null = $sb.Append("version: $Version`n")
$null = $sb.Append("date: $Date`n")
if ($Notes -ne '') { $null = $sb.Append("notes: $Notes`n") }
if ($NotesFile -ne '') { $null = $sb.Append("notes.file: $NotesFile`n") }
$null = $sb.Append("$Platform.kind: $Kind`n")
if ($Exe -ne '') { $null = $sb.Append("$Platform.exe: $Exe`n") }
if ($Uninstall -ne '') { $null = $sb.Append("$Platform.uninstall: $Uninstall`n") }
if ($Min -ne '') { $null = $sb.Append("$Platform.min: $Min`n") }
if ($waitList.Count -gt 0) { $null = $sb.Append("$Platform.wait: " + ($waitList -join ', ') + "`n") }

$names = @{}
$nFile = 0; $nSetup = 0; $nZip = 0
foreach ($f in $list) {
    $name = [string](Get-Field $f 'Name')
    $role = ([string](Get-Field $f 'Role')).ToLowerInvariant()
    if ($role -eq '') { $role = 'file' }
    $path = [string](Get-Field $f 'Path')
    $asset = [string](Get-Field $f 'Asset'); if ($asset -eq '') { $asset = $name }
    if (-not (Test-Name $name)) { Fail "file name '$name' is not a plain file name" }
    if (-not (Test-Name $asset)) { Fail "asset name '$asset' is not a plain file name" }
    if ($names.ContainsKey($name.ToLowerInvariant())) { Fail "file name '$name' is used twice" }
    $names[$name.ToLowerInvariant()] = $true
    if ($path -ne '' -and (Test-Path -LiteralPath $path -PathType Leaf)) {
        $sha = (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant()
        $size = (Get-Item -LiteralPath $path).Length
    } else {
        $sha = [string](Get-Field $f 'Sha256'); $size = Get-Field $f 'Size'
        if ($path -ne '' -and $sha -eq '') { Fail "file '$path' does not exist" }
        if ($sha -eq '' -or $null -eq $size) { Fail "file '$name' has no Path, or Sha256 and Size" }
    }
    if ($sha -notmatch '^[0-9a-f]{64}$') { Fail "sha256 of '$name' is not 64 lower-case hex digits" }
    if ([int64]$size -le 0) { Fail "'$name' is empty" }
    $size = [int64]$size
    switch ($role) {
        'file' {
            $nFile++
            if ($nFile -gt 9) { Fail 'at most 9 files' }
            $key = if ($nFile -eq 1) { "$Platform.file" } else { "$Platform.file$nFile" }
        }
        'zip' { $nZip++; if ($nZip -gt 1) { Fail 'at most one zip' }; $key = "$Platform.zip" }
        'setup' { $nSetup++; if ($nSetup -gt 1) { Fail 'at most one setup' }; $key = "$Platform.setup" }
        default { Fail "role '$role' is file, setup or zip" }
    }
    $null = $sb.Append("${key}: $name`n")
    $null = $sb.Append("$key.url: $base/$asset`n")
    $null = $sb.Append("$key.sha256: $sha`n")
    $null = $sb.Append("$key.size: $size`n")
    if ($role -eq 'setup') {
        $silent = [string](Get-Field $f 'Silent'); $scope = [string](Get-Field $f 'Scope')
        if ($silent -notmatch '^[A-Za-z0-9 /=_.:-]{1,200}$') { Fail "setup silent arguments '$silent' are empty or use characters Hub refuses" }
        if ($scope -ne 'user' -and $scope -ne 'machine') { Fail "setup scope '$scope' is user or machine" }
        $null = $sb.Append("$key.silent: $silent`n")
        $null = $sb.Append("$key.scope: $scope`n")
    }
}
switch ($Kind) {
    'setup' { if ($nSetup -ne 1) { Fail 'kind setup needs one file with Role setup' } }
    'files' { if ($nFile -lt 1) { Fail 'kind files needs at least one file' }; if ($Exe -eq '') { Fail 'kind files needs -Exe' } }
    'zip' { if ($nZip -ne 1) { Fail 'kind zip needs one file with Role zip' }; if ($Exe -eq '') { Fail 'kind zip needs -Exe' } }
}
$text = $sb.ToString()
$lines = ($text -split "`n").Count - 1
if ($lines -gt 128 -or [Text.Encoding]::UTF8.GetByteCount($text) -gt 16384) { Fail 'hub.txt would pass 128 lines or 16 KiB' }

$outFull = [IO.Path]::GetFullPath($Out)
$dir = Split-Path -Parent $outFull
if ($dir -and -not (Test-Path -LiteralPath $dir)) { New-Item -ItemType Directory -Path $dir | Out-Null }
[IO.File]::WriteAllText($outFull, $text, (New-Object System.Text.UTF8Encoding($false)))
Write-Host "write-hub-txt: wrote $Out ($lines lines)"
