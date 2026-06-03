Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$repoRoot = Split-Path -Parent $PSScriptRoot
Set-Location $repoRoot

Write-Host "Installing Python dependencies..."
python -m pip install -r requirements.txt

$dotnetCommand = Get-Command dotnet -ErrorAction SilentlyContinue
if (-not $dotnetCommand) {
    Write-Warning ".NET SDK was not found on PATH. Install it with: winget install Microsoft.DotNet.SDK.8"
} else {
    Write-Host "Building WPF project..."
    dotnet build .\frontend\CompanionApp\CompanionApp.csproj
}

Write-Host "Running backend tests..."
python -m pytest tests
