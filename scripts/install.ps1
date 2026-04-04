param(
  [string]$Version = $env:AYX_VERSION,
  [string]$InstallDir = $env:AYX_INSTALL_DIR
)

$ErrorActionPreference = 'Stop'

if (-not $Version) { $Version = 'latest' }
if (-not $InstallDir) { $InstallDir = Join-Path $HOME '.local\bin' }

$repoOwner = 'RyanMerlin'
$repoName = 'ayx-cli'
$artifactName = 'ayx-x86_64-pc-windows-msvc.zip'

$downloadUrl = if ($Version -eq 'latest') {
  "https://github.com/$repoOwner/$repoName/releases/latest/download/$artifactName"
} else {
  "https://github.com/$repoOwner/$repoName/releases/download/$Version/$artifactName"
}

$tmpDir = Join-Path ([System.IO.Path]::GetTempPath()) ("ayx-install-" + [guid]::NewGuid().ToString())
New-Item -ItemType Directory -Force -Path $tmpDir | Out-Null

try {
  $archivePath = Join-Path $tmpDir $artifactName
  try {
    Invoke-WebRequest -Uri $downloadUrl -OutFile $archivePath
  } catch {
    throw "failed to download $downloadUrl. $($_.Exception.Message)"
  }

  New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
  $extractDir = Join-Path $tmpDir 'extract'
  New-Item -ItemType Directory -Force -Path $extractDir | Out-Null
  try {
    Expand-Archive -Path $archivePath -DestinationPath $extractDir -Force
  } catch {
    $listing = & tar -tf $archivePath 2>$null
    if ($listing) {
      Write-Host "archive contents:"
      $listing | ForEach-Object { Write-Host $_ }
    }
    throw "failed to extract $downloadUrl. $($_.Exception.Message)"
  }

  $binaryPath = Get-ChildItem -Path $extractDir -Recurse -File -Filter 'ayx.exe' | Select-Object -First 1
  if (-not $binaryPath) {
    Write-Host "archive contents:"
    Get-ChildItem -Path $extractDir -Recurse | ForEach-Object { Write-Host $_.FullName }
    throw 'downloaded archive did not contain ayx.exe'
  }

  Copy-Item $binaryPath.FullName -Destination (Join-Path $InstallDir 'ayx.exe') -Force

  Write-Host "installed ayx to $InstallDir\ayx.exe"
  Write-Host "make sure $InstallDir is on your PATH"
}
finally {
  Remove-Item -Recurse -Force $tmpDir -ErrorAction SilentlyContinue
}
