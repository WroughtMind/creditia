# Interact with the installed candidate using native controls and keyboard input.
param([Parameter(Mandatory=$true)][string]$App, [string]$Output = 'evidence/ui')
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
Add-Type -AssemblyName System.Windows.Forms
New-Item -ItemType Directory -Force $Output | Out-Null
$records = [System.Collections.Generic.List[object]]::new()
function Record($name,$detail='') { $records.Add(@{test=$name;detail=$detail}); Write-Host "$name $detail" }
function Find-Control([string]$name) {
  $p.Refresh()
  if ($p.HasExited) { throw 'Candidate exited' }
  if ($p.MainWindowHandle -eq 0) { return $null }
  $root = [System.Windows.Automation.AutomationElement]::FromHandle($p.MainWindowHandle)
  $condition = New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::NameProperty,$name)
  $matches = $root.FindAll([System.Windows.Automation.TreeScope]::Descendants,$condition)
  return $matches | Where-Object { -not $_.Current.IsOffscreen } | Select-Object -First 1
}
function Wait-Control([string]$name, [switch]$Enabled) {
  $until = [DateTime]::UtcNow.AddSeconds(90)
  do {
    try {
      $el = Find-Control $name
      if ($null -ne $el -and (-not $Enabled -or $el.Current.IsEnabled)) { return $el }
    } catch [System.Windows.Automation.ElementNotAvailableException] { }
    Start-Sleep -Milliseconds 500
  } while ([DateTime]::UtcNow -lt $until)
  throw "No native control: $name"
}
# The style label updates before paint() replaces the old artwork and its menus.
# Wait for the native image element to be replaced, rather than using a fixed delay.
function Wait-ReplacedControl([string]$name, [string]$previousId) {
  $until = [DateTime]::UtcNow.AddSeconds(120)
  do {
    try {
      $el = Find-Control $name
      if ($null -ne $el -and ($el.GetRuntimeId() -join ',') -ne $previousId) { return $el }
    } catch [System.Windows.Automation.ElementNotAvailableException] { }
    Start-Sleep -Milliseconds 500
  } while ([DateTime]::UtcNow -lt $until)
  throw "Native artwork was not replaced: $name"
}
function Click-Control([string]$name) {
  $el = Wait-Control $name -Enabled
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
    $matches = @($all | Where-Object { -not $_.Current.IsOffscreen -and $_.Current.Name -match $pattern })
    if ($matches.Count) { return $matches[0].Current.Name }
    Start-Sleep -Milliseconds 500
  } while ([DateTime]::UtcNow -lt $until)
  throw "No native text: $pattern"
}
function Send-Keys([string]$keys) {
  $shell = New-Object -ComObject WScript.Shell
  if (-not $shell.AppActivate($p.Id)) { throw 'Could not focus the isolated candidate window' }
  [System.Windows.Forms.SendKeys]::SendWait($keys)
}
function Assert-Hidden([string]$name) {
  $until = [DateTime]::UtcNow.AddSeconds(30)
  do {
    if ($null -eq (Find-Control $name)) { return }
    Start-Sleep -Milliseconds 100
  } while ([DateTime]::UtcNow -lt $until)
  throw "Control remained visible: $name"
}
$p = Start-Process -FilePath $App -PassThru
try {
  Click-Control '设置'
  Click-Control 'English'
  $null = Wait-Control 'General'
  Click-Control 'Close'
  $null = Wait-Control 'creditia · Share card'
  Record 'language switched to English'
  Send-Keys '^k'
  $null = Wait-Control 'More'
  Send-Keys '{ESC}'
  Assert-Hidden 'More'
  Send-Keys '^i'
  $null = Wait-Text '^Link a folder'
  Send-Keys '{ESC}'
  Assert-Hidden 'Project settings'
  Send-Keys '^,'
  $null = Wait-Control 'General'
  Send-Keys '{ESC}'
  Assert-Hidden 'General'
  $root = [System.Windows.Automation.AutomationElement]::FromHandle($p.MainWindowHandle)
  $menuBars = $root.FindAll([System.Windows.Automation.TreeScope]::Descendants,(New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::ControlTypeProperty,[System.Windows.Automation.ControlType]::MenuBar)))
  if (@($menuBars | Where-Object { -not $_.Current.IsOffscreen }).Count) { throw 'Unexpected menu bar in Windows window' }
  Record 'native shortcuts' 'Ctrl+K, Ctrl+I, Ctrl+comma and Escape; no extra menu bar'
  Click-Control 'By model'
  Click-Control 'By tool'
  Record 'model and tool grouping'
  Click-Control 'Switch project'
  Click-Control 'More'
  Click-Control (Wait-Text '^Re-analyze')
  $updated = Wait-Text '^Updated'
  Record 'native re-analysis completed' $updated
  $previousArt = (Wait-Control 'creditia · Share card').GetRuntimeId() -join ','
  Click-Control 'Next style'
  $null = Wait-Control 'Minimal Dark'
  $null = Wait-ReplacedControl 'creditia · Share card' $previousArt
  Record 'visual style switched' 'Minimal Dark'
  Send-Keys '^2'
  $null = Wait-Text 'creditia.*Share card'
  Send-Keys '^1'
  $null = Wait-Control 'creditia · Share card'
  Record 'wall and roam views' 'Ctrl+2 and Ctrl+1'
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
  $null = Wait-Control 'Settings'
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
