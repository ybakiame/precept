# Explicit post-smoke removal of this attempt's owned extracted prototype only.
param([Parameter(Mandatory)][string]$AttemptDir, [Parameter(Mandatory)][string]$SmokeName)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
if ($SmokeName -notmatch '^smoke-[0-9]+$') { throw 'Expected owned smoke label' }
$attempt = (Resolve-Path -LiteralPath $AttemptDir).Path
$extracted = Join-Path $attempt 'extracted'
$smoke = Join-Path $attempt $SmokeName
$report = Get-Content -LiteralPath (Join-Path $smoke 'smoke.json') -Raw | ConvertFrom-Json
if (@($report.cells | Where-Object status -ne 'PASS').Count -ne 0) { throw 'Successful owned smoke required before removal' }
$state = (Get-Content -LiteralPath (Join-Path $smoke 'private-fixture-root.txt') -Raw).Trim()
# The path is a private pointer produced by our harness; never print or export it.
$before = @{}
foreach ($name in @('profile.sqlite3','auth-retain.txt','unknown-retain.txt')) {
    $before[$name] = (Get-FileHash -LiteralPath (Join-Path $state "state/$name") -Algorithm SHA256).Hash
}
# Unknown files beside and inside the product tree must survive exact removal.
$sentinels = @('synthetic-auth-retain.txt', 'Precept/bin/unknown-retain.txt')
foreach ($name in $sentinels) {
    $file = [IO.File]::Open((Join-Path $extracted $name), [IO.FileMode]::CreateNew, [IO.FileAccess]::Write)
    try { $bytes = [Text.Encoding]::UTF8.GetBytes('SYNTHETIC RETAIN'); $file.Write($bytes) } finally { $file.Dispose() }
}
$recipe = Join-Path $PSScriptRoot 'windows-package.mjs'
$removal = & node $recipe --remove-owned-product $extracted (Join-Path $attempt 'archive-inventory.json')
if ($LASTEXITCODE -ne 0) { throw 'Removal preflight refused; retain files' }
$removal = $removal | ConvertFrom-Json
foreach ($name in $before.Keys) {
    if ((Get-FileHash -LiteralPath (Join-Path $state "state/$name") -Algorithm SHA256).Hash -ne $before[$name]) { throw 'Separate synthetic state retention failed' }
}
foreach ($name in $sentinels) {
    if ([IO.File]::ReadAllText((Join-Path $extracted $name)) -ne 'SYNTHETIC RETAIN') { throw 'Unknown extraction file retention failed' }
}
# Empty product directories go; a directory holding an unknown file stays.
foreach ($gone in @('Precept/runtime', 'Precept/notices')) { if (Test-Path -LiteralPath (Join-Path $extracted $gone)) { throw "Empty product directory retained: $gone" } }
$left = @(Get-ChildItem -LiteralPath (Join-Path $extracted 'Precept') -Recurse -Force -File | ForEach-Object { [IO.Path]::GetRelativePath($extracted, $_.FullName).Replace('\', '/') })
if ($left.Count -ne 1 -or $left[0] -ne 'Precept/bin/unknown-retain.txt') { throw 'Unexpected files after exact removal' }
[ordered]@{ source_sha = $report.source_sha; result = 'PASS'; removed_inventory_files = $removal.removed_files; retained_directories = $removal.retained_directories; retained_separate_profile_auth_unknown = $true; retained_unknown_extraction_files = $sentinels.Count; recursive_delete = $false } | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $attempt 'retention.json')
'PASS: exact owned product removal; separate synthetic Profile/auth/unknown and unknown extraction files retained'
