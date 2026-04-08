Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$nativeDir = Join-Path $root "..\\native\\simnexus_sdk"
$buildDir = Join-Path $nativeDir "build"
$outDll = Join-Path $buildDir "Release\\simnexus_sdk.dll"
$resourceDir = Join-Path $root "resources\\plugins"
$resourceDll = Join-Path $resourceDir "simnexus_sdk.dll"

Write-Host "[simnexus] root: $root"
Write-Host "[simnexus] native: $nativeDir"

if (!(Test-Path $resourceDir)) {
  New-Item -ItemType Directory -Force -Path $resourceDir | Out-Null
}

Write-Host "[simnexus] configure (x64)"
cmake -S $nativeDir -B $buildDir -A x64

Write-Host "[simnexus] build (Release)"
cmake --build $buildDir --config Release

if (!(Test-Path $outDll)) {
  throw "Release DLL not found: $outDll"
}

Copy-Item -Force -Path $outDll -Destination $resourceDll

$meta = Get-Item $resourceDll
Write-Host ("[simnexus] copied: {0} ({1} bytes, mtime={2})" -f $resourceDll, $meta.Length, $meta.LastWriteTimeUtc.ToString("o"))

