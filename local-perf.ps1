#Requires -Version 7.0
<#
.SYNOPSIS
    Optimisations de vitesse de build qui exigent les droits administrateur.

.DESCRIPTION
    Ces deux reglages ne peuvent pas etre appliques par un processus non eleve,
    mais ils pesent lourd sur les temps de compilation :

      1. Plan d'alimentation "Haute performance" : en mode "Utilisation normale",
         le processeur baisse ses frequences pendant les longues compilations.
      2. Exclusions Windows Defender : l'antivirus analyse chaque fichier lu et
         ecrit par cargo, Gradle, le NDK et le SDK Android ; les exclure accelere
         nettement les compilations et les tests.

    Le script verifie qu'il est bien eleve, affiche ce qu'il va faire, et n'agit
    qu'avec -Appliquer (sinon il se contente d'un rapport). -Revert annule.

.PARAMETER Appliquer
    Applique reellement les changements (sans ce commutateur : simulation).

.PARAMETER Revert
    Retablit le plan "Utilisation normale" et retire les exclusions ajoutees.

.PARAMETER Defender
    Ne traiter que les exclusions Defender.

.PARAMETER PowerPlan
    Ne traiter que le plan d'alimentation.

.PARAMETER Plan
    GUID du plan d'alimentation a activer (defaut : Haute performance).

.EXAMPLE
    # Dans un terminal PowerShell ouvert en administrateur :
    pwsh ./local-perf.ps1
    pwsh ./local-perf.ps1 -Appliquer
    pwsh ./local-perf.ps1 -Revert -Appliquer
#>
[CmdletBinding()]
param(
    [switch] $Appliquer,
    [switch] $Revert,
    [switch] $Defender,
    [switch] $PowerPlan,
    [string] $Plan = '8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c',
    [string] $PlanNormal = '381b4222-f694-41f0-9685-ff5bb260df2e'
)

$ErrorActionPreference = 'Stop'
$racine = $PSScriptRoot

function Etape([string] $texte) { Write-Host ''; Write-Host ('=== ' + $texte) -ForegroundColor Cyan }
function Ok([string] $texte) { Write-Host ('  [ok]      ' + $texte) -ForegroundColor Green }
function Info([string] $texte) { Write-Host ('  [info]    ' + $texte) -ForegroundColor Gray }
function Alerte([string] $texte) { Write-Host ('  [attention] ' + $texte) -ForegroundColor Yellow }
function Echec([string] $texte) { Write-Host ('  [echec]   ' + $texte) -ForegroundColor Red; exit 1 }

$principal = New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
$estAdmin = $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
if (-not $estAdmin) {
    Alerte 'Ce script doit etre lance dans un terminal PowerShell ouvert en administrateur.'
    Write-Host ''
    Write-Host '  Demarrer > "PowerShell" > clic droit > Executer en tant qu administrateur' -ForegroundColor Gray
    Write-Host ('  puis : pwsh "' + (Join-Path $racine 'local-perf.ps1') + '" -Appliquer') -ForegroundColor Gray
    exit 1
}
Ok 'Processus eleve.'

$faireDefender = $Defender -or (-not $PowerPlan -and -not $Defender)
$fairePlan = $PowerPlan -or (-not $PowerPlan -and -not $Defender)
if ($PowerPlan -and -not $Defender) { $faireDefender = $false }
if ($Defender -and -not $PowerPlan) { $fairePlan = $false }

if ($fairePlan) {
    Etape 'Plan d''alimentation'
    $actif = (powercfg /getactivescheme) -join ' '
    Info ('Actuel : ' + $actif.Trim())
    if ($Revert) {
        if ($Appliquer) { powercfg /setactive $PlanNormal | Out-Null; Ok 'Plan "Utilisation normale" retabli.' }
        else { Alerte ('[simulation] powercfg /setactive ' + $PlanNormal) }
    } else {
        if ($Appliquer) { powercfg /setactive $Plan | Out-Null; Ok ('Plan active : ' + ((powercfg /getactivescheme) -join ' ').Trim()) }
        else { Alerte ('[simulation] powercfg /setactive ' + $Plan + '  (Haute performance)') }
    }
}

if ($faireDefender) {
    Etape 'Exclusions Windows Defender'
    $chemins = @(
        $racine,
        (Join-Path $racine 'target'),
        (Join-Path $racine 'android\build'),
        (Join-Path $env:USERPROFILE '.cargo'),
        (Join-Path $env:USERPROFILE '.gradle'),
        (Join-Path $env:USERPROFILE '.rustup'),
        (Join-Path $env:LOCALAPPDATA 'Android\Sdk'),
        (Join-Path $env:LOCALAPPDATA 'Garmin\ConnectIQ')
    )
    $existants = @()
    try { $existants = @((Get-MpPreference).ExclusionPath) } catch { $existants = @() }

    foreach ($c in $chemins) {
        if (-not (Test-Path $c)) { Info ('ignore (absent) : ' + $c); continue }
        if ($existants -contains $c) { Ok ('deja exclu : ' + $c); continue }
        if ($Revert) {
            if ($Appliquer) { Remove-MpPreference -ExclusionPath $c -ErrorAction SilentlyContinue; Ok ('exclusion retiree : ' + $c) }
            else { Alerte ('[simulation] retrait de l exclusion : ' + $c) }
        } else {
            if ($Appliquer) { Add-MpPreference -ExclusionPath $c -ErrorAction SilentlyContinue; Ok ('exclu : ' + $c) }
            else { Alerte ('[simulation] exclusion : ' + $c) }
        }
    }
}

Etape 'Etat final'
Info ('Plan d''alimentation : ' + ((powercfg /getactivescheme) -join ' ').Trim())
try {
    $ex = @((Get-MpPreference).ExclusionPath)
    Info ('Exclusions Defender : ' + $ex.Count + ' chemin(s)')
    foreach ($e in $ex) { Write-Host ('    ' + $e) }
} catch { Alerte 'Exclusions illisibles.' }

if (-not $Appliquer) {
    Write-Host ''
    Alerte 'Simulation uniquement : relancez avec -Appliquer pour agir.'
}
