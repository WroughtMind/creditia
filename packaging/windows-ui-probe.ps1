param([Parameter(Mandatory=$true)][string]$App, [string]$Output = 'evidence/ui')
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
Add-Type -AssemblyName System.Drawing
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class CandidateWindow {
  [StructLayout(LayoutKind.Sequential)] public struct Rect { public int Left, Top, Right, Bottom; }
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hwnd, out Rect rect);
  [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr hwnd, IntPtr hdc, uint flags);
}
'@
New-Item -ItemType Directory -Force $Output | Out-Null
$p = Start-Process -FilePath $App -PassThru
try {
  Start-Sleep -Seconds 10
  $p.Refresh()
  if ($p.HasExited -or $p.MainWindowHandle -eq 0) { throw 'No candidate window' }
  $root = [System.Windows.Automation.AutomationElement]::FromHandle($p.MainWindowHandle)
  $all = $root.FindAll([System.Windows.Automation.TreeScope]::Descendants, [System.Windows.Automation.Condition]::TrueCondition)
  @($all | ForEach-Object { @{ name=$_.Current.Name; type=$_.Current.ControlType.ProgrammaticName; enabled=$_.Current.IsEnabled; rect=$_.Current.BoundingRectangle.ToString() } }) | ConvertTo-Json -Depth 5 | Set-Content "$Output/native-elements.json"
  Start-Sleep -Seconds 5
  $all = $root.FindAll([System.Windows.Automation.TreeScope]::Descendants, [System.Windows.Automation.Condition]::TrueCondition)
  @($all | ForEach-Object { @{ name=$_.Current.Name; type=$_.Current.ControlType.ProgrammaticName; enabled=$_.Current.IsEnabled } }) | ConvertTo-Json -Depth 5 | Set-Content "$Output/native-elements-after.json"
  # Print only this candidate window. Never capture the desktop or other apps.
  $rect = New-Object CandidateWindow+Rect
  if (-not [CandidateWindow]::GetWindowRect($p.MainWindowHandle, [ref]$rect)) { throw 'Cannot read candidate window bounds' }
  $bitmap = New-Object System.Drawing.Bitmap(($rect.Right-$rect.Left),($rect.Bottom-$rect.Top))
  $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
  $hdc = $graphics.GetHdc()
  try { if (-not [CandidateWindow]::PrintWindow($p.MainWindowHandle,$hdc,2)) { throw 'Cannot capture candidate window' } }
  finally { $graphics.ReleaseHdc($hdc) }
  $bitmap.Save("$((Resolve-Path $Output).Path)/candidate-window.png")
  $graphics.Dispose(); $bitmap.Dispose()
  Write-Host "Native accessible elements: $($all.Count)"
  if ($all.Count -lt 10) { throw 'Native accessibility does not expose sufficient controls' }
} finally { if (-not $p.HasExited) { Stop-Process -Id $p.Id } }
