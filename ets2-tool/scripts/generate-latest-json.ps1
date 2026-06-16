param(
  [Parameter(Mandatory = $true)]
  [string]$Version,

  [Parameter(Mandatory = $true)]
  [string]$Tag,

  [string]$Repo = "xLieferant/Save-Edit-Tool",

  [string]$BundleDir = "src-tauri/target/release/bundle/nsis",

  [string]$Notes = ""
)

$ErrorActionPreference = "Stop"

$projectRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$resolvedBundleDir = Join-Path $projectRoot $BundleDir

if (-not (Test-Path -LiteralPath $resolvedBundleDir)) {
  throw "Bundle directory not found: $resolvedBundleDir"
}

$installerPattern = "*_$($Version)_x64-setup.exe"
$installers = @(Get-ChildItem -LiteralPath $resolvedBundleDir -File -Filter $installerPattern)

if ($installers.Count -eq 0) {
  throw "No NSIS installer found for version $Version in $resolvedBundleDir using pattern $installerPattern"
}

if ($installers.Count -gt 1) {
  $names = ($installers | ForEach-Object { $_.Name }) -join ", "
  throw "Multiple NSIS installers found for version ${Version}: $names"
}

$installer = $installers[0]
$signaturePath = "$($installer.FullName).sig"

if (-not (Test-Path -LiteralPath $signaturePath)) {
  throw "Signature file not found for installer: $signaturePath"
}

$signature = (Get-Content -LiteralPath $signaturePath -Raw).Trim()
if ([string]::IsNullOrWhiteSpace($signature)) {
  throw "Signature file is empty: $signaturePath"
}

$assetName = $installer.Name
$encodedAssetName = [System.Uri]::EscapeDataString($assetName)
$downloadUrl = "https://github.com/$Repo/releases/download/$Tag/$encodedAssetName"
$releaseNotes = if ([string]::IsNullOrWhiteSpace($Notes)) { "Release $Version" } else { $Notes }

$manifest = [ordered]@{
  version = $Version
  notes = $releaseNotes
  pub_date = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
  platforms = [ordered]@{
    "windows-x86_64" = [ordered]@{
      signature = $signature
      url = $downloadUrl
    }
  }
}

$json = $manifest | ConvertTo-Json -Depth 8
$utf8NoBom = New-Object System.Text.UTF8Encoding($false)
$rootLatestJson = Join-Path $projectRoot "latest.json"
$bundleLatestJson = Join-Path $resolvedBundleDir "latest.json"

[System.IO.File]::WriteAllText($rootLatestJson, $json + [Environment]::NewLine, $utf8NoBom)
[System.IO.File]::WriteAllText($bundleLatestJson, $json + [Environment]::NewLine, $utf8NoBom)

Write-Host "Generated latest.json:"
Write-Host "  $rootLatestJson"
Write-Host "  $bundleLatestJson"
Write-Host ""
Write-Host "Release assets:"
Write-Host "  $($installer.FullName)"
Write-Host "  $signaturePath"
Write-Host "  $rootLatestJson"
