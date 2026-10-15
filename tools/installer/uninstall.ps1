<#
.SYNOPSIS
    Verwijdert MathPrime (de installatie van install.ps1). Je instellingen (%APPDATA%\MathPrime) en je
    eigen werkbladen blijven staan.
#>
param([switch]$Stil)
$ErrorActionPreference = "Stop"
$map = $PSScriptRoot
Add-Type -AssemblyName System.Windows.Forms

function Melding([string]$tekst, [string]$soort = "Information") {
    if ($Stil) { Write-Output $tekst; return }
    [void][System.Windows.Forms.MessageBox]::Show($tekst, "MathPrime verwijderen", "OK", $soort)
}

try {
    $exe = Join-Path $map "mathprime.exe"
    if (Get-Process mathprime -ErrorAction SilentlyContinue | Where-Object { $_.Path -eq $exe }) {
        Melding "MathPrime is nog geopend. Sla je werk op, sluit MathPrime en probeer het opnieuw." "Warning"
        exit 1
    }
    if (-not $Stil) {
        $ok = [System.Windows.Forms.MessageBox]::Show("MathPrime verwijderen? Je werkbladen en instellingen blijven bewaard.", "MathPrime verwijderen", "YesNo", "Question")
        if ($ok -ne "Yes") { exit 0 }
    }

    foreach ($lnk in (Join-Path ([Environment]::GetFolderPath("Programs")) "MathPrime.lnk"),
                     (Join-Path ([Environment]::GetFolderPath("Desktop")) "MathPrime.lnk")) {
        if (Test-Path $lnk) { Remove-Item $lnk -Force }
    }

    $classes = "HKCU:\Software\Classes"
    if (Test-Path "$classes\MathPrime.Werkblad") { Remove-Item "$classes\MathPrime.Werkblad" -Recurse -Force }
    # .rkn alleen weghalen als het nog naar MathPrime wijst
    if ((Get-ItemProperty "$classes\.rkn" -ErrorAction SilentlyContinue)."(default)" -eq "MathPrime.Werkblad") {
        Remove-Item "$classes\.rkn" -Recurse -Force
    }
    if (Test-Path "$classes\.mcdx\OpenWithProgids") {
        Remove-ItemProperty "$classes\.mcdx\OpenWithProgids" "MathPrime.Werkblad" -ErrorAction SilentlyContinue
    }

    $pad = [Environment]::GetEnvironmentVariable("Path", "User")
    $rest = @($pad -split ";" | Where-Object { $_ -ne "" -and $_ -ne $map })
    [Environment]::SetEnvironmentVariable("Path", ($rest -join ";"), "User")

    $uninst = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\MathPrime"
    if (Test-Path $uninst) { Remove-Item $uninst -Recurse -Force }

    Set-Location $env:TEMP
    Remove-Item $map -Recurse -Force
    Melding "MathPrime is verwijderd."
}
catch {
    Melding "Verwijderen is niet helemaal gelukt:`n$($_.Exception.Message)" "Error"
    exit 1
}
