#Requires -Version 7.0
<#
.SYNOPSIS
    Audite, sauvegarde et nettoie la montre Garmin de developpement M-pacer.

.DESCRIPTION
    Une montre Garmin branchee en USB se monte comme un lecteur contenant un
    dossier GARMIN (repere par GARMIN\GarminDevice.xml). Ce script :

      -Audit   (defaut) : modele, firmware, numero d'unite, occupation par dossier,
                          applications .prg installees, nombre de seances.
      -Backup  : copie les donnees personnelles (seances, monitoring, parcours,
                 sports, .prg) vers le PC AVANT tout nettoyage.
      -Clean   : supprime uniquement ce qui est explicitement demande :
                   * toujours : les fichiers de transit GARMIN\NEWFILES ;
                   * -RemoveOldApps : les .prg absents de -KeepApps ;
                   * -RemoveMusic   : musique et podcasts ;
                   * -ActivitiesOlderThanDays <n> : les seances de plus de n jours.
                 Sans aucune de ces options, -Clean ne touche ni aux applications
                 ni aux donnees : il ne vide que les fichiers de transit.

    -DryRun affiche ce qui serait fait sans rien supprimer. Aucune suppression
    n'a lieu si GARMIN\GarminDevice.xml est absent (garde-fou).

.PARAMETER WatchPath
    Racine du lecteur de la montre (ex. E:\). Detectee automatiquement si omise.

.PARAMETER Backup
    Copie les donnees vers -BackupDir (defaut : .\sauvegarde-garmin\<date>).

.PARAMETER Clean
    Applique le nettoyage decrit ci-dessus.

.PARAMETER RemoveOldApps
    Avec -Clean : supprime les applications .prg non listees dans -KeepApps.

.PARAMETER KeepApps
    Noms de fichiers .prg a conserver (defaut : mpacer.prg).

.PARAMETER RemoveMusic
    Avec -Clean : supprime musique et podcasts.

.PARAMETER ActivitiesOlderThanDays
    Avec -Clean : supprime les seances plus anciennes que ce nombre de jours.

.PARAMETER DryRun
    N'efface rien : affiche les actions.

.EXAMPLE
    pwsh ./local-garmin.ps1
    pwsh ./local-garmin.ps1 -Backup
    pwsh ./local-garmin.ps1 -Clean -DryRun
    pwsh ./local-garmin.ps1 -Clean -RemoveOldApps -KeepApps mpacer.prg
    pwsh ./local-garmin.ps1 -Clean -ActivitiesOlderThanDays 180
#>
[CmdletBinding()]
param(
    [string] $WatchPath,
    [switch] $Backup,
    [switch] $Clean,
    [switch] $RemoveOldApps,
    [string[]] $KeepApps = @('mpacer.prg'),
    [switch] $RemoveMusic,
    [int] $ActivitiesOlderThanDays = 0,
    [switch] $DryRun,
    [string] $BackupDir
)

$ErrorActionPreference = 'Stop'
$racine = $PSScriptRoot

function Etape([string] $texte) { Write-Host ''; Write-Host ('=== ' + $texte) -ForegroundColor Cyan }
function Ok([string] $texte) { Write-Host ('  [ok]      ' + $texte) -ForegroundColor Green }
function Info([string] $texte) { Write-Host ('  [info]    ' + $texte) -ForegroundColor Gray }
function Alerte([string] $texte) { Write-Host ('  [attention] ' + $texte) -ForegroundColor Yellow }
function Echec([string] $texte) { Write-Host ('  [echec]   ' + $texte) -ForegroundColor Red; exit 1 }

function Resolve-Montre {
    if ($WatchPath) {
        $garmin = Join-Path $WatchPath 'GARMIN'
        if (-not (Test-Path (Join-Path $garmin 'GarminDevice.xml'))) {
            Echec ('GARMIN\GarminDevice.xml introuvable sous ' + $WatchPath)
        }
        return @{ Racine = (Resolve-Path $WatchPath).Path; Garmin = (Resolve-Path $garmin).Path }
    }
    foreach ($lettre in (Get-PSDrive -PSProvider FileSystem -ErrorAction SilentlyContinue | Where-Object { $_.Root -match '^[A-Z]:\\$' })) {
        $g = Join-Path $lettre.Root 'GARMIN'
        if (Test-Path (Join-Path $g 'GarminDevice.xml')) {
            return @{ Racine = $lettre.Root; Garmin = (Resolve-Path $g).Path }
        }
    }
    Echec 'Aucune montre Garmin detectee. Branchez-la en USB, ou indiquez -WatchPath E:\'
}

function Get-Taille([string] $chemin) {
    if (-not (Test-Path $chemin)) { return $null }
    return (Get-ChildItem $chemin -Recurse -Force -File -ErrorAction SilentlyContinue | Measure-Object -Property Length -Sum).Sum
}

function Format-Taille($octets) {
    if ($null -eq $octets) { return 'absent' }
    if ($octets -ge 1GB) { return ('{0:N2} Go' -f ($octets / 1GB)) }
    if ($octets -ge 1MB) { return ('{0:N1} Mo' -f ($octets / 1MB)) }
    return ('{0:N0} Ko' -f ($octets / 1KB))
}

$montre = Resolve-Montre
$garmin = $montre.Garmin

function Show-Audit {
    Etape 'Montre'
    try {
        [xml] $xml = Get-Content (Join-Path $garmin 'GarminDevice.xml') -Raw
        $modele = $xml.Device.Model.Description
        $part = $xml.Device.Model.PartNumber
        $unite = $xml.Device.Id
        $fw = $null
        foreach ($p in $xml.SelectNodes('//*[local-name()="SoftwareVersion"]')) { if (-not $fw) { $fw = $p.InnerText } }
        Ok ('Modele : ' + $modele + ' (' + $part + ')')
        Info ('Numero d''unite : ' + $unite)
        Info ('Firmware : ' + $fw)
    } catch {
        Alerte ('GarminDevice.xml illisible : ' + $_.Exception.Message)
    }
    Info ('Lecteur : ' + $montre.Racine + '   GARMIN : ' + $garmin)

    Etape 'Occupation'
    $dossiers = 'ACTIVITY', 'MONITOR', 'APPS', 'MUSIC', 'PODCASTS', 'AUDIOBOOKS', 'COURSES', 'WORKOUTS', 'LOCATIONS', 'SPORTS', 'RECORDS', 'NEWFILES', 'REMOTESW', 'SETTINGS', 'TOTALS', 'SCREENSHOT'
    $total = 0
    foreach ($d in $dossiers) {
        $t = Get-Taille (Join-Path $garmin $d)
        if ($null -ne $t) {
            $nb = @(Get-ChildItem (Join-Path $garmin $d) -Recurse -File -ErrorAction SilentlyContinue).Count
            Write-Host ('    {0,-12} {1,12}   {2} fichier(s)' -f $d, (Format-Taille $t), $nb)
            $total += $t
        }
    }
    Write-Host ('    {0,-12} {1,12}' -f 'TOTAL', (Format-Taille $total)) -ForegroundColor Gray

    Etape 'Applications installees'
    $prg = @(Get-ChildItem (Join-Path $garmin 'APPS') -Filter '*.prg' -File -ErrorAction SilentlyContinue)
    if ($prg.Count -eq 0) { Info 'Aucune application .prg.' }
    foreach ($f in ($prg | Sort-Object Name)) { Write-Host ('    {0,-32} {1,10}   {2}' -f $f.Name, (Format-Taille $f.Length), $f.LastWriteTime.ToString('yyyy-MM-dd')) }

    Etape 'Seances'
    $activites = @(Get-ChildItem (Join-Path $garmin 'ACTIVITY') -File -Filter '*.fit' -ErrorAction SilentlyContinue)
    if ($activites.Count -eq 0) { Info 'Aucune seance.' }
    else {
        $min = ($activites | Sort-Object LastWriteTime | Select-Object -First 1).LastWriteTime
        $max = ($activites | Sort-Object LastWriteTime | Select-Object -Last 1).LastWriteTime
        Ok ("{0} seance(s), de {1} a {2}" -f $activites.Count, $min.ToString('yyyy-MM-dd'), $max.ToString('yyyy-MM-dd'))
    }
}

function Invoke-Backup {
    $dossier = $BackupDir
    if (-not $dossier) { $dossier = Join-Path $racine ('sauvegarde-garmin\' + (Get-Date -Format 'yyyy-MM-dd-HHmmss')) }
    New-Item -ItemType Directory -Force -Path $dossier | Out-Null
    Etape 'Sauvegarde'
    Info ('Destination : ' + (Resolve-Path $dossier).Path)
    $aCopier = 'ACTIVITY', 'MONITOR', 'RECORDS', 'COURSES', 'WORKOUTS', 'SPORTS', 'LOCATIONS'
    foreach ($d in $aCopier) {
        $src = Join-Path $garmin $d
        if (Test-Path $src) {
            $dst = Join-Path $dossier $d
            New-Item -ItemType Directory -Force -Path $dst | Out-Null
            Copy-Item (Join-Path $src '*') -Destination $dst -Recurse -Force -ErrorAction SilentlyContinue
            Ok ('{0} : {1}' -f $d, (Format-Taille (Get-Taille $dst)))
        }
    }
    $appsSrc = Join-Path $garmin 'APPS'
    if (Test-Path $appsSrc) {
        $dst = Join-Path $dossier 'APPS'
        New-Item -ItemType Directory -Force -Path $dst | Out-Null
        Copy-Item (Join-Path $appsSrc '*.prg') -Destination $dst -Force -ErrorAction SilentlyContinue
        Ok ('APPS : ' + (Format-Taille (Get-Taille $dst)))
    }
    $total = Get-Taille $dossier
    Ok ('Sauvegarde complete : ' + (Format-Taille $total))
}

function Remove-Element([string] $chemin, [string] $libelle) {
    if (-not (Test-Path $chemin)) { return }
    $taille = Get-Taille $chemin
    if ($DryRun) { Alerte ('[simulation] suppression de ' + $libelle + ' (' + (Format-Taille $taille) + ')'); return }
    Remove-Item $chemin -Recurse -Force -ErrorAction SilentlyContinue
    Ok ('supprime : ' + $libelle + ' (' + (Format-Taille $taille) + ')')
}

function Invoke-Clean {
    Etape 'Nettoyage'
    if (-not (Test-Path (Join-Path $garmin 'GarminDevice.xml'))) { Echec 'Garde-fou : GarminDevice.xml absent, aucune suppression.' }

    # Fichiers de transit : toujours sans danger, la montre les recree.
    $newFiles = Join-Path $garmin 'NEWFILES'
    if (Test-Path $newFiles) {
        $f = @(Get-ChildItem $newFiles -Recurse -File -ErrorAction SilentlyContinue)
        if ($f.Count -gt 0) { Remove-Element (Join-Path $newFiles '*') ('fichiers de transit NEWFILES (' + $f.Count + ')') }
        else { Info 'NEWFILES deja vide.' }
    }

    if ($RemoveOldApps) {
        $apps = Join-Path $garmin 'APPS'
        $cibles = @(Get-ChildItem $apps -Filter '*.prg' -File -ErrorAction SilentlyContinue | Where-Object { $KeepApps -notcontains $_.Name })
        if ($cibles.Count -eq 0) { Info 'Aucune application a retirer.' }
        foreach ($c in $cibles) {
            if ($DryRun) { Alerte ('[simulation] suppression de l''application ' + $c.Name) }
            else { Remove-Item $c.FullName -Force -ErrorAction SilentlyContinue; Ok ('application retiree : ' + $c.Name) }
        }
    }

    if ($RemoveMusic) {
        Remove-Element (Join-Path $garmin 'MUSIC') 'musique'
        Remove-Element (Join-Path $garmin 'PODCASTS') 'podcasts'
        Remove-Element (Join-Path $garmin 'AUDIOBOOKS') 'livres audio'
    }

    if ($ActivitiesOlderThanDays -gt 0) {
        $limite = (Get-Date).AddDays(-$ActivitiesOlderThanDays)
        $vieilles = @(Get-ChildItem (Join-Path $garmin 'ACTIVITY') -File -ErrorAction SilentlyContinue | Where-Object { $_.LastWriteTime -lt $limite })
        if ($vieilles.Count -eq 0) { Info ('Aucune seance de plus de ' + $ActivitiesOlderThanDays + ' jours.') }
        foreach ($v in $vieilles) {
            if ($DryRun) { Alerte ('[simulation] suppression de la seance ' + $v.Name + ' (' + $v.LastWriteTime.ToString('yyyy-MM-dd') + ')') }
            else { Remove-Item $v.FullName -Force -ErrorAction SilentlyContinue; Ok ('seance supprimee : ' + $v.Name) }
        }
    }

    Etape 'Occupation apres nettoyage'
    foreach ($d in 'ACTIVITY', 'APPS', 'MUSIC', 'NEWFILES') {
        Write-Host ('    {0,-12} {1,12}' -f $d, (Format-Taille (Get-Taille (Join-Path $garmin $d))))
    }
}

if ($Backup) { Invoke-Backup }
if ($Clean) { Invoke-Clean }
if (-not $Backup -and -not $Clean) { Show-Audit }
