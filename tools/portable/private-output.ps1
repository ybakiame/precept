# Called immediately after exclusive creation; never adopts a pre-existing tree.
param([Parameter(Mandatory)][string]$Directory)
$ErrorActionPreference = 'Stop'
$item = Get-Item -LiteralPath $Directory -Force
if (-not $item.PSIsContainer -or ($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -or @(Get-ChildItem -LiteralPath $Directory -Force).Count -ne 0) { throw 'Expected fresh empty regular output directory' }
$identity = [Security.Principal.WindowsIdentity]::GetCurrent().User
$acl = Get-Acl -LiteralPath $Directory
# An elevated administrator's new objects are owned by the Administrators
# group (as on hosted runners); this exclusive, fresh directory then takes the
# user as owner below. Any other owner is refused.
$owner = $acl.GetOwner([Security.Principal.SecurityIdentifier])
$administrators = [Security.Principal.SecurityIdentifier]::new('S-1-5-32-544')
$elevated = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole($administrators)
if ($owner -ne $identity -and -not ($elevated -and $owner -eq $administrators)) { throw 'Output ownership mismatch' }
$newAcl = [Security.AccessControl.DirectorySecurity]::new()
$newAcl.SetOwner($identity)
$newAcl.SetAccessRuleProtection($true, $false)
$newAcl.AddAccessRule([Security.AccessControl.FileSystemAccessRule]::new($identity, 'FullControl', 'ContainerInherit,ObjectInherit', 'None', 'Allow'))
Set-Acl -LiteralPath $Directory -AclObject $newAcl
$checked = Get-Acl -LiteralPath $Directory
if (-not $checked.AreAccessRulesProtected -or $checked.GetOwner([Security.Principal.SecurityIdentifier]) -ne $identity) { throw 'Private output ACL unavailable' }
foreach ($rule in $checked.GetAccessRules($true, $true, [Security.Principal.SecurityIdentifier])) {
    if ($rule.IdentityReference -ne $identity -or $rule.AccessControlType -ne 'Allow') { throw 'Unexpected output principal' }
}
