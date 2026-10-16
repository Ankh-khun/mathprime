<#
.SYNOPSIS
    Maakt het installatiepakket target\installer\MathPrime-<versie>-setup.exe.

.DESCRIPTION
    Bouwt mathprime, mathprime-cli en mathprime-setup (release), zet de programma's met
    tools\installer\install.ps1 en uninstall.ps1, de README en de voorbeelden in een map, en plakt die
    als ZIP achter mathprime-setup.exe (mathprime-setup --pak). Het installatieprogramma (crates\rb-setup)
    toont een venster, pakt uit in een tijdelijke map en start install.ps1: installeren voor de huidige
    gebruiker, zonder beheerdersrechten. /Q installeert zonder venster.

.EXAMPLE
    powershell -ExecutionPolicy Bypass -File tools\maak-installer.ps1
#>
param([switch]$NietBouwen)
$ErrorActionPreference = "Stop"
$root = Resolve-Path (Join-Path $PSScriptRoot "..")
$versie = (Select-String -Path (Join-Path $root "Cargo.toml") -Pattern '^version = "(.+)"').Matches[0].Groups[1].Value

if (-not $NietBouwen) {
    Push-Location $root
    cargo build --release -p rb-gui -p rb-cli -p rb-setup
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
if (Test-Path $exe) { Remove-Item $exe -Force }
$setup = Join-Path $root "target\release\mathprime-setup.exe"
# een venster-programma: met Start-Process -Wait op het einde wachten
$p = Start-Process -FilePath $setup -ArgumentList "--pak", "`"$pakket`"", "`"$exe`"" -Wait -PassThru -NoNewWindow
if ($p.ExitCode -ne 0 -or -not (Test-Path $exe)) { throw "mathprime-setup --pak maakte geen pakket (code $($p.ExitCode))" }
"installatiepakket: $exe ({0:N1} MB)" -f ((Get-Item $exe).Length / 1MB)
