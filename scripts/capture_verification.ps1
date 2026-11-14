$env:PATH = "D:\dev\Qt\6.8.2\mingw_64\bin;D:\dev\Qt\Tools\mingw1310_64\bin;" + $env:PATH
$exe = "E:\bw\CESIUM\3dtiles-editor\build\3dtiles-editor.exe"
$tileset = "D:\BaiduNetdiskDownload\SXNWDSMY\gygcdm\tileset.json"
$proc = Start-Process -FilePath $exe -ArgumentList "`"$tileset`"" -PassThru
Start-Sleep -Seconds 7

Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing

Add-Type -ReferencedAssemblies "System.Drawing.dll" -TypeDefinition @"
using System;
using System.Text;
using System.Drawing;
using System.Runtime.InteropServices;
public class WinCapFinal {
    public delegate bool EnumWindowsProc(IntPtr hWnd, IntPtr lParam);
    [DllImport("user32.dll")]
    public static extern bool EnumChildWindows(IntPtr hWndParent, EnumWindowsProc lpEnumFunc, IntPtr lParam);
    [DllImport("user32.dll")]
    public static extern bool GetWindowRect(IntPtr hWnd, out RECT lpRect);
    [DllImport("user32.dll")]
    public static extern bool SetForegroundWindow(IntPtr hWnd);
    [DllImport("user32.dll")]
    public static extern IntPtr GetDC(IntPtr hWnd);
    [DllImport("user32.dll")]
    public static extern int ReleaseDC(IntPtr hWnd, IntPtr hDC);
    [DllImport("gdi32.dll")]
    public static extern bool BitBlt(IntPtr hObject, int nXDest, int nYDest, int nWidth, int nHeight, IntPtr hObjSource, int nXSrc, int nYSrc, int dwRop);

    [StructLayout(LayoutKind.Sequential)]
    public struct RECT { public int Left, Top, Right, Bottom; }

    public static Bitmap CaptureChildDc(IntPtr hWnd, int w, int h) {
        IntPtr hdcSrc = GetDC(hWnd);
        Bitmap bmp = new Bitmap(w, h);
        using (Graphics g = Graphics.FromImage(bmp)) {
            IntPtr hdcDest = g.GetHdc();
            BitBlt(hdcDest, 0, 0, w, h, hdcSrc, 0, 0, 0x00CC0020);
            g.ReleaseHdc(hdcDest);
        }
        ReleaseDC(hWnd, hdcSrc);
        return bmp;
    }
}
"@

$proc.Refresh()
$hwnd = $proc.MainWindowHandle
[WinCapFinal]::SetForegroundWindow($hwnd)
Start-Sleep -Milliseconds 600

$childHwnds = New-Object System.Collections.Generic.List[IntPtr]
$enumProc = [WinCapFinal+EnumWindowsProc]{
    param($h, $l)
    $childHwnds.Add($h)
    return $true
}
[WinCapFinal]::EnumChildWindows($hwnd, $enumProc, [IntPtr]::Zero)

$ch = $childHwnds[0]
$chRect = New-Object WinCapFinal+RECT
[WinCapFinal]::GetWindowRect($ch, [ref]$chRect)
$cw = $chRect.Right - $chRect.Left
$chH = $chRect.Bottom - $chRect.Top

# Capture child DC (Vulkan 3D Viewport)
$childBmp = [WinCapFinal]::CaptureChildDc($ch, $cw, $chH)

# Capture parent window (Qt Quick UI)
$pRect = New-Object WinCapFinal+RECT
[WinCapFinal]::GetWindowRect($hwnd, [ref]$pRect)
$pw = $pRect.Right - $pRect.Left
$ph = $pRect.Bottom - $pRect.Top

$parentBmp = New-Object System.Drawing.Bitmap $pw, $ph
$gParent = [System.Drawing.Graphics]::FromImage($parentBmp)
$gParent.CopyFromScreen($pRect.Left, $pRect.Top, 0, 0, (New-Object System.Drawing.Size $pw, $ph))

# Composite child viewport directly into the parent window at exact child offset
$relX = $chRect.Left - $pRect.Left
$relY = $chRect.Top - $pRect.Top
Write-Host "Compositing child viewport at ($relX, $relY), size: $cw x $chH on parent $pw x $ph"

$gParent.DrawImage($childBmp, $relX, $relY, $cw, $chH)

# Save the final verified full editor screenshot
$outFile = "E:\bw\CESIUM\3dtiles-editor\editor_verified_screen.png"
$parentBmp.Save($outFile, [System.Drawing.Imaging.ImageFormat]::Png)

$childBmp.Dispose()
$gParent.Dispose()
$parentBmp.Dispose()
Write-Host "Successfully saved $outFile"

Stop-Process -Id $proc.Id -Force
