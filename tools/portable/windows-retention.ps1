# Explicit post-smoke removal of this attempt's owned extracted prototype only.
param([Parameter(Mandatory)][string]$AttemptDir, [Parameter(Mandatory)][string]$SmokeName)
$ErrorActionPreference = 'Stop'
if ($SmokeName -notmatch '^smoke-[0-9]+$') { throw 'Expected owned smoke label' }
$attempt = (Resolve-Path -LiteralPath $AttemptDir).Path
$package = Join-Path $attempt 'extracted'
$smoke = Join-Path $attempt $SmokeName
$report = Get-Content -LiteralPath (Join-Path $smoke 'smoke.json') -Raw | ConvertFrom-Json
if (@($report.cells | Where-Object status -ne 'PASS').Count -ne 0) { throw 'Successful owned smoke required before removal' }
$state = (Get-Content -LiteralPath (Join-Path $smoke 'private-fixture-root.txt') -Raw).Trim()
# The path is a private pointer produced by our harness; never print or export it.
$before = @{}
foreach ($name in @('profile.sqlite3','auth-retain.txt','unknown-retain.txt')) {
    $before[$name] = (Get-FileHash -LiteralPath (Join-Path $state "state/$name") -Algorithm SHA256).Hash
}
foreach ($name in @('synthetic-auth-retain.txt','unknown-retain.txt')) {
    $file = [IO.File]::Open((Join-Path $package $name), [IO.FileMode]::CreateNew, [IO.FileAccess]::Write)
    try { $bytes = [Text.Encoding]::UTF8.GetBytes('SYNTHETIC RETAIN'); $file.Write($bytes) } finally { $file.Dispose() }
}
$recipe = Join-Path $PSScriptRoot 'windows-package.mjs'
& node $recipe --remove-owned-product $package (Join-Path $attempt 'archive-inventory.json')
if ($LASTEXITCODE -ne 0) { throw 'Removal preflight refused; retain files' }
foreach ($name in $before.Keys) {
    if ((Get-FileHash -LiteralPath (Join-Path $state "state/$name") -Algorithm SHA256).Hash -ne $before[$name]) { throw 'Separate synthetic state retention failed' }
}
foreach ($name in @('synthetic-auth-retain.txt','unknown-retain.txt')) {
    if ([IO.File]::ReadAllText((Join-Path $package $name)) -ne 'SYNTHETIC RETAIN') { throw 'Unknown extraction file retention failed' }
}
@{ source_sha = $report.source_sha; result = 'PASS'; removed_inventory_files = @(Get-Content -LiteralPath (Join-Path $attempt 'archive-inventory.json') -Raw | ConvertFrom-Json).Count; retained_separate_profile_auth_unknown = $true; retained_unknown_extraction_files = 2; recursive_delete = $false } | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $attempt 'retention.json')
'PASS: exact owned product removal; separate synthetic Profile/auth/unknown and unknown extraction files retained'
