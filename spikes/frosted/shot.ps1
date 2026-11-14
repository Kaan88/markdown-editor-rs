# Runs the spike window for a few seconds and saves a screen capture of it (with 40 px around it,
# for the shadow and corners) to shots\<Name>.png, and its log to logs\<Name>.txt.
#   -Pattern   puts the spike's colourful pattern window behind it first
#   -SpikeArgs the spike's own arguments, as one string
# A screen capture (GDI CopyFromScreen) is what the compositor put on the screen; it is close to,
# but not proof of, what the eye sees.
param(
    [Parameter(Mandatory = $true)][string]$Name,
    [string]$SpikeArgs = "",
    [switch]$Pattern,
    [int]$HoldMs = 3500,
    [string]$Exe = "C:\Users\kaank\Desktop\md-wt\target-frosted\release\frosted-spike.exe"
)
Add-Type -AssemblyName System.Drawing
Add-Type @'
using System.Runtime.InteropServices;
public static class Dpi { [DllImport("user32.dll")] public static extern bool SetProcessDPIAware(); }
'@
[void][Dpi]::SetProcessDPIAware()
$here = Split-Path -Parent $MyInvocation.MyCommand.Path
New-Item -ItemType Directory -Force "$here\shots", "$here\logs" | Out-Null
$log = "$here\logs\$Name.txt"
$procs = @()
if ($Pattern) {
    $procs += Start-Process $Exe -PassThru -ArgumentList "--pattern --top --pos 160,110 --size 1000,660 --auto-exit-ms $($HoldMs + 1500)"
    Start-Sleep -Milliseconds 700
}
$procs += Start-Process $Exe -PassThru -ArgumentList "--top --pos 240,170 --auto-exit-ms $HoldMs --log `"$log`" $SpikeArgs"
Start-Sleep -Milliseconds ([int]($HoldMs * 0.6))
$line = Get-Content $log | Where-Object { $_ -match 'window rect (-?\d+),(-?\d+),(-?\d+),(-?\d+)' } | Select-Object -First 1
if ($line -match 'window rect (-?\d+),(-?\d+),(-?\d+),(-?\d+)') {
    $pad = 40
    $x = [int]$Matches[1] - $pad; $y = [int]$Matches[2] - $pad
    $w = [int]$Matches[3] - [int]$Matches[1] + 2 * $pad; $h = [int]$Matches[4] - [int]$Matches[2] + 2 * $pad
    $bmp = New-Object System.Drawing.Bitmap $w, $h
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.CopyFromScreen($x, $y, 0, 0, $bmp.Size)
    $bmp.Save("$here\shots\$Name.png", [System.Drawing.Imaging.ImageFormat]::Png)
    $g.Dispose(); $bmp.Dispose()
    "saved shots\$Name.png ($w x $h)"
} else {
    "no window rect in the log"
}
$procs | ForEach-Object { $_.WaitForExit(15000) | Out-Null }
Get-Content $log
