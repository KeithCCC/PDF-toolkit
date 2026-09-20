$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
$destination = Join-Path $projectRoot 'vendor/pdfium'
New-Item -ItemType Directory -Force -Path $destination | Out-Null
$archive = Join-Path $destination 'pdfium.tgz'
Invoke-WebRequest 'https://github.com/bblanchon/pdfium-binaries/releases/download/chromium%2F7881/pdfium-win-x64.tgz' -OutFile $archive
$expected = '73CC0DE638AC2095E7445BF56A38200A5B7C7CA0E9F4BA144598F2457377AC08'
if ((Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash -ne $expected) { throw 'PDFium archive checksum mismatch' }
tar -xzf $archive -C $destination
if ($LASTEXITCODE -ne 0) { throw 'PDFium extraction failed' }
Write-Host 'PDFium chromium/7881 x64 is ready. Run npm ci, then npm run tauri dev.'
