param([Parameter(Mandatory=$true)][string]$App, [string]$Output = 'evidence/ui')
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
New-Item -ItemType Directory -Force $Output | Out-Null
$p = Start-Process -FilePath $App -PassThru
try {
  Start-Sleep -Seconds 10
  $p.Refresh()
  if ($p.HasExited -or $p.MainWindowHandle -eq 0) { throw 'No candidate window' }
  $root = [System.Windows.Automation.AutomationElement]::FromHandle($p.MainWindowHandle)
  $all = $root.FindAll([System.Windows.Automation.TreeScope]::Descendants, [System.Windows.Automation.Condition]::TrueCondition)
  @($all | ForEach-Object { @{ name=$_.Current.Name; type=$_.Current.ControlType.ProgrammaticName; enabled=$_.Current.IsEnabled; rect=$_.Current.BoundingRectangle.ToString() } }) | ConvertTo-Json -Depth 5 | Set-Content "$Output/native-elements.json"
  Write-Host "Native accessible elements: $($all.Count)"
  if ($all.Count -lt 10) { throw 'Native accessibility does not expose sufficient controls' }
} finally { if (-not $p.HasExited) { Stop-Process -Id $p.Id } }
