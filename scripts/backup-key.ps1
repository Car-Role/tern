# Makes an encrypted backup of Tern's updater signing key in OneDrive.
# You type a passphrase; it is never stored or shown. AES-256, key from PBKDF2-SHA256 (600,000 rounds), with an HMAC check.
$ErrorActionPreference = "Stop"
$src = Join-Path $env:USERPROFILE ".tauri\tern-updater.key"
$pub = Join-Path $env:USERPROFILE ".tauri\tern-updater.key.pub"
$dir = Join-Path $env:OneDrive "Tern key backup"
New-Item -ItemType Directory -Force $dir | Out-Null

function Plain($secure) {
  $b = [Runtime.InteropServices.Marshal]::SecureStringToBSTR($secure)
  try { [Runtime.InteropServices.Marshal]::PtrToStringBSTR($b) } finally { [Runtime.InteropServices.Marshal]::ZeroFreeBSTR($b) }
}

Write-Host ""
Write-Host "Back up Tern's updater key to: $dir" -ForegroundColor Cyan
Write-Host "Choose a passphrase you will remember (12+ characters). Without it, the backup cannot be opened." -ForegroundColor Cyan
$p1 = Plain (Read-Host "Passphrase" -AsSecureString)
$p2 = Plain (Read-Host "Type it again" -AsSecureString)
if ($p1 -ne $p2) { Write-Host "The two passphrases are different. Nothing was saved." -ForegroundColor Red; exit 1 }
if ($p1.Length -lt 12) { Write-Host "Use at least 12 characters. Nothing was saved." -ForegroundColor Red; exit 1 }

$data = [IO.File]::ReadAllBytes($src)
$salt = New-Object byte[] 16; $iv = New-Object byte[] 16
$rng = [Security.Cryptography.RandomNumberGenerator]::Create(); $rng.GetBytes($salt); $rng.GetBytes($iv)
$kdf = New-Object Security.Cryptography.Rfc2898DeriveBytes($p1, $salt, 600000, [Security.Cryptography.HashAlgorithmName]::SHA256)
$keys = $kdf.GetBytes(64); $encKey = $keys[0..31]; $macKey = $keys[32..63]
$aes = [Security.Cryptography.Aes]::Create(); $aes.Key = $encKey; $aes.IV = $iv
$cipher = $aes.CreateEncryptor().TransformFinalBlock($data, 0, $data.Length)
$body = [byte[]]($salt + $iv + $cipher)
$mac = (New-Object Security.Cryptography.HMACSHA256(,$macKey)).ComputeHash($body)
$out = Join-Path $dir "tern-updater.key.enc"
[IO.File]::WriteAllBytes($out, [byte[]]([Text.Encoding]::ASCII.GetBytes("TERNKEY1") + $body + $mac))
Copy-Item $pub (Join-Path $dir "tern-updater.key.pub") -Force
Copy-Item (Join-Path $PSScriptRoot "restore-key.ps1") (Join-Path $dir "restore-key.ps1") -Force

# Check the backup opens with the same passphrase before saying it worked.
$check = & (Join-Path $dir "restore-key.ps1") -File $out -Passphrase $p1 -CheckOnly
if ($check -ne "OK") { Write-Host "The backup did not verify. Delete it and try again." -ForegroundColor Red; exit 1 }
Write-Host ""
Write-Host "Done. The backup is saved and verified:" -ForegroundColor Green
Write-Host "  $out"
Write-Host "To restore it later, run restore-key.ps1 in that folder."
Write-Host "Also save the passphrase in a password manager." -ForegroundColor Yellow
