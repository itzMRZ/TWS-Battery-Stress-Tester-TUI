# Experimental Windows installer for tws-tester (probe only; soaks are Linux).
# Install or update from the latest GitHub release. SHA-256 is checked first.
#
#   irm https://github.com/itzMRZ/TWS-Battery-Stress-Tester-TUI/releases/latest/download/install.ps1 | iex

$ErrorActionPreference = "Stop"
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12

$Repo = if ($env:TWS_TESTER_REPO) { $env:TWS_TESTER_REPO } else { "https://github.com/itzMRZ/TWS-Battery-Stress-Tester-TUI" }
$DestDir = if ($env:TWS_TESTER_BIN) { $env:TWS_TESTER_BIN } else { Join-Path $env:LOCALAPPDATA "tws-tester" }
$Dest = Join-Path $DestDir "tws-tester.exe"
$Ua = "tws-tester-install (+$Repo)"

Write-Host "Windows is experimental: probe works, soak and playback do not."

$arch = $env:PROCESSOR_ARCHITECTURE
if ($arch -ne "AMD64") {
    throw "no GitHub binary for Windows $arch (x86_64 only). Build from source: cargo build --release"
}

$asset = "tws-tester-x86_64-pc-windows-msvc.exe"
$url = "$Repo/releases/latest/download/$asset"
$tmpDir = Join-Path $env:TEMP ("tws-tester-install-" + [guid]::NewGuid().ToString("n"))
New-Item -ItemType Directory -Path $tmpDir | Out-Null
$bin = Join-Path $tmpDir $asset
$sumFile = Join-Path $tmpDir "$asset.sha256"

function Fetch([string]$From, [string]$To) {
    try {
        Invoke-WebRequest -UseBasicParsing -Uri $From -OutFile $To -UserAgent $Ua -TimeoutSec 120
    } catch {
        throw @"
could not download $From
Publish a GitHub release (tag vX.Y.Z matching Cargo.toml) or build from source: cargo build --release
"@
    }
    if (-not (Test-Path $To) -or (Get-Item $To).Length -eq 0) {
        throw "download was empty: $From"
    }
}

try {
    Write-Host "downloading $url"
    Fetch $url $bin
    Fetch "$url.sha256" $sumFile

    $got = (Get-FileHash -Algorithm SHA256 -Path $bin).Hash.ToLowerInvariant()
    $want = ((Get-Content -Raw $sumFile).Trim() -split "\s+")[0].ToLowerInvariant()
    if ($want.Length -ne 64) {
        throw "checksum file is not a SHA-256 hex digest"
    }
    if ($got -ne $want) {
        throw "SHA-256 mismatch (got $got, expected $want). Left the installed binary alone."
    }

    $fs = [System.IO.File]::OpenRead($bin)
    try {
        $mz0 = $fs.ReadByte()
        $mz1 = $fs.ReadByte()
    } finally {
        $fs.Dispose()
    }
    if ($mz0 -ne 0x4D -or $mz1 -ne 0x5A) {
        throw "downloaded file is not a Windows executable"
    }

    $ver = & $bin --version
    if ($LASTEXITCODE -ne 0 -or "$ver" -notmatch "^tws-tester ") {
        throw "downloaded binary did not report tws-tester --version"
    }

    New-Item -ItemType Directory -Force -Path $DestDir | Out-Null
    $new = Join-Path $DestDir "tws-tester.new.exe"
    Copy-Item -Force $bin $new
    if (Test-Path $Dest) {
        $bak = Join-Path $DestDir "tws-tester.bak.exe"
        Move-Item -Force $Dest $bak
        try {
            Move-Item -Force $new $Dest
            Remove-Item -Force $bak -ErrorAction SilentlyContinue
        } catch {
            Move-Item -Force $bak $Dest
            throw "could not replace $Dest. New binary is at $new. Move it into place after tws-tester exits."
        }
    } else {
        Move-Item -Force $new $Dest
    }

    Write-Host "SHA-256 ok"
    Write-Host $ver
    Write-Host "installed $Dest"

    $userPath = [Environment]::GetEnvironmentVariable("Path", "User")
    if ($userPath -notlike "*$DestDir*") {
        [Environment]::SetEnvironmentVariable("Path", "$userPath;$DestDir", "User")
        $env:Path = "$env:Path;$DestDir"
        Write-Host "added $DestDir to user PATH (new shells pick it up)"
    }
} finally {
    Remove-Item -Recurse -Force $tmpDir -ErrorAction SilentlyContinue
}
