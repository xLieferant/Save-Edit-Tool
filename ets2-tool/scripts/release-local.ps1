param(
  [Parameter(Mandatory = $true)]
  [string]$Version,

  [string]$Tag = "",

  [string]$Notes = ""
)

$ErrorActionPreference = "Stop"

$projectRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$tauriConfigPath = Join-Path $projectRoot "src-tauri/tauri.conf.json"
$cargoTomlPath = Join-Path $projectRoot "src-tauri/Cargo.toml"
$bundleDir = Join-Path $projectRoot "src-tauri/target/release/bundle/nsis"
$releaseTag = if ([string]::IsNullOrWhiteSpace($Tag)) { $Version } else { $Tag }

if ($releaseTag -match '^[vV]') {
  throw "Release tags must not start with v. Use $Version instead of $releaseTag."
}

if (-not (Test-Path -LiteralPath $tauriConfigPath)) {
  throw "Tauri config not found: $tauriConfigPath"
}

if (-not (Test-Path -LiteralPath $cargoTomlPath)) {
  throw "Cargo.toml not found: $cargoTomlPath"
}

$tauriConfig = Get-Content -LiteralPath $tauriConfigPath -Raw | ConvertFrom-Json
$tauriVersion = [string]$tauriConfig.version

if ($tauriVersion -ne $Version) {
  throw "Version mismatch: src-tauri/tauri.conf.json has $tauriVersion, expected $Version"
}

$cargoToml = Get-Content -LiteralPath $cargoTomlPath -Raw
$cargoVersionMatch = [regex]::Match($cargoToml, '(?m)^version\s*=\s*"([^"]+)"')
if (-not $cargoVersionMatch.Success) {
  throw "Could not read package version from src-tauri/Cargo.toml"
}

$cargoVersion = $cargoVersionMatch.Groups[1].Value
if ($cargoVersion -ne $Version) {
  throw "Version mismatch: src-tauri/Cargo.toml has $cargoVersion, expected $Version"
}

Push-Location (Join-Path $projectRoot "src-tauri")
try {
  cargo tauri build
} finally {
  Pop-Location
}

& (Join-Path $PSScriptRoot "generate-latest-json.ps1") `
  -Version $Version `
  -Tag $releaseTag `
  -Notes $Notes

$assetName = "Save-Edit.Tool.xLieferant_$($Version)_x64-setup.exe"
$installerPath = Join-Path $bundleDir $assetName
if (-not (Test-Path -LiteralPath $installerPath)) {
  throw "Build completed, but normalized installer was not found: $installerPath"
}

$signaturePath = "$installerPath.sig"
$rootLatestJson = Join-Path $projectRoot "latest.json"
$bundleLatestJson = Join-Path $bundleDir "latest.json"

Write-Host ""
Write-Host "Upload these files to GitHub release ${releaseTag}:"
Write-Host "  $installerPath"
Write-Host "  $signaturePath"
Write-Host "  $rootLatestJson"
Write-Host ""
Write-Host "A copy of latest.json was also written to:"
Write-Host "  $bundleLatestJson"
