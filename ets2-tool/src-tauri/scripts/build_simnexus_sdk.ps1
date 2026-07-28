param(
  [switch]$Install,
  [ValidateSet("ets2", "ats")]
  [string]$Game = "ets2",
  [string]$PluginDirectory
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Assert-LastExitCode([string]$Operation) {
  if ($LASTEXITCODE -ne 0) {
    throw "$Operation failed with exit code $LASTEXITCODE"
  }
}

function Get-SteamLibraryRoots {
  $roots = [System.Collections.Generic.List[string]]::new()
  $steamPath = (Get-ItemProperty -LiteralPath "HKCU:\\Software\\Valve\\Steam" -ErrorAction Stop).SteamPath
  $roots.Add($steamPath)
  $vdfPath = Join-Path $steamPath "steamapps\\libraryfolders.vdf"
  if (Test-Path -LiteralPath $vdfPath) {
    foreach ($match in [regex]::Matches((Get-Content -Raw -LiteralPath $vdfPath), '"path"\s*"([^"]+)"')) {
      $roots.Add($match.Groups[1].Value.Replace("\\", "\"))
    }
  }
  return $roots | Sort-Object -Unique
}

function Resolve-PluginDirectory([string]$GameId) {
  $appId = if ($GameId -eq "ets2") { "227300" } else { "270880" }
  $gameDir = if ($GameId -eq "ets2") { "Euro Truck Simulator 2" } else { "American Truck Simulator" }
  $exe = if ($GameId -eq "ets2") { "eurotrucks2.exe" } else { "amtrucks.exe" }
  foreach ($library in Get-SteamLibraryRoots) {
    $manifest = Join-Path $library "steamapps\\appmanifest_$appId.acf"
    $binaryDir = Join-Path $library "steamapps\\common\\$gameDir\\bin\\win_x64"
    if ((Test-Path -LiteralPath $manifest) -and (Test-Path -LiteralPath (Join-Path $binaryDir $exe))) {
      return Join-Path $binaryDir "plugins"
    }
  }
  throw "Could not locate the $GameId win_x64 plugin directory"
}

function Assert-GameStopped([string]$GameId) {
  $processName = if ($GameId -eq "ets2") { "eurotrucks2" } else { "amtrucks" }
  if (Get-Process -Name $processName -ErrorAction SilentlyContinue) {
    throw "Refusing to replace the telemetry DLL while $processName.exe is running"
  }
}

function Get-Hash([string]$Path) {
  return (Get-FileHash -Algorithm SHA256 -LiteralPath $Path).Hash
}

function Resolve-CMake {
  $command = Get-Command cmake -ErrorAction SilentlyContinue
  if ($command) {
    return $command.Source
  }
  $visualStudioCMake = "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\Common7\IDE\CommonExtensions\Microsoft\CMake\CMake\bin\cmake.exe"
  if (Test-Path -LiteralPath $visualStudioCMake) {
    return $visualStudioCMake
  }
  throw "CMake was not found in PATH or Visual Studio 2022 Build Tools"
}

$root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$nativeDir = (Resolve-Path (Join-Path $root "..\\native\\simnexus_sdk")).Path
$buildDir = Join-Path $nativeDir "build-v3-x64"
$outDll = Join-Path $buildDir "Release\\simnexus_sdk.dll"
$selfTest = Join-Path $buildDir "Release\\simnexus_bridge_selftest.exe"
$resourceDir = Join-Path $root "resources\\plugins"
$resourceDll = Join-Path $resourceDir "simnexus_sdk.dll"
$cmake = Resolve-CMake

Write-Host "[simnexus] native: $nativeDir"
Write-Host "[simnexus] cmake: $cmake"
New-Item -ItemType Directory -Force -Path $resourceDir | Out-Null

Write-Host "[simnexus] configure (x64 Release)"
& $cmake -S $nativeDir -B $buildDir -A x64
Assert-LastExitCode "CMake configure"

Write-Host "[simnexus] build (Release)"
& $cmake --build $buildDir --config Release
Assert-LastExitCode "CMake build"

if (!(Test-Path -LiteralPath $outDll)) {
  throw "Release DLL not found: $outDll"
}
if (!(Test-Path -LiteralPath $selfTest)) {
  throw "Self-test executable not found: $selfTest"
}

Copy-Item -Force -LiteralPath $outDll -Destination $resourceDll
$buildHash = Get-Hash $outDll
$resourceHash = Get-Hash $resourceDll
if ($buildHash -ne $resourceHash) {
  throw "Resource DLL hash mismatch: build=$buildHash resource=$resourceHash"
}
$resourceMeta = Get-Item -LiteralPath $resourceDll
Write-Host ("[simnexus] resource verified: {0} ({1} bytes, sha256={2})" -f $resourceDll, $resourceMeta.Length, $resourceHash)

if ($Install) {
  Assert-GameStopped $Game
  $resolvedPluginDirectory = if ($PluginDirectory) {
    [System.IO.Path]::GetFullPath($PluginDirectory)
  } else {
    Resolve-PluginDirectory $Game
  }
  New-Item -ItemType Directory -Force -Path $resolvedPluginDirectory | Out-Null
  $targetDll = Join-Path $resolvedPluginDirectory "simnexus_sdk.dll"
  if (Test-Path -LiteralPath $targetDll) {
    $backupDll = "$targetDll.bak"
    Copy-Item -Force -LiteralPath $targetDll -Destination $backupDll
    Write-Host "[simnexus] backup: $backupDll"
  }
  Copy-Item -Force -LiteralPath $resourceDll -Destination $targetDll
  $targetHash = Get-Hash $targetDll
  if ($resourceHash -ne $targetHash) {
    throw "Installed DLL hash mismatch: resource=$resourceHash target=$targetHash"
  }
  Write-Host "[simnexus] installed and verified: $targetDll (sha256=$targetHash)"
}

Write-Host "[simnexus] self-test executable: $selfTest"
