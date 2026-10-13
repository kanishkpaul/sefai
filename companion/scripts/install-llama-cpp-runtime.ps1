Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$repoRoot = Split-Path -Parent $PSScriptRoot
Set-Location $repoRoot

$version = "b9490"
$runtimeDir = Join-Path $repoRoot "runtime_tools\llama_cpp"
$extractDir = Join-Path $runtimeDir $version
$zipPath = Join-Path $runtimeDir "llama-$version-bin-win-cpu-x64.zip"
$downloadUrl = "https://github.com/ggml-org/llama.cpp/releases/download/$version/llama-$version-bin-win-cpu-x64.zip"

New-Item -ItemType Directory -Force -Path $runtimeDir | Out-Null

if (-not (Test-Path (Join-Path $extractDir "llama-cli.exe"))) {
    Write-Host "Downloading llama.cpp runtime $version..."
    Invoke-WebRequest $downloadUrl -OutFile $zipPath

    if (Test-Path $extractDir) {
        Remove-Item -Recurse -Force $extractDir
    }

    Write-Host "Extracting llama.cpp runtime..."
    Expand-Archive -LiteralPath $zipPath -DestinationPath $extractDir -Force
} else {
    Write-Host "llama.cpp runtime already installed at $extractDir"
}

Write-Host "Runtime ready: $(Join-Path $extractDir 'llama-cli.exe')"
