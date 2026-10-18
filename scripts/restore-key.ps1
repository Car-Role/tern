# Restores Tern's updater signing key from the encrypted backup made by backup-key.ps1.
# Usage: right-click > Run with PowerShell, or: powershell -File restore-key.ps1
param([string]$File = (Join-Path $PSScriptRoot "tern-updater.key.enc"), [string]$Passphrase, [switch]$CheckOnly)
$ErrorActionPreference = "Stop"
if (-not $Passphrase) {
  $s = Read-Host "Passphrase" -AsSecureString
  $b = [Runtime.InteropServices.Marshal]::SecureStringToBSTR($s)
  try { $Passphrase = [Runtime.InteropServices.Marshal]::PtrToStringBSTR($b) } finally { [Runtime.InteropServices.Marshal]::ZeroFreeBSTR($b) }
}
$all = [IO.File]::ReadAllBytes($File)
if ([Text.Encoding]::ASCII.GetString($all, 0, 8) -ne "TERNKEY1") { throw "This is not a Tern key backup." }
$body = $all[8..($all.Length - 33)]; $mac = $all[($all.Length - 32)..($all.Length - 1)]
$salt = $body[0..15]; $iv = $body[16..31]; $cipher = [byte[]]$body[32..($body.Length - 1)]
$kdf = New-Object Security.Cryptography.Rfc2898DeriveBytes($Passphrase, [byte[]]$salt, 600000, [Security.Cryptography.HashAlgorithmName]::SHA256)
$keys = $kdf.GetBytes(64); $encKey = $keys[0..31]; $macKey = $keys[32..63]
$want = (New-Object Security.Cryptography.HMACSHA256(,[byte[]]$macKey)).ComputeHash([byte[]]$body)
if ([Convert]::ToBase64String($want) -ne [Convert]::ToBase64String([byte[]]$mac)) {
  if ($CheckOnly) { return "BAD" }
  throw "Wrong passphrase, or the backup is damaged."
}
$aes = [Security.Cryptography.Aes]::Create(); $aes.Key = [byte[]]$encKey; $aes.IV = [byte[]]$iv
$plain = $aes.CreateDecryptor().TransformFinalBlock($cipher, 0, $cipher.Length)
if ($CheckOnly) { return "OK" }
$dest = Join-Path $env:USERPROFILE ".tauri\tern-updater.key"
if (Test-Path $dest) { $dest = "$dest.restored" }
New-Item -ItemType Directory -Force (Split-Path $dest) | Out-Null
[IO.File]::WriteAllBytes($dest, $plain)
Write-Host "Restored to $dest" -ForegroundColor Green
