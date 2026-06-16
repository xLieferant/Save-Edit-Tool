param(
  [Parameter(Mandatory = $true)]
  [string]$Version,

  [string]$Tag = "",

  [string]$Repo = "xLieferant/Save-Edit-Tool",

  [string]$BundleDir = "src-tauri/target/release/bundle/nsis",

  [string]$Notes = ""
)

$ErrorActionPreference = "Stop"

$projectRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$resolvedBundleDir = Join-Path $projectRoot $BundleDir
$releaseTag = if ([string]::IsNullOrWhiteSpace($Tag)) { $Version } else { $Tag }

if ($releaseTag -match '^[vV]') {
  throw "Release tags must not start with v. Use $Version instead of $releaseTag."
}

if (-not (Test-Path -LiteralPath $resolvedBundleDir)) {
  throw "Bundle directory not found: $resolvedBundleDir"
}

$installerPattern = "*_$($Version)_x64-setup.exe"
$installers = @(Get-ChildItem -LiteralPath $resolvedBundleDir -File -Filter $installerPattern | Sort-Object LastWriteTimeUtc -Descending)

if ($installers.Count -eq 0) {
  throw "No NSIS installer found for version $Version in $resolvedBundleDir using pattern $installerPattern"
}

$installer = $null
$signaturePath = $null
foreach ($candidate in $installers) {
  $candidateSignaturePath = "$($candidate.FullName).sig"
  if (Test-Path -LiteralPath $candidateSignaturePath) {
    $installer = $candidate
    $signaturePath = $candidateSignaturePath
    break
  }
}

if (-not $installer -or -not $signaturePath) {
  $names = ($installers | ForEach-Object { $_.Name }) -join ", "
  throw "No matching .sig file found for NSIS installer candidates: $names"
}

$assetName = "Save-Edit.Tool.xLieferant_$($Version)_x64-setup.exe"
$assetPath = Join-Path $resolvedBundleDir $assetName
$assetSignaturePath = "$assetPath.sig"

if ($installer.FullName -ne $assetPath) {
  Copy-Item -LiteralPath $installer.FullName -Destination $assetPath -Force
}

if ($signaturePath -ne $assetSignaturePath) {
  Copy-Item -LiteralPath $signaturePath -Destination $assetSignaturePath -Force
}

$signature = (Get-Content -LiteralPath $assetSignaturePath -Raw).Trim()
if ([string]::IsNullOrWhiteSpace($signature)) {
  throw "Signature file is empty: $assetSignaturePath"
}

$downloadUrl = "https://github.com/$Repo/releases/download/$releaseTag/$assetName"
$pubDate = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ss.fffZ")

function ConvertTo-JsonString {
  param([AllowNull()][string]$Value)
  return ($Value | ConvertTo-Json -Compress)
}

$jsonLines = @(
  "{",
  "  `"version`": $(ConvertTo-JsonString $Version),",
  "  `"notes`": $(ConvertTo-JsonString $Notes),",
  "  `"pub_date`": $(ConvertTo-JsonString $pubDate),",
  "  `"platforms`": {",
  "    `"windows-x86_64`": {",
  "      `"signature`": $(ConvertTo-JsonString $signature),",
  "      `"url`": $(ConvertTo-JsonString $downloadUrl)",
  "    },",
  "    `"windows-x86_64-nsis`": {",
  "      `"signature`": $(ConvertTo-JsonString $signature),",
  "      `"url`": $(ConvertTo-JsonString $downloadUrl)",
  "    }",
  "  }",
  "}"
)

$json = $jsonLines -join [Environment]::NewLine
$utf8NoBom = New-Object System.Text.UTF8Encoding($false)
$rootLatestJson = Join-Path $projectRoot "latest.json"
$bundleLatestJson = Join-Path $resolvedBundleDir "latest.json"

try {
  [System.IO.File]::WriteAllText($rootLatestJson, $json + [Environment]::NewLine, $utf8NoBom)
  [System.IO.File]::WriteAllText($bundleLatestJson, $json + [Environment]::NewLine, $utf8NoBom)
} catch {
  throw "Failed to write latest.json: $($_.Exception.Message)"
}

Write-Host "Generated latest.json:"
Write-Host "  $rootLatestJson"
Write-Host "  $bundleLatestJson"
Write-Host ""
Write-Host "Upload these files to GitHub release ${releaseTag}:"
Write-Host "  $assetPath"
Write-Host "  $assetSignaturePath"
Write-Host "  $rootLatestJson"
