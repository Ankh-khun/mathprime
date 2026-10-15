<#
.SYNOPSIS
    Maakt het installatiepakket target\installer\MathPrime-<versie>-setup.exe.

.DESCRIPTION
    Bouwt mathprime en mathprime-cli (release), zet ze met tools\installer\install.ps1 en
    uninstall.ps1, de README en de voorbeelden in een map, en pakt die in met IExpress (zit in
    Windows; niets te downloaden). Het pakket pakt zichzelf uit in een tijdelijke map en start
    install.ps1: installeren voor de huidige gebruiker, zonder beheerdersrechten.

.EXAMPLE
    powershell -ExecutionPolicy Bypass -File tools\maak-installer.ps1
#>
param([switch]$NietBouwen)
$ErrorActionPreference = "Stop"
$root = Resolve-Path (Join-Path $PSScriptRoot "..")
$versie = (Select-String -Path (Join-Path $root "Cargo.toml") -Pattern '^version = "(.+)"').Matches[0].Groups[1].Value

if (-not $NietBouwen) {
    Push-Location $root
    cargo build --release -p rb-gui -p rb-cli
    $ok = $?
    Pop-Location
    if (-not $ok) { throw "cargo build mislukt (draait mathprime.exe uit target\release nog?)" }
}

$uit = Join-Path $root "target\installer"
$pakket = Join-Path $uit "pakket"
if (Test-Path $pakket) { Remove-Item $pakket -Recurse -Force }
New-Item -ItemType Directory -Force $pakket | Out-Null

Copy-Item (Join-Path $root "target\release\mathprime.exe") $pakket
Copy-Item (Join-Path $root "target\release\mathprime-cli.exe") $pakket
Copy-Item (Join-Path $root "README.md") (Join-Path $pakket "LEESMIJ.md")
Set-Content (Join-Path $pakket "versie.txt") $versie -Encoding ascii
# Windows PowerShell 5.1 leest scripts zonder BOM als ANSI: met BOM wegschrijven (ë, ï in meldingen)
$utf8bom = New-Object System.Text.UTF8Encoding($true)
foreach ($s in "install.ps1", "uninstall.ps1") {
    $tekst = [IO.File]::ReadAllText((Join-Path $PSScriptRoot "installer\$s"))
    [IO.File]::WriteAllText((Join-Path $pakket $s), $tekst, $utf8bom)
}
foreach ($v in Get-ChildItem (Join-Path $root "examples") -Filter "*.rkn") {
    Copy-Item $v.FullName (Join-Path $pakket "voorbeeld_$($v.Name)")
}

$exe = Join-Path $uit "MathPrime-$versie-setup.exe"
$files = Get-ChildItem $pakket -File
$sed = @"
[Version]
Class=IEXPRESS
SEDVersion=3
[Options]
PackagePurpose=InstallApp
ShowInstallProgramWindow=1
HideExtractAnimation=1
UseLongFileName=1
InsideCompressed=0
CAB_FixedSize=0
CAB_ResvCodeSigning=0
RebootMode=N
InstallPrompt=%InstallPrompt%
DisplayLicense=%DisplayLicense%
FinishMessage=%FinishMessage%
TargetName=%TargetName%
FriendlyName=%FriendlyName%
AppLaunched=%AppLaunched%
PostInstallCmd=%PostInstallCmd%
AdminQuietInstCmd=%AdminQuietInstCmd%
UserQuietInstCmd=%UserQuietInstCmd%
SourceFiles=SourceFiles
[Strings]
InstallPrompt=
DisplayLicense=
FinishMessage=
TargetName=$exe
FriendlyName=MathPrime $versie
AppLaunched=powershell.exe -NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -File install.ps1
PostInstallCmd=<None>
AdminQuietInstCmd=powershell.exe -NoProfile -ExecutionPolicy Bypass -File install.ps1 -Stil
UserQuietInstCmd=powershell.exe -NoProfile -ExecutionPolicy Bypass -File install.ps1 -Stil
"@
for ($k = 0; $k -lt $files.Count; $k++) { $sed += "`r`nFILE$k=`"$($files[$k].Name)`"" }
$sed += "`r`n[SourceFiles]`r`nSourceFiles0=$pakket\`r`n[SourceFiles0]"
for ($k = 0; $k -lt $files.Count; $k++) { $sed += "`r`n%FILE$k%=" }
$sed = $sed -replace "`r?`n", "`r`n"   # IExpress wil overal CRLF
$sedPad = Join-Path $uit "mathprime.sed"
Set-Content $sedPad $sed -Encoding ascii

if (Test-Path $exe) { Remove-Item $exe -Force }
$p = Start-Process -FilePath "$env:SystemRoot\System32\iexpress.exe" -ArgumentList "/N", "/Q", $sedPad -Wait -PassThru
if (-not (Test-Path $exe)) { throw "IExpress maakte geen pakket (code $($p.ExitCode)); zie $sedPad" }
"installatiepakket: $exe ({0:N1} MB)" -f ((Get-Item $exe).Length / 1MB)
