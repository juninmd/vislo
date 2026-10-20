# Runs the same steps as .github/workflows/ci.yml locally (useful while hosted runners are unavailable).
$ErrorActionPreference = "Stop"
$env:CI = "true"
function Step($cmd) { Write-Host "> $cmd"; Invoke-Expression $cmd; if ($LASTEXITCODE -ne 0) { throw "failed: $cmd" } }
Step "npm ci --silent"
Step "npm test --silent"
Step "npm run build --silent"
Step "cargo test --quiet --manifest-path src-tauri/Cargo.toml"
Step "cargo clippy --quiet --all-targets --manifest-path src-tauri/Cargo.toml -- -D warnings"
Write-Host "local CI green"
