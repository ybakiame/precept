# Synthetic negative fixture only. The caller exclusively created this empty directory.
param([Parameter(Mandatory)][string]$Directory)
$ErrorActionPreference = 'Stop'
$item = Get-Item -LiteralPath $Directory
if (-not $item.PSIsContainer -or ($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -or @(Get-ChildItem -LiteralPath $Directory -Force).Count -ne 0) { throw 'Expected empty owned fixture' }
$identity = [Security.Principal.WindowsIdentity]::GetCurrent().User
$acl = Get-Acl -LiteralPath $Directory
if ($acl.GetOwner([Security.Principal.SecurityIdentifier]) -ne $identity) { throw 'Fixture ownership mismatch' }
$everyone = [Security.Principal.SecurityIdentifier]::new('S-1-1-0')
$acl.AddAccessRule([Security.AccessControl.FileSystemAccessRule]::new($everyone, 'ReadAndExecute', 'Allow'))
Set-Acl -LiteralPath $Directory -AclObject $acl
@{ synthetic_acl_fixture = $true } | ConvertTo-Json -Compress
