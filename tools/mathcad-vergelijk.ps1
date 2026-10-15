<#
.SYNOPSIS
    Laat Mathcad Prime dezelfde bladen doorrekenen als MathPrime en zet de uitkomsten naast elkaar.

.DESCRIPTION
    Ontwikkelgereedschap, geen onderdeel van MathPrime zelf. Gebruikt alleen de officiële,
    gedocumenteerde Automation API van Mathcad Prime (COM "MathcadPrime.Application").

    1. MathPrime slaat elk .rkn-blad uit de corpusmap op als .mcdx (mathprime-cli export).
    2. Mathcad opent elk bestand, rekent het door (Synchronize) en slaat het op (SaveAs).
       Mathcad schrijft daarbij zijn eigen uitkomsten in result.xml.
    3. mathprime-cli vergelijk zet de uitkomsten van Mathcad naast die van MathPrime en schrijft
       een rapport.

    Een bestand dat Mathcad niet kan openen, betekent dat de .mcdx-schrijver van MathPrime iets
    schrijft wat Mathcad niet accepteert; dat staat ook in de uitvoer.

.EXAMPLE
    powershell -ExecutionPolicy Bypass -File tools\mathcad-vergelijk.ps1
#>
param(
    [string]$Corpus = (Join-Path $PSScriptRoot "..\tests\corpus"),
    [string]$Werk = (Join-Path $env:TEMP "mathprime-vergelijk"),
    [string]$Cli = (Join-Path $PSScriptRoot "..\target\release\mathprime-cli.exe")
)
$ErrorActionPreference = "Stop"
if (-not (Test-Path $Cli)) { throw "mathprime-cli niet gevonden: $Cli (eerst: cargo build --release -p rb-cli)" }

$naar = Join-Path $Werk "naar-mathcad"
$van = Join-Path $Werk "van-mathcad"
Remove-Item -Recurse -Force $Werk -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $naar, $van | Out-Null

# 1. MathPrime -> .mcdx
foreach ($f in Get-ChildItem $Corpus -Filter *.rkn | Sort-Object Name) {
    & $Cli export $f.FullName (Join-Path $naar ($f.BaseName + ".mcdx")) | Out-Null
}
Write-Host "MathPrime: $((Get-ChildItem $naar -Filter *.mcdx).Count) bladen opgeslagen als .mcdx"

# 2. Mathcad: openen, doorrekenen, opslaan
$app = New-Object -ComObject MathcadPrime.Application
try { $app.Visible = $false } catch { }
$mislukt = @()
try {
    foreach ($f in Get-ChildItem $naar -Filter *.mcdx | Sort-Object Name) {
        try {
            $ws = $app.Open($f.FullName)
            # zonder activeren en hervatten rekent Mathcad een via automation geopend blad niet door
            try { $ws.Activate() } catch { }
            try { $ws.ResumeCalculation() } catch { }
            $ws.Synchronize()
            Start-Sleep -Seconds 2
            $ws.Synchronize()
            $ws.SaveAs((Join-Path $van $f.Name))
            $ws.Close(2)   # spDiscardChanges: is al opgeslagen
            Write-Host "Mathcad: $($f.Name) doorgerekend"
        } catch {
            $mislukt += $f.Name
            Write-Host "Mathcad kon $($f.Name) niet openen of doorrekenen: $($_.Exception.Message)" -ForegroundColor Red
        }
    }
} finally {
    try { $app.Quit(2) } catch { }
    [System.Runtime.InteropServices.Marshal]::ReleaseComObject($app) | Out-Null
}

# 3. vergelijken
$rapport = Join-Path $Werk "rapport.md"
& $Cli vergelijk $van --rapport $rapport
if ($mislukt.Count -gt 0) { Write-Host "Niet geopend door Mathcad: $($mislukt -join ', ')" -ForegroundColor Red }
Write-Host "Rapport: $rapport"
