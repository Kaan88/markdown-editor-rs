# Renders the launcher mockups (branch mock/launcher). Needs `cargo build --release -p mdedit`.
# Every run is --behind --no-session, with its settings in a scratch folder under %TEMP%, and
# --shot never shows a window at all; nothing of %LOCALAPPDATA%\mdedit is read or written.
param([string]$Out = "shots/mock-hub")
$root = Resolve-Path (Join-Path $PSScriptRoot "..\..")
$exe = Join-Path $root "target\release\mdedit.exe"
$scratch = Join-Path $env:TEMP "mock-hub-settings"
New-Item -ItemType Directory -Force $scratch | Out-Null
$env:MDEDIT_SETTINGS = Join-Path $scratch "settings.ini"
$names = "main","menu","updating","running","notes","failed","rollback","first-run","tray","settings-updates","settings-general","self-update","linux","icons","icons-small","icons-updating","icons-failed"
foreach ($n in $names) {
  foreach ($t in "light","dark") {
    & $exe --behind --no-session --shot (Join-Path $root "$Out\$n-$t.png") --size 1280x800 --$t --mock "hub-$n"
  }
}
# The compact size (860x600) for the states that matter when the window is small.
foreach ($n in "main","updating","failed","first-run","icons","icons-small","icons-updating") {
  foreach ($t in "light","dark") {
    & $exe --behind --no-session --shot (Join-Path $root "$Out\$n-compact-$t.png") --size 860x600 --$t --mock "hub-$n"
  }
}
# The report goes to stderr of a program with no console, so it is captured through cmd.
$tmp = Join-Path $scratch "report.png"
cmd /c "`"$exe`" --behind --no-session --shot `"$tmp`" --size 1280x800 --dark --mock hub-report 2> `"$(Join-Path $root "$Out\numbers.txt")`""
