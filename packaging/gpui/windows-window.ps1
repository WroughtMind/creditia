param(
  [Parameter(Mandatory=$true)][string]$PackageRoot,
  [string]$Fixtures,
  [string]$ProjectName,
  [switch]$CheckPlatform,
  [string]$Candidate = $env:GITHUB_SHA,
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
  [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr window, out uint processId);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] static extern int GetClassName(IntPtr window, System.Text.StringBuilder name, int length);
  public static bool FileDialogActive(uint pid) {
    IntPtr window = GetForegroundWindow();
    uint process;
    GetWindowThreadProcessId(window, out process);
    var name = new System.Text.StringBuilder(128);
    GetClassName(window, name, name.Capacity);
    return process == pid && name.ToString() == "#32770";
  }
  [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr handle, int command);
  [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr handle, IntPtr after, int x, int y, int width, int height, uint flags);
  [DllImport("user32.dll")] public static extern uint GetDpiForWindow(IntPtr handle);
  [DllImport("user32.dll")] public static extern bool SetProcessDpiAwarenessContext(IntPtr context);
}
'@
[CreditiaWindow]::SetProcessDpiAwarenessContext([IntPtr](-4)) | Out-Null
$PackageRoot = [IO.Path]::GetFullPath($PackageRoot)
if ($Fixtures) { $Fixtures = [IO.Path]::GetFullPath($Fixtures) }
$Output = [IO.Path]::GetFullPath($Output)
[IO.Directory]::CreateDirectory($Output) | Out-Null
$manifest = if ($Fixtures) { Get-Content (Join-Path $Fixtures 'fixture-manifest.json') -Raw | ConvertFrom-Json }
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

$languages = if ($ProjectName) { 'saved' } else { @('zh','en') }
$themes = if ($ProjectName) { 'saved' } else { @('light','dark') }
foreach ($language in $languages) {
  foreach ($theme in $themes) {
    $key = "windows-$language-$theme"
    $process = $null; $temporary = Join-Path ([IO.Path]::GetTempPath()) ([Guid]::NewGuid().ToString())
    [IO.Directory]::CreateDirectory($temporary) | Out-Null
    $case = @{key=$key;status='failed';checks=@();artifacts=@()}
    $previousEvents=$env:CREDITIA_APP_EVENTS
    $previousLibrary=$env:CREDITIA_HOME
    try {
      $env:CREDITIA_APP_EVENTS=Join-Path $temporary 'events.jsonl'
      $start = @{FilePath=(Join-Path $PackageRoot 'creditia-gpui.exe');WorkingDirectory=$temporary;PassThru=$true;
        RedirectStandardOutput=(Join-Path $Output "$key-stdout.log");RedirectStandardError=(Join-Path $Output "$key-stderr.log")}
      if (!$ProjectName) {
        $fixture = Get-Content (Join-Path $Fixtures "minimal-light-$language.json") -Raw | ConvertFrom-Json
        $fixture.appearance = $theme
        $fixturePath = Join-Path $temporary 'fixture.json'
        [IO.File]::WriteAllText($fixturePath,($fixture | ConvertTo-Json -Depth 100 -Compress),(New-Object Text.UTF8Encoding($false)))
        $env:CREDITIA_HOME=Join-Path $temporary 'library'
        $start.ArgumentList = @('--fixture',('"'+$fixturePath+'"'))
      }
      $process = Start-Process @start
      $deadline = [DateTime]::UtcNow.AddSeconds(45)
      do {
        Start-Sleep -Milliseconds 200; $process.Refresh()
        if ($process.HasExited) { throw "Native process exited $($process.ExitCode): $(Get-Content (Join-Path $Output "$key-stderr.log") -Raw)" }
      } while ($process.MainWindowHandle -eq [IntPtr]::Zero -and [DateTime]::UtcNow -lt $deadline)
      if ($process.MainWindowHandle -eq [IntPtr]::Zero) { throw 'Timed out waiting for a native window' }
      $screen = [System.Windows.Forms.Screen]::PrimaryScreen.WorkingArea
      [CreditiaWindow]::SetWindowPos($process.MainWindowHandle,[IntPtr]::Zero,($screen.Left+8),($screen.Top+8),[Math]::Min(1440,$screen.Width-16),[Math]::Min(1000,$screen.Height-16),0x0040) | Out-Null
      [CreditiaWindow]::SetForegroundWindow($process.MainWindowHandle)|Out-Null
      $ready=Wait-AppState $process $env:CREDITIA_APP_EVENTS {param($state)
        $state.ready -and $state.active -and $state.view -eq 'wall' -and
          (!$ProjectName -or ($state.project_name -eq $ProjectName -and $state.viewport_ready -and
            @($state.visible_tiles | Where-Object image_ready).Count -gt 0))
      } 90
      $wall = Capture-Window $process "$key-wall.png"; $case.artifacts += $wall
      if ($CheckPlatform) {
        Set-Clipboard -Value 'creditia-install-check'
        [System.Windows.Forms.SendKeys]::SendWait('^+c')
        $deadline=[DateTime]::UtcNow.AddSeconds(10)
        do {
          try { $archive=Get-Clipboard -Raw | ConvertFrom-Json } catch { $archive=$null }
          if ($archive.project.name -eq $ProjectName) { break }
          Start-Sleep -Milliseconds 100
        } while ([DateTime]::UtcNow -lt $deadline)
        if ($archive.project.name -ne $ProjectName) { throw 'Copy Archive did not reach the Windows clipboard' }
        [System.Windows.Forms.SendKeys]::SendWait('^o')
        $deadline=[DateTime]::UtcNow.AddSeconds(15)
        while (![CreditiaWindow]::FileDialogActive($process.Id) -and [DateTime]::UtcNow -lt $deadline) { Start-Sleep -Milliseconds 100 }
        if (![CreditiaWindow]::FileDialogActive($process.Id)) { throw 'Native folder picker did not open' }
        [System.Windows.Forms.SendKeys]::SendWait('{ESC}')
        $deadline=[DateTime]::UtcNow.AddSeconds(10)
        while ([CreditiaWindow]::FileDialogActive($process.Id) -and [DateTime]::UtcNow -lt $deadline) { Start-Sleep -Milliseconds 100 }
        if ([CreditiaWindow]::FileDialogActive($process.Id)) { throw 'Native folder picker did not close' }
        $case.checks += @{id='system-clipboard-and-folder-dialog';passed=$true;project=$ProjectName}
      }
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
      if ($ProjectName) {
        [System.Windows.Forms.SendKeys]::SendWait('^q')
        if (!$process.WaitForExit(15000) -or $process.ExitCode -ne 0) { throw 'Installed window did not quit normally' }
      }
      $case.status = 'passed'
    } catch { $case.error = $_.Exception.Message }
    finally {
      if ($process) { if (!$process.HasExited) { $process.CloseMainWindow() | Out-Null; if (!$process.WaitForExit(5000)) { Stop-Process -Id $process.Id -Force } } }
      if ($process) { $process.WaitForExit(); $process.Dispose() }
      $events=Join-Path $temporary 'events.jsonl'
      if(Test-Path $events){$name="$key-events.jsonl";Copy-Item $events (Join-Path $Output $name);$case.artifacts+=@{path=$name;sha256=(Get-FileHash (Join-Path $Output $name) -Algorithm SHA256).Hash.ToLowerInvariant();bytes=(Get-Item (Join-Path $Output $name)).Length}}
      $env:CREDITIA_APP_EVENTS=$previousEvents
      $env:CREDITIA_HOME=$previousLibrary
      # Windows can retain an exited child's working directory briefly.
      $deadline = [DateTime]::UtcNow.AddSeconds(20)
      while (Test-Path -LiteralPath $temporary) {
        try { Remove-Item -LiteralPath $temporary -Recurse -Force -ErrorAction Stop }
        catch [System.IO.IOException] {
          if ([DateTime]::UtcNow -ge $deadline) { throw }
          Start-Sleep -Milliseconds 200
        }
      }
      $cases += $case
    }
  }
}
$complete = @($cases | Where-Object status -ne 'passed').Count -eq 0
$report = @{schema_version=1;kind='native-ui';provenance=@{candidate_sha=$Candidate;environment=@{platform='windows';arch='x86_64';renderer='native-window';os=[Environment]::OSVersion.ToString()};fixture=$manifest.fixture;profile='packaged-window-four-cases'};cases=$cases;complete=$complete}
[IO.File]::WriteAllText((Join-Path $Output 'manifest.json'),($report | ConvertTo-Json -Depth 100),(New-Object Text.UTF8Encoding($false)))
if (!$complete) { throw 'One or more packaged Windows window checks failed; inspect native-window evidence' }
