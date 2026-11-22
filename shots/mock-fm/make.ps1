# Renders the file manager mockups (branch mock/file-manager). Needs `cargo build --release -p mdedit`
# (or pass -Exe). Every run is --behind --no-session, settings in a scratch folder under %TEMP%;
# --shot shows no window at all, and nothing of %LOCALAPPDATA%\mdedit is read or written.
param([string]$Out = "shots/mock-fm", [string]$Exe = "")
$root = Resolve-Path (Join-Path $PSScriptRoot "..\..")
if (-not $Exe) { $Exe = Join-Path $root "target\release\mdedit.exe" }
$scratch = Join-Path $env:TEMP "mock-fm-settings"
New-Item -ItemType Directory -Force $scratch | Out-Null
$env:MDEDIT_SETTINGS = Join-Path $scratch "settings.ini"
$names = "main","downloads","split","list","icons","preview","menu-tools","menu-view","context","context-shell","context-split","context-bg","queue","conflict","rename","delete","settings","compare","find"
foreach ($n in $names) {
  foreach ($t in "light","dark") {
    & $Exe --behind --no-session --shot (Join-Path $root "$Out\fm-$n-$t.png") --size 1280x800 --$t --mock "fm-$n"
  }
}
# The compact size (860x600) for the states that matter when the window is small.
foreach ($n in "main","split","icons","context","conflict","compare") {
  foreach ($t in "light","dark") {
    & $Exe --behind --no-session --shot (Join-Path $root "$Out\fm-$n-compact-$t.png") --size 860x600 --$t --mock "fm-$n"
  }
}
# The commander-style mockup.
foreach ($n in "main","vfs","lister","plugins") {
  foreach ($t in "light","dark") {
    & $Exe --behind --no-session --shot (Join-Path $root "$Out\cm-$n-$t.png") --size 1280x800 --$t --mock "cm-$n"
  }
}
foreach ($t in "light","dark") {
  & $Exe --behind --no-session --shot (Join-Path $root "$Out\cm-main-compact-$t.png") --size 860x600 --$t --mock "cm-main"
}
