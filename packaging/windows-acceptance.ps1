# Interact with the installed candidate using Windows UI Automation only.
param([Parameter(Mandatory=$true)][string]$App, [string]$Output = 'evidence/ui')
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
New-Item -ItemType Directory -Force $Output | Out-Null
$records = [System.Collections.Generic.List[object]]::new()
function Record($name,$detail='') { $records.Add(@{test=$name;detail=$detail}); Write-Host "$name $detail" }
function Find-Control([string]$name) {
  $p.Refresh()
  if ($p.HasExited) { throw 'Candidate exited' }
  if ($p.MainWindowHandle -eq 0) { return $null }
  $root = [System.Windows.Automation.AutomationElement]::FromHandle($p.MainWindowHandle)
  $condition = New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::NameProperty,$name)
  return $root.FindFirst([System.Windows.Automation.TreeScope]::Descendants,$condition)
}
function Wait-Control([string]$name) {
  $until = [DateTime]::UtcNow.AddSeconds(90)
  do { $el = Find-Control $name; if ($null -ne $el) { return $el }; Start-Sleep -Milliseconds 500 } while ([DateTime]::UtcNow -lt $until)
  throw "No native control: $name"
}
function Click-Control([string]$name) {
  $el = Wait-Control $name
  if (-not $el.Current.IsEnabled) { throw "Disabled native control: $name" }
  Write-Host "Activating native control: $name"
  $pattern = $null
  if ($el.TryGetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern,[ref]$pattern)) { $pattern.Invoke(); return }
  if ($el.TryGetCurrentPattern([System.Windows.Automation.ExpandCollapsePattern]::Pattern,[ref]$pattern)) { $pattern.Expand(); return }
  if ($el.TryGetCurrentPattern([System.Windows.Automation.TogglePattern]::Pattern,[ref]$pattern)) { $pattern.Toggle(); return }
  if ($el.TryGetCurrentPattern([System.Windows.Automation.SelectionItemPattern]::Pattern,[ref]$pattern)) { $pattern.Select(); return }
  if ($el.TryGetCurrentPattern([System.Windows.Automation.LegacyIAccessiblePattern]::Pattern,[ref]$pattern)) { $pattern.DoDefaultAction(); return }
  throw "No supported native action on $name"
}
function Wait-Text([string]$pattern) {
  $until = [DateTime]::UtcNow.AddSeconds(120)
  do {
    $root = [System.Windows.Automation.AutomationElement]::FromHandle($p.MainWindowHandle)
    $all = $root.FindAll([System.Windows.Automation.TreeScope]::Descendants,[System.Windows.Automation.Condition]::TrueCondition)
    $matches = @($all | Where-Object { $_.Current.Name -match $pattern })
    if ($matches.Count) { return $matches[0].Current.Name }
    Start-Sleep -Milliseconds 500
  } while ([DateTime]::UtcNow -lt $until)
  throw "No native text: $pattern"
}
$p = Start-Process -FilePath $App -PassThru
try {
  $null = Wait-Control '切换到英文'
  Click-Control '切换到英文'
  $null = Wait-Control 'Switch to Chinese'
  Record 'language switched to English'
  Click-Control 'Switch project'
  Click-Control 'Re-analyze'
  $updated = Wait-Text '^Updated'
  Record 'native re-analysis completed' $updated
  Click-Control 'By model'
  Click-Control 'By tool'
  Record 'model and tool grouping'
  Click-Control 'Next style'
  $null = Wait-Control 'Minimal Dark'
  Record 'visual style switched' 'Minimal Dark'
  Click-Control 'Roam'
  $null = Wait-Text 'creditia.*Share card'
  Click-Control 'Wall'
  Record 'wall and roam views'
  $exportDir = Join-Path $env:USERPROFILE 'Downloads'
  foreach ($format in @('PNG','SVG','PDF','HTML','GIF')) {
    $before = @(Get-ChildItem $exportDir -File -ErrorAction SilentlyContinue | ForEach-Object Name)
    Click-Control 'Export'
    Click-Control $format
    $saved = Wait-Text '^Saved to '
    $files = @(Get-ChildItem $exportDir -File | Where-Object { $_.Name -notin $before -and $_.Extension -eq ".$($format.ToLower())" })
    if ($files.Count -ne 1 -or $files[0].Length -lt 100) { throw "No complete new $format file after $saved" }
    Copy-Item $files[0].FullName $Output
    Record "native $format export" @{file=$files[0].Name;bytes=$files[0].Length}
  }
  if (-not $p.CloseMainWindow()) { throw 'Could not close candidate window normally' }
  if (-not $p.WaitForExit(15000)) { throw 'Candidate did not exit after close' }
  $p = Start-Process -FilePath $App -PassThru
  $null = Wait-Control 'Switch to Chinese'
  $null = Wait-Control 'Minimal Dark'
  $null = Wait-Text 'creditia.*Share card'
  Record 'restart restores language, style and rendered project'
  @{passed=$true;application=$App;records=$records} | ConvertTo-Json -Depth 8 | Set-Content "$Output/result.json"
} catch {
  @{passed=$false;error="$($_.Exception.Message)";records=$records} | ConvertTo-Json -Depth 8 | Set-Content "$Output/result.json"
  throw
} finally {
  if (-not $p.HasExited) {
    $root = [System.Windows.Automation.AutomationElement]::FromHandle($p.MainWindowHandle)
    $all = $root.FindAll([System.Windows.Automation.TreeScope]::Descendants,[System.Windows.Automation.Condition]::TrueCondition)
    @($all | ForEach-Object { @{name=$_.Current.Name;type=$_.Current.ControlType.ProgrammaticName} }) | ConvertTo-Json -Depth 4 | Set-Content "$Output/native-elements.json"
    $null = $p.CloseMainWindow()
    if (-not $p.WaitForExit(5000)) { Stop-Process -Id $p.Id }
  }
}
