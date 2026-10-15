<#
.SYNOPSIS
    Installeert MathPrime voor de huidige gebruiker (geen beheerdersrechten nodig).

.DESCRIPTION
    - kopieert mathprime.exe en mathprime-cli.exe naar %LOCALAPPDATA%\Programs\MathPrime
    - snelkoppelingen in het Startmenu en (optioneel) op het bureaublad
    - .rkn-bestanden openen met MathPrime; voor .mcdx alleen "Openen met" (Mathcad blijft de standaard)
    - de map op het PATH van de gebruiker (voor mathprime-cli)
    - een vermelding in Instellingen > Apps, met verwijderen

    Wordt uitgevoerd vanuit het installatiepakket (MathPrime-x.y.z-setup.exe), of los vanuit een map
    met de bestanden. -Stil: geen vensters (voor tests). -Doel: een andere installatiemap.
    -AlleenBestanden: alleen kopieren (geen register, snelkoppelingen of PATH; voor tests).
#>
param(
    [string]$Doel = (Join-Path $env:LOCALAPPDATA "Programs\MathPrime"),
    [switch]$Stil,
    [switch]$GeenBureaublad,
    [switch]$AlleenBestanden
)
$ErrorActionPreference = "Stop"
$bron = $PSScriptRoot
$versie = (Get-Content (Join-Path $bron "versie.txt") -ErrorAction SilentlyContinue | Select-Object -First 1)
if (-not $versie) { $versie = "onbekend" }
Add-Type -AssemblyName System.Windows.Forms

function Melding([string]$tekst, [string]$soort = "Information") {
    if ($Stil) { Write-Output $tekst; return }
    [void][System.Windows.Forms.MessageBox]::Show($tekst, "MathPrime $versie", "OK", $soort)
}

try {
    # een draaiende MathPrime uit deze map houdt het bestand vast: niet afsluiten (er kan werk open staan)
    $exe = Join-Path $Doel "mathprime.exe"
    $draait = Get-Process mathprime -ErrorAction SilentlyContinue | Where-Object { $_.Path -eq $exe }
    if ($draait) {
        Melding "MathPrime is nog geopend. Sla je werk op, sluit MathPrime en start de installatie opnieuw." "Warning"
        exit 1
    }

    New-Item -ItemType Directory -Force $Doel | Out-Null
    foreach ($f in "mathprime.exe", "mathprime-cli.exe", "uninstall.ps1", "LEESMIJ.md", "versie.txt") {
        $p = Join-Path $bron $f
        if (Test-Path $p) { Copy-Item $p (Join-Path $Doel $f) -Force }
    }
    # voorbeelden: in het pakket plat (IExpress kent geen mappen) als voorbeeld_<naam>.rkn
    $voorbeelden = Get-ChildItem $bron -Filter "voorbeeld_*.rkn" -ErrorAction SilentlyContinue
    if ($voorbeelden) {
        $vm = Join-Path $Doel "voorbeelden"
        New-Item -ItemType Directory -Force $vm | Out-Null
        foreach ($v in $voorbeelden) { Copy-Item $v.FullName (Join-Path $vm ($v.Name -replace "^voorbeeld_", "")) -Force }
    }
    if ($AlleenBestanden) { Melding "Bestanden gekopieerd naar $Doel"; exit 0 }

    # snelkoppelingen
    $shell = New-Object -ComObject WScript.Shell
    $start = Join-Path ([Environment]::GetFolderPath("Programs")) "MathPrime.lnk"
    $doelen = @($start)
    if (-not $GeenBureaublad) { $doelen += Join-Path ([Environment]::GetFolderPath("Desktop")) "MathPrime.lnk" }
    foreach ($lnk in $doelen) {
        $s = $shell.CreateShortcut($lnk)
        $s.TargetPath = $exe
        $s.WorkingDirectory = [Environment]::GetFolderPath("MyDocuments")
        $s.IconLocation = "$exe,0"
        $s.Description = "MathPrime - rekenblad met eenheden"
        $s.Save()
    }

    # bestandstypen (alleen voor deze gebruiker)
    $classes = "HKCU:\Software\Classes"
    $progid = "MathPrime.Werkblad"
    New-Item -Force "$classes\$progid" | Out-Null
    Set-ItemProperty "$classes\$progid" "(default)" "MathPrime-werkblad"
    New-Item -Force "$classes\$progid\DefaultIcon" | Out-Null
    Set-ItemProperty "$classes\$progid\DefaultIcon" "(default)" "`"$exe`",0"
    New-Item -Force "$classes\$progid\shell\open\command" | Out-Null
    Set-ItemProperty "$classes\$progid\shell\open\command" "(default)" "`"$exe`" `"%1`""
    New-Item -Force "$classes\.rkn" | Out-Null
    Set-ItemProperty "$classes\.rkn" "(default)" $progid
    New-Item -Force "$classes\.rkn\OpenWithProgids" | Out-Null
    New-ItemProperty "$classes\.rkn\OpenWithProgids" $progid -Value "" -PropertyType String -Force | Out-Null
    # .mcdx: alleen in "Openen met", de standaard (Mathcad) blijft
    New-Item -Force "$classes\.mcdx\OpenWithProgids" | Out-Null
    New-ItemProperty "$classes\.mcdx\OpenWithProgids" $progid -Value "" -PropertyType String -Force | Out-Null

    # mathprime-cli op het PATH van de gebruiker
    $pad = [Environment]::GetEnvironmentVariable("Path", "User")
    $delen = @($pad -split ";" | Where-Object { $_ -ne "" })
    if ($delen -notcontains $Doel) {
        [Environment]::SetEnvironmentVariable("Path", (($delen + $Doel) -join ";"), "User")
    }

    # Instellingen > Apps
    $uninst = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\MathPrime"
    New-Item -Force $uninst | Out-Null
    $grootte = [int]((Get-ChildItem $Doel -Recurse -File | Measure-Object Length -Sum).Sum / 1KB)
    $verwijder = "powershell.exe -NoProfile -ExecutionPolicy Bypass -File `"$Doel\uninstall.ps1`""
    $waarden = @{
        DisplayName = "MathPrime"; DisplayVersion = $versie; Publisher = "MathPrime"
        DisplayIcon = "`"$exe`",0"; InstallLocation = $Doel
        UninstallString = $verwijder; QuietUninstallString = "$verwijder -Stil"
    }
    foreach ($k in $waarden.Keys) { Set-ItemProperty $uninst $k $waarden[$k] }
    foreach ($k in "NoModify", "NoRepair") { New-ItemProperty $uninst $k -Value 1 -PropertyType DWord -Force | Out-Null }
    New-ItemProperty $uninst "EstimatedSize" -Value $grootte -PropertyType DWord -Force | Out-Null

    # Verkenner laten weten dat de bestandstypen veranderd zijn (pictogrammen verversen)
    Add-Type -Namespace MathPrime -Name Shell -MemberDefinition '[DllImport("shell32.dll")] public static extern void SHChangeNotify(int e, int f, System.IntPtr a, System.IntPtr b);'
    [MathPrime.Shell]::SHChangeNotify(0x08000000, 0, [IntPtr]::Zero, [IntPtr]::Zero)

    if ($Stil) { Write-Output "MathPrime $versie geïnstalleerd in $Doel"; exit 0 }
    $antwoord = [System.Windows.Forms.MessageBox]::Show(
        "MathPrime $versie is geïnstalleerd.`n`nJe vindt het in het Startmenu en op het bureaublad; .rkn-bestanden openen met een dubbelklik. Verwijderen kan via Instellingen > Apps.`n`nMathPrime nu starten?",
        "MathPrime $versie", "YesNo", "Information")
    if ($antwoord -eq "Yes") { Start-Process $exe }
}
catch {
    Melding "De installatie is mislukt:`n$($_.Exception.Message)" "Error"
    exit 1
}
