# Windows installer test: install silently from a path with Arabic letters
# and spaces, check what the installer created, then uninstall.
#
#   pwsh scripts/installer-test.ps1 <path-to-setup.exe>

param([Parameter(Mandatory)] [string] $Setup)
$ErrorActionPreference = "Stop"
$failed = $false
function Check([string] $name, [bool] $ok, [string] $detail = "") {
  $mark = if ($ok) { "PASS" } else { "FAIL" }
  Write-Host "$mark $name $(if ($detail) { "- $detail" })"
  if (-not $ok) { $script:failed = $true }
}
function Run([string] $exe, [string[]] $arguments, [int] $seconds = 180) {
  $p = Start-Process -FilePath $exe -ArgumentList $arguments -PassThru
  if (-not $p.WaitForExit($seconds * 1000)) {
    $p.Kill()
    throw "$exe did not finish within $seconds s (a message box may be waiting)"
  }
  return $p.ExitCode
}

$item = Get-Item $Setup
Check "installer file looks complete" ($item.Length -gt 1MB) "$([math]::Round($item.Length / 1MB, 2)) MB"

# 1. Install from an awkward path.
$dir = Join-Path $env:RUNNER_TEMP "مجلد تجربة أوان"
New-Item -ItemType Directory -Force -Path $dir | Out-Null
$copy = Join-Path $dir $item.Name
Copy-Item $item.FullName $copy
$code = Run $copy @("/S")
Check "silent install exits with 0" ($code -eq 0) "exit code $code"

# 2. What the installer created.
$installDir = Join-Path $env:LOCALAPPDATA "Awan"
$exe = Get-ChildItem $installDir -Filter *.exe -ErrorAction SilentlyContinue |
  Where-Object { $_.Name -notlike "uninstall*" } | Select-Object -First 1
Check "app installed under %LOCALAPPDATA%\Awan" ($null -ne $exe) "$($exe.FullName)"
Check "uninstaller present" (Test-Path (Join-Path $installDir "uninstall.exe"))

$startMenu = Join-Path ([Environment]::GetFolderPath("Programs")) "Awan.lnk"
Check "Start menu shortcut" (Test-Path $startMenu) $startMenu
$desktop = Join-Path ([Environment]::GetFolderPath("Desktop")) "Awan.lnk"
Check "desktop shortcut" (Test-Path $desktop) $desktop

$entry = Get-ChildItem "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall" |
  ForEach-Object { Get-ItemProperty $_.PSPath } |
  Where-Object { $_.DisplayName -like "Awan*" } | Select-Object -First 1
Check "listed in Installed apps" ($null -ne $entry) "$($entry.DisplayName) $($entry.DisplayVersion)"
if ($entry) {
  Check "publisher shown" ($entry.Publisher -eq "cylinder303") "$($entry.Publisher)"
  Check "app icon registered" ([bool] $entry.DisplayIcon) "$($entry.DisplayIcon)"
}

# 3. Uninstall.
$code = Run (Join-Path $installDir "uninstall.exe") @("/S")
Check "silent uninstall exits with 0" ($code -eq 0) "exit code $code"
# The uninstaller relaunches itself from a temp copy; wait for it to finish.
for ($i = 0; $i -lt 30; $i++) {
  $left = Get-ChildItem $installDir -Filter *.exe -ErrorAction SilentlyContinue | Where-Object { $_.Name -notlike "uninstall*" }
  if ($null -eq $left) { break }
  Start-Sleep -Seconds 1
}
Check "app removed" ($null -eq $left)
Check "Start menu shortcut removed" (-not (Test-Path $startMenu))

if ($failed) { exit 1 }
