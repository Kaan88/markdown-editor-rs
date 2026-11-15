# Saves the screen rectangle X,Y,W,H (device px) to a PNG. A screen capture is what the compositor
# put out: close to, but not proof of, what the eye sees.
param([int]$X, [int]$Y, [int]$W, [int]$H, [string]$Out)
Add-Type -AssemblyName System.Drawing
Add-Type @'
using System.Runtime.InteropServices;
public static class Dpi { [DllImport("user32.dll")] public static extern bool SetProcessDPIAware(); }
'@
[void][Dpi]::SetProcessDPIAware()
$bmp = New-Object System.Drawing.Bitmap $W, $H
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.CopyFromScreen($X, $Y, 0, 0, $bmp.Size)
$bmp.Save($Out, [System.Drawing.Imaging.ImageFormat]::Png)
$g.Dispose(); $bmp.Dispose()
