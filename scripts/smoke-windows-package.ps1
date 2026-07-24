[CmdletBinding()]
param(
  [Parameter(Mandatory = $true)]
  [string] $TargetDirectory,

  [Parameter(Mandatory = $true)]
  [string] $OutputDirectory,

  [string] $SmokeParent = "target"
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

function Resolve-AbsolutePath {
  param([Parameter(Mandatory = $true)][string] $Path)

  if ([System.IO.Path]::IsPathRooted($Path)) {
    return [System.IO.Path]::GetFullPath($Path)
  }
  return [System.IO.Path]::GetFullPath((Join-Path (Get-Location).Path $Path))
}

function Get-OpenConKitUninstallEntries {
  $entries = @()
  $roots = @(
    "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall",
    "HKLM:\Software\Microsoft\Windows\CurrentVersion\Uninstall",
    "HKLM:\Software\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall"
  )

  foreach ($root in $roots) {
    if (-not (Test-Path -LiteralPath $root)) {
      continue
    }
    foreach ($key in (Get-ChildItem -LiteralPath $root)) {
      $item = Get-ItemProperty -LiteralPath $key.PSPath -ErrorAction SilentlyContinue
      if ($null -eq $item) {
        continue
      }
      $displayNameProperty = $item.PSObject.Properties["DisplayName"]
      if ($null -ne $displayNameProperty -and $displayNameProperty.Value -eq "OpenConKit") {
        $displayVersionProperty = $item.PSObject.Properties["DisplayVersion"]
        $installLocationProperty = $item.PSObject.Properties["InstallLocation"]
        $displayIconProperty = $item.PSObject.Properties["DisplayIcon"]
        $uninstallStringProperty = $item.PSObject.Properties["UninstallString"]
        $entries += [PSCustomObject]@{
          RegistryPath = $key.PSPath
          DisplayName = [string] $displayNameProperty.Value
          DisplayVersion = [string] $displayVersionProperty.Value
          InstallLocation = [string] $installLocationProperty.Value
          DisplayIcon = [string] $displayIconProperty.Value
          UninstallString = [string] $uninstallStringProperty.Value
        }
      }
    }
  }
  return @($entries)
}

function Wait-ForProcess {
  param(
    [Parameter(Mandatory = $true)][System.Diagnostics.Process] $Process,
    [Parameter(Mandatory = $true)][int] $TimeoutMilliseconds,
    [Parameter(Mandatory = $true)][string] $Label
  )

  if (-not $Process.WaitForExit($TimeoutMilliseconds)) {
    Stop-Process -Id $Process.Id -Force
    throw "$Label timed out."
  }
  if ($Process.ExitCode -ne 0) {
    throw "$Label exited with code $($Process.ExitCode)."
  }
}

function Get-Sha256Hex {
  param([Parameter(Mandatory = $true)][string] $Path)

  $stream = [System.IO.File]::OpenRead($Path)
  try {
    $algorithm = [System.Security.Cryptography.SHA256]::Create()
    try {
      $digest = $algorithm.ComputeHash($stream)
    }
    finally {
      $algorithm.Dispose()
    }
  }
  finally {
    $stream.Dispose()
  }
  return (-join ($digest | ForEach-Object { $_.ToString("X2") }))
}

function Invoke-AppLaunchSmoke {
  param(
    [Parameter(Mandatory = $true)][string] $Executable,
    [Parameter(Mandatory = $true)][string] $WorkingDirectory,
    [Parameter(Mandatory = $true)][string] $ProfileRoot,
    [Parameter(Mandatory = $true)][string] $Label
  )

  New-Item -ItemType Directory -Path $ProfileRoot | Out-Null
  $startInfo = [System.Diagnostics.ProcessStartInfo]::new()
  $startInfo.FileName = $Executable
  $startInfo.WorkingDirectory = $WorkingDirectory
  $startInfo.UseShellExecute = $false
  $startInfo.Environment["USERPROFILE"] = $ProfileRoot
  [void] $startInfo.Environment.Remove("OPENCONKIT_HOME")

  $process = [System.Diagnostics.Process]::Start($startInfo)
  if ($null -eq $process) {
    throw "$Label did not start."
  }

  try {
    Start-Sleep -Seconds 12
    $process.Refresh()
    if ($process.HasExited) {
      throw "$Label exited early with code $($process.ExitCode)."
    }

    $appHome = Join-Path $ProfileRoot ".openconkit"
    $webviewData = Join-Path $appHome "cache\webview"
    if (-not (Test-Path -LiteralPath $webviewData -PathType Container)) {
      throw "$Label did not create its webview data directory inside app home."
    }

    $adjacentProfiles = @(
      Get-ChildItem -LiteralPath $WorkingDirectory -Directory -Filter "*.WebView2" -ErrorAction SilentlyContinue
    )
    if ($adjacentProfiles.Count -ne 0) {
      throw "$Label created executable-adjacent WebView2 data."
    }
  }
  finally {
    if (-not $process.HasExited) {
      [void] $process.CloseMainWindow()
      if (-not $process.WaitForExit(5000)) {
        Stop-Process -Id $process.Id -Force
        $process.WaitForExit()
      }
    }
    $process.Dispose()
  }
}

if ($env:OS -ne "Windows_NT") {
  throw "Windows package smoke tests must run on Windows."
}

$targetRoot = Resolve-AbsolutePath $TargetDirectory
$outputRoot = Resolve-AbsolutePath $OutputDirectory
$smokeParentRoot = Resolve-AbsolutePath $SmokeParent
$version = (Get-Content -LiteralPath "VERSION" -Raw).Trim()
$releaseRoot = Join-Path $targetRoot "x86_64-pc-windows-msvc\release"
$releaseExecutable = Join-Path $releaseRoot "openconkit-desktop.exe"
$installerRoot = Join-Path $releaseRoot "bundle\nsis"
$portableArchive = Join-Path $outputRoot "OpenConKit_${version}_windows_x64_portable.zip"

foreach ($requiredFile in @($releaseExecutable, $portableArchive)) {
  if (-not (Test-Path -LiteralPath $requiredFile -PathType Leaf)) {
    throw "Required package input is missing: $requiredFile"
  }
}

$installers = @(Get-ChildItem -LiteralPath $installerRoot -File -Filter "*-setup.exe")
if ($installers.Count -ne 1) {
  throw "Expected one NSIS installer under $installerRoot; found $($installers.Count)."
}
if (@(Get-OpenConKitUninstallEntries).Count -ne 0) {
  throw "Refusing package smoke because OpenConKit is already installed."
}

$smokeRoot = Join-Path $smokeParentRoot "openconkit-package-smoke-$([Guid]::NewGuid().ToString('N'))"
$portableExtractRoot = Join-Path $smokeRoot "portable"
$portableProfile = Join-Path $smokeRoot "portable-profile"
$installedProfile = Join-Path $smokeRoot "installed-profile"
$installDirectory = Join-Path $smokeRoot "installed-program"
$uninstaller = $null
$testFailure = $null

try {
  New-Item -ItemType Directory -Path $portableExtractRoot | Out-Null
  Expand-Archive -LiteralPath $portableArchive -DestinationPath $portableExtractRoot
  $portableRoot = Join-Path $portableExtractRoot "OpenConKit_${version}_windows_x64_portable"
  $portableExecutable = Join-Path $portableRoot "OpenConKit.exe"
  foreach (
    $requiredFile in @(
      $portableExecutable,
      (Join-Path $portableRoot "codex-app-server.exe"),
      (Join-Path $portableRoot "OPENCONKIT_PORTABLE"),
      (Join-Path $portableRoot "PORTABLE_README.txt"),
      (Join-Path $portableRoot "licenses\THIRD_PARTY_NOTICES.md")
    )
  ) {
    if (-not (Test-Path -LiteralPath $requiredFile -PathType Leaf)) {
      throw "Portable package file is missing: $requiredFile"
    }
  }
  if (
    (Get-Sha256Hex -Path $portableExecutable) -ne
    (Get-Sha256Hex -Path $releaseExecutable)
  ) {
    throw "Portable executable does not match the release executable."
  }

  Invoke-AppLaunchSmoke `
    -Executable $portableExecutable `
    -WorkingDirectory $portableRoot `
    -ProfileRoot $portableProfile `
    -Label "Portable OpenConKit"

  if ($installDirectory.Contains(" ")) {
    throw "NSIS smoke install directory must not contain spaces: $installDirectory"
  }
  $installProcess = Start-Process `
    -FilePath $installers[0].FullName `
    -ArgumentList @("/S", "/D=$installDirectory") `
    -WindowStyle Hidden `
    -PassThru
  Wait-ForProcess -Process $installProcess -TimeoutMilliseconds 240000 -Label "OpenConKit installer"
  $installProcess.Dispose()

  $entries = @()
  for ($attempt = 0; $attempt -lt 30 -and $entries.Count -eq 0; $attempt += 1) {
    Start-Sleep -Milliseconds 500
    $entries = @(Get-OpenConKitUninstallEntries)
  }
  if ($entries.Count -ne 1) {
    throw "Expected one OpenConKit uninstall entry; found $($entries.Count)."
  }

  $entry = $entries[0]
  $registeredInstallDirectory = $entry.InstallLocation.Trim().Trim('"')
  if (
    -not [System.IO.Path]::GetFullPath($registeredInstallDirectory).Equals(
      [System.IO.Path]::GetFullPath($installDirectory),
      [StringComparison]::OrdinalIgnoreCase
    )
  ) {
    throw "NSIS installed to an unexpected directory: $registeredInstallDirectory"
  }

  $installedExecutable = $entry.DisplayIcon.Trim().Trim('"')
  $uninstaller = $entry.UninstallString.Trim().Trim('"')
  foreach ($requiredFile in @($installedExecutable, $uninstaller)) {
    if (-not (Test-Path -LiteralPath $requiredFile -PathType Leaf)) {
      throw "Installed package file is missing: $requiredFile"
    }
  }
  if ((Get-Item -LiteralPath $installedExecutable).VersionInfo.ProductVersion -ne $version) {
    throw "Installed executable version does not match VERSION."
  }

  Invoke-AppLaunchSmoke `
    -Executable $installedExecutable `
    -WorkingDirectory $registeredInstallDirectory `
    -ProfileRoot $installedProfile `
    -Label "Installed OpenConKit"

  $uninstallProcess = Start-Process `
    -FilePath $uninstaller `
    -ArgumentList "/S" `
    -WindowStyle Hidden `
    -PassThru
  Wait-ForProcess -Process $uninstallProcess -TimeoutMilliseconds 180000 -Label "OpenConKit uninstaller"
  $uninstallProcess.Dispose()
  $uninstaller = $null

  for ($attempt = 0; $attempt -lt 30; $attempt += 1) {
    if (
      @(Get-OpenConKitUninstallEntries).Count -eq 0 -and
      -not (Test-Path -LiteralPath $registeredInstallDirectory)
    ) {
      break
    }
    Start-Sleep -Milliseconds 500
  }
  if (@(Get-OpenConKitUninstallEntries).Count -ne 0) {
    throw "OpenConKit uninstall registration remains after uninstall."
  }
  if (Test-Path -LiteralPath $registeredInstallDirectory) {
    throw "OpenConKit install directory remains after uninstall."
  }

  Write-Output "Windows installer and portable package smoke tests passed."
}
catch {
  $testFailure = $_
}
finally {
  if ($null -ne $uninstaller -and (Test-Path -LiteralPath $uninstaller -PathType Leaf)) {
    $cleanup = Start-Process -FilePath $uninstaller -ArgumentList "/S" -WindowStyle Hidden -PassThru
    [void] $cleanup.WaitForExit(180000)
    $cleanup.Dispose()
  }

  $resolvedSmokeRoot = [System.IO.Path]::GetFullPath($smokeRoot)
  $resolvedParent = $smokeParentRoot.TrimEnd("\") + "\"
  if (-not $resolvedSmokeRoot.StartsWith($resolvedParent, [StringComparison]::OrdinalIgnoreCase)) {
    throw "Refusing to clean an unconfined smoke-test directory: $resolvedSmokeRoot"
  }
  if (Test-Path -LiteralPath $resolvedSmokeRoot) {
    Remove-Item -LiteralPath $resolvedSmokeRoot -Recurse -Force
  }
}

if ($null -ne $testFailure) {
  throw $testFailure
}
