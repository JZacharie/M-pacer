#Requires -Version 7.0
<#
.SYNOPSIS
    Nettoie et audite la montre Wear OS de developpement M-pacer (Galaxy Watch).

.DESCRIPTION
    L'application montre est com.mpacer.watch. Ce script se connecte a la montre
    via adb (USB ou debogage Wi-Fi) et propose trois usages :

      -Audit   (defaut) : lecture seule. Modele, version d'Android, espace libre,
                         applications tierces, paquets desactives, caches, RAM,
                         batterie, echelle des animations.
      -Backup  : copie l'archive locale des seances de la montre vers le PC avant
                 tout nettoyage (recommandee).
      -Clean   : libere les caches, coupe les animations (montre plus reactive),
                         arrete les applications en arriere-plan, et peut
                         recompiler les applications pour accelerer leur lancement.

    Aucune suppression d'application ni de donnee n'est faite sans -Uninstall
    explicite : le nettoyage par defaut ne detruit rien d'irreversible.

.PARAMETER Audit
    Affiche l'etat de la montre (lecture seule). Action par defaut.

.PARAMETER Backup
    Copie /data/data/com.mpacer.watch/files (archive des seances) dans le dossier
    indique par -BackupDir (defaut : .\sauvegarde-montre\<date>).

.PARAMETER Clean
    Applique le nettoyage : caches, animations a 0.5, arret des applications,
    TRIM du stockage.

.PARAMETER Compile
    Avec -Clean : recompile les applications en mode speed (AOT). Lancement plus
    rapide, au prix de quelques centaines de Mo. Reversible par une mise a jour.

.PARAMETER Uninstall
    Liste de paquets a desinstaller (uniquement ceux que vous nommez). Rien n'est
    supprime automatiquement.

.PARAMETER RestoreAnimations
    Remet les animations a l'echelle normale (1.0).

.PARAMETER BackupDir
    Dossier de sauvegarde (defaut : .\sauvegarde-montre\<horodatage>).

.PARAMETER Serial
    Numero de serie adb si plusieurs appareils sont connectes.

.EXAMPLE
    pwsh ./local-watch.ps1
    pwsh ./local-watch.ps1 -Backup
    pwsh ./local-watch.ps1 -Clean
    pwsh ./local-watch.ps1 -Uninstall com.exemple.jeu,com.exemple.demo
    pwsh ./local-watch.ps1 -RestoreAnimations
#>
[CmdletBinding()]
param(
    [switch] $Audit,
    [switch] $Backup,
    [switch] $Clean,
    [switch] $Compile,
    [switch] $RestoreAnimations,
    [string[]] $Uninstall,
    [string] $BackupDir,
    [string] $Serial,
    [string] $Package = 'com.mpacer.watch'
)

$ErrorActionPreference = 'Stop'
$racine = $PSScriptRoot

function Etape([string] $texte) { Write-Host ''; Write-Host ('=== ' + $texte) -ForegroundColor Cyan }
function Ok([string] $texte) { Write-Host ('  [ok]      ' + $texte) -ForegroundColor Green }
function Info([string] $texte) { Write-Host ('  [info]    ' + $texte) -ForegroundColor Gray }
function Alerte([string] $texte) { Write-Host ('  [attention] ' + $texte) -ForegroundColor Yellow }
function Echec([string] $texte) { Write-Host ('  [echec]   ' + $texte) -ForegroundColor Red; exit 1 }

function Resolve-Adb {
    $cmd = Get-Command adb -ErrorAction SilentlyContinue
    if ($cmd) { return $cmd.Source }
    $candidat = Join-Path $env:LOCALAPPDATA 'Android\Sdk\platform-tools\adb.exe'
    if (Test-Path $candidat) { return $candidat }
    Echec 'adb introuvable (installez les platform-tools du SDK Android).'
}

$script:Adb = Resolve-Adb
$script:Args = @()
if ($Serial) { $script:Args += @('-s', $Serial) }

function Adb([string[]] $arguments) {
    $tout = $script:Args + $arguments
    return (& $script:Adb @tout 2>&1)
}

function Sh([string] $commande) {
    $sortie = Adb @('shell', $commande)
    return ($sortie -join [char]10)
}

function Test-Appareil {
    Etape 'Appareil'
    $liste = (Adb @('devices')) -join [char]10
    if ($liste -notmatch '\tdevice') {
        Alerte 'Aucune montre en etat "device" :'
        Write-Host ('    ' + (($liste -split [char]10 | Where-Object { $_ -and $_ -notmatch '^List' }) -join ' / '))
        Alerte 'Activez le debogage Wi-Fi (Parametres > Developpeur > Debogage sans fil) puis :'
        Write-Host '    adb pair <ip>:<port>      # premiere fois, avec le code affiche'
        Write-Host '    adb connect <ip>:<port>'
        exit 2
    }
    $modele = (Sh 'getprop ro.product.model').Trim()
    $version = (Sh 'getprop ro.build.version.release').Trim()
    $build = (Sh 'getprop ro.build.display.id').Trim()
    Ok ('Montre : ' + $modele + ' - Android ' + $version + ' - build ' + $build)
}

function Show-Audit {
    Etape 'Stockage'
    Sh 'df -h /data /storage/emulated 2>/dev/null'

    Etape 'Applications tierces installees'
    $tierces = @(Sh 'pm list packages -3' | Select-String -Pattern '^package:' | ForEach-Object { $_.Line.Replace('package:', '') })
    if ($tierces.Count -eq 0) { Info 'Aucune application tierce.' }
    foreach ($p in ($tierces | Sort-Object)) {
        $taille = (Sh ('dumpsys diskstats 2>/dev/null | grep -m1 ' + $p)).Trim()
        Write-Host ('    ' + $p)
    }

    Etape 'Paquets desactives'
    Sh 'pm list packages -d' | Select-Object -First 25

    Etape 'Applications les plus gourmandes (memoire)'
    Sh 'dumpsys meminfo 2>/dev/null | head -18'

    Etape 'Caches'
    Sh 'du -sh /data/data/*/cache 2>/dev/null | sort -h | tail -12'

    Etape 'Animations (0.5 = plus reactif)'
    foreach ($k in 'window_animation_scale', 'transition_animation_scale', 'animator_duration_scale') {
        $v = (Sh ('settings get global ' + $k)).Trim()
        Write-Host ('    ' + $k + ' = ' + $v)
    }

    Etape 'Batterie'
    Sh 'dumpsys battery 2>/dev/null | head -12'

    Etape 'Sante du systeme'
    Sh 'uptime'
    Sh 'getprop ro.build.characteristics'
}

function Invoke-Backup {
    $dossier = $BackupDir
    if (-not $dossier) { $dossier = Join-Path $racine ('sauvegarde-montre\' + (Get-Date -Format 'yyyy-MM-dd-HHmmss')) }
    if (-not (Test-Path $dossier)) { New-Item -ItemType Directory -Force -Path $dossier | Out-Null }
    Etape 'Sauvegarde de l''archive des seances'
    Info ('Dossier : ' + (Resolve-Path $dossier).Path)
    $distant = '/sdcard/mpacer-sauvegarde'
    Adb @('shell', 'rm -rf ' + $distant) | Out-Null
    $copie = Sh ('run-as ' + $Package + ' sh -c "mkdir -p ' + $distant + ' && cp -r files ' + $distant + '/ 2>/dev/null"')
    if ($copie) { Write-Host ('    ' + $copie) }
    $pull = Adb @('pull', ($distant + '/files'), $dossier)
    Write-Host ('    ' + (($pull -join ' ') -replace '\s+', ' '))
    Adb @('shell', 'rm -rf ' + $distant) | Out-Null
    $fichiers = @(Get-ChildItem $dossier -Recurse -File -ErrorAction SilentlyContinue)
    if ($fichiers.Count -gt 0) {
        $taille = ($fichiers | Measure-Object -Property Length -Sum).Sum
        Ok ('{0} fichier(s), {1:N1} Ko sauvegardes.' -f $fichiers.Count, ($taille / 1KB))
    } else {
        Alerte 'Rien n''a ete recupere (application non deboguable ou archive vide).'
        Info 'Alternative : adb backup -f sauvegarde.ab ' + $Package
    }
}

function Invoke-Clean {
    Etape 'Nettoyage'
    Info 'Liberation des caches de toutes les applications...'
    Adb @('shell', 'pm trim-caches 8G') | Out-Null
    Sh 'sync'
    Ok 'Caches liberes.'

    Info 'Animations a 0.5 (interface plus reactive)...'
    foreach ($k in 'window_animation_scale', 'transition_animation_scale', 'animator_duration_scale') {
        Sh ('settings put global ' + $k + ' 0.5') | Out-Null
    }
    Ok 'Animations reglees.'

    Info 'Arret des applications en arriere-plan...'
    Adb @('shell', 'am kill-all') | Out-Null
    Ok 'Applications arretees.'

    if ($Compile) {
        Info 'Recompilation des applications (AOT, lancement plus rapide)...'
        Sh 'cmd package compile -m speed -a'
        Ok 'Recompilation terminee.'
    }

    Etape 'Espace apres nettoyage'
    Sh 'df -h /data'
}

if ($Uninstall -and $Uninstall.Count -gt 0) {
    Test-Appareil
    Etape 'Desinstallation (paquets nommes explicitement)'
    foreach ($p in $Uninstall) {
        Alerte ('Desinstallation de ' + $p)
        Adb @('uninstall', $p)
    }
}

if ($RestoreAnimations) {
    Test-Appareil
    Etape 'Restauration des animations'
    foreach ($k in 'window_animation_scale', 'transition_animation_scale', 'animator_duration_scale') {
        Sh ('settings put global ' + $k + ' 1.0') | Out-Null
    }
    Ok 'Animations remises a 1.0.'
}

if ($Backup) { Test-Appareil; Invoke-Backup }
if ($Clean) { Test-Appareil; Invoke-Clean }

if (-not $Backup -and -not $Clean -and -not $RestoreAnimations -and (-not $Uninstall -or $Uninstall.Count -eq 0)) {
    Test-Appareil
    Show-Audit
}
