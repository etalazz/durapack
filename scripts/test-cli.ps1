# Requires: PowerShell 5+
# Purpose: End-to-end smoke tests for durapack CLI commands on Windows
# Usage:   pwsh -File scripts/test-cli.ps1
#          powershell -ExecutionPolicy Bypass -File scripts\test-cli.ps1

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

function Write-Section($msg) { Write-Host "`n=== $msg ===" -ForegroundColor Cyan }
function Assert-FileNotEmpty($Path) {
    if (-not (Test-Path $Path)) { throw "File not found: $Path" }
    $len = (Get-Item $Path).Length
    if ($len -le 0) { throw "File is empty: $Path" }
}
function Assert-ContentContains($Path, $Substring) {
    Assert-FileNotEmpty $Path
    $content = Get-Content -Raw -Encoding UTF8 $Path
    if ($content -notmatch [regex]::Escape($Substring)) { throw "Expected substring not found in ${Path}: ${Substring}" }
}

# Resolve repo root (assume script in scripts/ under repo)
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$RepoRoot  = Split-Path -Parent $ScriptDir
Push-Location $RepoRoot

try {
    $OutDir = Join-Path $ScriptDir 'out'
    if (Test-Path $OutDir) { Remove-Item $OutDir -Recurse -Force }
    New-Item -ItemType Directory -Force -Path $OutDir | Out-Null

    Write-Section 'Build CLI'
    & cargo build -p durapack-cli | Write-Host

    $Bin = Join-Path $RepoRoot 'target/debug/durapack.exe'
    if (-not (Test-Path $Bin)) { throw "Binary not found: $Bin" }

    # Create sample inputs
    $JsonArray = '[{"k":1},{"k":2},{"k":3}]'
    $JsonArrayPath = Join-Path $OutDir 'data.json'
    $JsonArray | Out-File -FilePath $JsonArrayPath -Encoding UTF8 -NoNewline

    $Jsonl = @(
        '{"k":1}'
        '{"k":2}'
        '{"k":3}'
    ) -join "`n"
    $JsonlPath = Join-Path $OutDir 'data.jsonl'
    $Jsonl | Out-File -FilePath $JsonlPath -Encoding UTF8 -NoNewline

    Write-Section 'pack (file input -> out_file.durp)'
    $OutDurpFile = Join-Path $OutDir 'out_file.durp'
    & $Bin pack -i $JsonArrayPath -o $OutDurpFile --blake3 | Write-Host
    Assert-FileNotEmpty $OutDurpFile

    Write-Section 'pack (stdin JSON array -> out_stdin.durp)'
    $OutDurpStdin = Join-Path $OutDir 'out_stdin.durp'
    Get-Content -Raw -Encoding UTF8 $JsonArrayPath | & $Bin pack -i - -o $OutDurpStdin --blake3 | Write-Host
    Assert-FileNotEmpty $OutDurpStdin

    Write-Section 'pack (stdin JSONL -> out_jsonl.durp)'
    $OutDurpJsonl = Join-Path $OutDir 'out_jsonl.durp'
    Get-Content -Raw -Encoding UTF8 $JsonlPath | & $Bin pack -i - -o $OutDurpJsonl --blake3 --jsonl | Write-Host
    Assert-FileNotEmpty $OutDurpJsonl

    Write-Section 'pack (single large frame)'
    $largeObject = @{ 'data' = 'a' * 10MB }
    $LargeJson = ConvertTo-Json -InputObject @($largeObject) -Compress
    $LargeJsonPath = Join-Path $OutDir 'large_data.json'
    Set-Content -Path $LargeJsonPath -Value $LargeJson -NoNewline
    $OutDurpLarge = Join-Path $OutDir 'out_large.durp'
    & $Bin pack -i $LargeJsonPath -o $OutDurpLarge --blake3 | Write-Host
    Assert-FileNotEmpty $OutDurpLarge

    Write-Section 'scan (JSONL -> stdout and file)'
    $ScanJsonl = Join-Path $OutDir 'scan.jsonl'
    & $Bin scan -i $OutDurpFile --jsonl -o $ScanJsonl | Write-Host
    Assert-FileNotEmpty $ScanJsonl
    Assert-ContentContains $ScanJsonl '"type":"stats"'

    Write-Section 'scan (pretty JSON -> scan.json)'
    $ScanJson = Join-Path $OutDir 'scan.json'
    & $Bin scan -i $OutDurpFile -o $ScanJson | Write-Host
    Assert-FileNotEmpty $ScanJson
    Assert-ContentContains $ScanJson '"frame_id"'

    Write-Section 'scan (carve payloads)'
    $CarvePattern = (Join-Path $OutDir 'payload_{stream}_{frame}.bin')
    & $Bin scan -i $OutDurpFile --jsonl --carve-payloads $CarvePattern -o - | Write-Host
    $carved = Get-ChildItem $OutDir -Filter 'payload_*_*.bin'
    if (-not $carved) { throw 'No carved payload files were produced' }

    Write-Section 'verify (file and stdin)'
    & $Bin verify -i $OutDurpFile --report-gaps | Write-Host
    cmd /c "type `"$OutDurpFile`"" | & $Bin verify -i - --report-gaps | Write-Host

    Write-Section 'timeline (DOT and JSON)'
    $DotPath = Join-Path $OutDir 'timeline.dot'
    & $Bin timeline -i $OutDurpFile --dot -o $DotPath | Write-Host
    Assert-FileNotEmpty $DotPath
    Assert-ContentContains $DotPath 'digraph timeline'

    $TimelineJson = Join-Path $OutDir 'timeline.json'
    & $Bin timeline -i $OutDurpFile -o $TimelineJson | Write-Host
    Assert-FileNotEmpty $TimelineJson
    Assert-ContentContains $TimelineJson '"frames"'

    Write-Host "`nAll CLI tests PASSED" -ForegroundColor Green
    Write-Host "Artifacts in: $OutDir"
}
catch {
    Write-Error $_
    exit 1
}
finally {
    Pop-Location
}
