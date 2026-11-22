# Renders the task manager mockups (branch mock/task-manager). Needs `cargo build --release -p mdedit`
# (or pass -Exe). Every run is --behind --no-session, settings in a scratch folder under %TEMP%;
# --shot shows no window at all, and nothing of %LOCALAPPDATA%\mdedit is read or written.
param([string]$Out = "shots/mock-tm", [string]$Exe = "")
$root = Resolve-Path (Join-Path $PSScriptRoot "..\..")
if (-not $Exe) { $Exe = Join-Path $root "target\release\mdedit.exe" }
$scratch = Join-Path $env:TEMP "mock-tm-settings"
New-Item -ItemType Directory -Force $scratch | Out-Null
$env:MDEDIT_SETTINGS = Join-Path $scratch "settings.ini"
function Shot($name, $size, $file) {
  foreach ($t in "light","dark") {
    & $Exe --behind --no-session --shot (Join-Path $root "$Out\$file-$t.png") --size $size --$t --mock "tm-$name"
  }
}
$full = "procs","tree","flat","search","details","details-modules","details-handles","details-net","details-env","details-denied","context","endtree","affinity","elevate","perf-cpu","perf-cores","perf-mem","perf-disk","perf-net","perf-gpu","startup","startup-admin","services","files","files-deep","settings","privacy","cost"
foreach ($n in $full) { Shot $n "1280x800" "tm-$n" }
# The compact size (860x600) for the states that matter when the window is small.
foreach ($n in "procs","tree","details","context","perf-cpu","perf-cores","startup","services","files") { Shot $n "860x600" "tm-$n-compact" }
# The tray flyout and the compact always-on-top window have their own sizes.
Shot "tray" "720x620" "tm-tray"
Shot "compact" "380x590" "tm-compact"
