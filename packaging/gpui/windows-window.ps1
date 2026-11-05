param(
  [Parameter(Mandatory=$true)][string]$PackageRoot,
  [Parameter(Mandatory=$true)][string]$Fixtures,
  [Parameter(Mandatory=$true)][string]$Output
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName System.Windows.Forms
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class CreditiaWindow {
  [StructLayout(LayoutKind.Sequential)] public struct Rect { public int Left, Top, Right, Bottom; }
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr handle, out Rect rect);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr handle);
  [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
  [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr handle, int command);
  [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr handle, IntPtr after, int x, int y, int width, int height, uint flags);
  [DllImport("user32.dll")] public static extern uint GetDpiForWindow(IntPtr handle);
  [DllImport("user32.dll")] public static extern bool SetProcessDpiAwarenessContext(IntPtr context);
}
'@
[CreditiaWindow]::SetProcessDpiAwarenessContext([IntPtr](-4)) | Out-Null
$PackageRoot = [IO.Path]::GetFullPath($PackageRoot)
$Fixtures = [IO.Path]::GetFullPath($Fixtures)
$Output = [IO.Path]::GetFullPath($Output)
[IO.Directory]::CreateDirectory($Output) | Out-Null
$manifest = Get-Content (Join-Path $Fixtures 'fixture-manifest.json') -Raw | ConvertFrom-Json
$cases = @()

function Wait-AppState($process,[string]$events,[scriptblock]$predicate,[int]$seconds=30) {
  $deadline=[DateTime]::UtcNow.AddSeconds($seconds)
  do {
    $process.Refresh()
    if($process.HasExited){throw "Native process exited $($process.ExitCode)"}
    $state=$null
    if(Test-Path $events){
      foreach($line in (Get-Content $events)){
        try{$event=$line|ConvertFrom-Json;if($event.event -eq 'state'){$state=$event.state}}catch{} # Last line may be in flight.
      }
    }
    if($null -ne $state -and (& $predicate $state)){return $state}
    Start-Sleep -Milliseconds 100
  } while([DateTime]::UtcNow -lt $deadline)
  throw "Native state did not satisfy the input assertion: $($state|ConvertTo-Json -Compress)"
}

function Capture-Window($process, [string]$name) {
  $process.Refresh()
  $handle = $process.MainWindowHandle
  if ($handle -eq [IntPtr]::Zero) { throw 'Native application has no top-level window' }
  [CreditiaWindow]::ShowWindow($handle,9) | Out-Null
  [CreditiaWindow]::SetForegroundWindow($handle) | Out-Null
  Start-Sleep -Milliseconds 300
  if ([CreditiaWindow]::GetForegroundWindow() -ne $handle) { throw 'Cannot capture a foreground native window' }
  $rect = New-Object CreditiaWindow+Rect
  if (![CreditiaWindow]::GetWindowRect($handle,[ref]$rect)) { throw 'GetWindowRect failed' }
  $width = $rect.Right - $rect.Left; $height = $rect.Bottom - $rect.Top
  if ($width -lt 700 -or $height -lt 500) { throw "Unexpected native window size $width x $height" }
  $bitmap = New-Object System.Drawing.Bitmap($width,$height)
  $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
  try {
    $graphics.CopyFromScreen($rect.Left,$rect.Top,0,0,$bitmap.Size)
    $colors = New-Object 'System.Collections.Generic.HashSet[int]'
    for ($y=30; $y -lt $height; $y+=8) { for ($x=0; $x -lt $width; $x+=8) { $colors.Add($bitmap.GetPixel($x,$y).ToArgb()) | Out-Null } }
    if ($colors.Count -lt 64) { throw 'Native window appears blank; graphics or assets may have failed' }
    $path = Join-Path $Output $name
    $bitmap.Save($path,[System.Drawing.Imaging.ImageFormat]::Png)
    return @{path=$name;sha256=(Get-FileHash $path -Algorithm SHA256).Hash.ToLowerInvariant();bytes=(Get-Item $path).Length;width=$width;height=$height;dpi=[CreditiaWindow]::GetDpiForWindow($handle);sampled_colors=$colors.Count}
  } finally { $graphics.Dispose(); $bitmap.Dispose() }
}

foreach ($language in @('zh','en')) {
  foreach ($theme in @('light','dark')) {
    $key = "windows-$language-$theme"
    $process = $null; $temporary = Join-Path ([IO.Path]::GetTempPath()) ([Guid]::NewGuid().ToString())
    [IO.Directory]::CreateDirectory($temporary) | Out-Null
    $case = @{key=$key;status='failed';checks=@();artifacts=@()}
    $previousEvents=$env:CREDITIA_APP_EVENTS
    $previousLibrary=$env:CREDITIA_HOME
    try {
      $fixture = Get-Content (Join-Path $Fixtures "minimal-light-$language.json") -Raw | ConvertFrom-Json
      $fixture.appearance = $theme
      $fixturePath = Join-Path $temporary 'fixture.json'
      [IO.File]::WriteAllText($fixturePath,($fixture | ConvertTo-Json -Depth 100 -Compress),(New-Object Text.UTF8Encoding($false)))
      $env:CREDITIA_APP_EVENTS=Join-Path $temporary 'events.jsonl'
      $env:CREDITIA_HOME=Join-Path $temporary 'library'
      $process = Start-Process -FilePath (Join-Path $PackageRoot 'creditia-gpui.exe') -ArgumentList @('--fixture',('"'+$fixturePath+'"')) -WorkingDirectory $temporary -PassThru -RedirectStandardOutput (Join-Path $temporary 'stdout.log') -RedirectStandardError (Join-Path $temporary 'stderr.log')
      $deadline = [DateTime]::UtcNow.AddSeconds(45)
      do {
        Start-Sleep -Milliseconds 200; $process.Refresh()
        if ($process.HasExited) { throw "Native process exited $($process.ExitCode): $(Get-Content (Join-Path $temporary 'stderr.log') -Raw)" }
      } while ($process.MainWindowHandle -eq [IntPtr]::Zero -and [DateTime]::UtcNow -lt $deadline)
      if ($process.MainWindowHandle -eq [IntPtr]::Zero) { throw 'Timed out waiting for a native window' }
      $screen = [System.Windows.Forms.Screen]::PrimaryScreen.WorkingArea
      [CreditiaWindow]::SetWindowPos($process.MainWindowHandle,[IntPtr]::Zero,($screen.Left+8),($screen.Top+8),[Math]::Min(1440,$screen.Width-16),[Math]::Min(1000,$screen.Height-16),0x0040) | Out-Null
      [CreditiaWindow]::SetForegroundWindow($process.MainWindowHandle)|Out-Null
      $ready=Wait-AppState $process $env:CREDITIA_APP_EVENTS {param($state)$state.ready -and $state.active -and $state.view -eq 'wall'} 45
      $wall = Capture-Window $process "$key-wall.png"; $case.artifacts += $wall
      $case.checks += @{id='packaged-native-window-and-decoded-artwork';passed=$true;state=$ready}
      [System.Windows.Forms.SendKeys]::SendWait('^2')
      $null=Wait-AppState $process $env:CREDITIA_APP_EVENTS {param($state)$state.view -eq 'roam'}
      $roam = Capture-Window $process "$key-roam.png"; $case.artifacts += $roam
      if ($wall.sha256 -eq $roam.sha256) { throw 'Roam shortcut did not change native pixels' }
      [System.Windows.Forms.SendKeys]::SendWait('^1')
      $null=Wait-AppState $process $env:CREDITIA_APP_EVENTS {param($state)$state.view -eq 'wall'}
      [System.Windows.Forms.SendKeys]::SendWait('^,')
      $null=Wait-AppState $process $env:CREDITIA_APP_EVENTS {param($state)$state.panel -eq 'settings'}
      $settings = Capture-Window $process "$key-settings.png"; $case.artifacts += $settings
      if ($wall.sha256 -eq $settings.sha256) { throw 'Settings shortcut did not change native pixels' }
      [System.Windows.Forms.SendKeys]::SendWait('{ESC}')
      $null=Wait-AppState $process $env:CREDITIA_APP_EVENTS {param($state)$null -eq $state.panel}
      $case.checks += @{id='keyboard-roam-wall-settings';passed=$true}
      $case.status = 'passed'
    } catch { $case.error = $_.Exception.Message }
    finally {
      if ($process) { if (!$process.HasExited) { $process.CloseMainWindow() | Out-Null; if (!$process.WaitForExit(5000)) { Stop-Process -Id $process.Id -Force } } }
      foreach ($log in @('stdout','stderr')) { $file=Join-Path $temporary "$log.log"; if(Test-Path $file){Copy-Item $file (Join-Path $Output "$key-$log.log")} }
      $events=Join-Path $temporary 'events.jsonl'
      if(Test-Path $events){$name="$key-events.jsonl";Copy-Item $events (Join-Path $Output $name);$case.artifacts+=@{path=$name;sha256=(Get-FileHash (Join-Path $Output $name) -Algorithm SHA256).Hash.ToLowerInvariant();bytes=(Get-Item (Join-Path $Output $name)).Length}}
      $env:CREDITIA_APP_EVENTS=$previousEvents
      $env:CREDITIA_HOME=$previousLibrary
      Remove-Item $temporary -Recurse -Force
      $cases += $case
    }
  }
}
$complete = @($cases | Where-Object status -ne 'passed').Count -eq 0
$report = @{schema_version=1;kind='native-ui';provenance=@{candidate_sha=$env:GITHUB_SHA;environment=@{platform='windows';arch='x86_64';renderer='native-window';os=[Environment]::OSVersion.ToString()};fixture=$manifest.fixture;profile='packaged-window-four-cases'};cases=$cases;complete=$complete}
[IO.File]::WriteAllText((Join-Path $Output 'manifest.json'),($report | ConvertTo-Json -Depth 100),(New-Object Text.UTF8Encoding($false)))
if (!$complete) { throw 'One or more packaged Windows window checks failed; inspect native-window evidence' }
