#Requires -Version 7.0
<#
.SYNOPSIS
    Compile l'application montre M-pacer (et le module compagnon) en local.

.DESCRIPTION
    Chaine complete, sans CI :
      1. verification de l'environnement (JDK 17, SDK Android 35, NDK r27, cargo-ndk, cibles rustup)
      2. compilation du coeur Rust pour les trois ABI (arm64-v8a, armeabi-v7a, x86_64)
      3. assemblage Gradle des modules :core (socle), :app (montre), :phone
         (courir avec le telephone) et :companion (appoint)
      4. installation sur une montre ou un emulateur connecte (-Install)

    La date de compilation (-BuildDate, defaut : le jour courant en UTC) est
    exportee dans MPACER_BUILD_DATE : les APK l'affichent dans Reglages > Version,
    et garmin/build.ps1 la lit s'il est lance depuis le meme terminal.

.PARAMETER Target
    watch (defaut) : la montre. phone : l'application de course du telephone.
    companion : l'application d'appoint. all : les trois. rust : le coeur Rust seul.

.PARAMETER ApiUrl
    URL du backend inscrite dans l'APK (BuildConfig.DEFAULT_API_URL).
    Par defaut : http://10.0.2.2:8080 (l'hote vu depuis l'emulateur Android).

.PARAMETER BuildDate
    Jour de compilation inscrit dans les APK (AAAA-MM-JJ). Une publication y met
    la date de son tag ; en local, le jour courant en UTC convient.

.PARAMETER CargoProfile
    Profil cargo du coeur Rust : release (defaut) ou debug.

.EXAMPLE
    pwsh ./local-ci.ps1 -Check
    pwsh ./local-ci.ps1
    pwsh ./local-ci.ps1 -KeepAwake            # garde la montre allumee sur son chargeur (dev)
    pwsh ./local-ci.ps1 -RestoreSleep         # retablit la veille normale
    pwsh ./local-ci.ps1 -Target all -ApiUrl http://192.168.0.152:8080 -Install
    pwsh ./local-ci.ps1 -Release -CargoProfile release -Test
    pwsh ./local-ci.ps1 -Release -BuildDate 2026-10-10   # meme date dans les trois APK
#>
[CmdletBinding()]
param(
    [ValidateSet('watch', 'phone', 'companion', 'all', 'rust')]
    [string] $Target = 'watch',

    [switch] $Release,
    [switch] $Clean,
    [switch] $Check,
    [switch] $Install,
    [switch] $SkipRust,
    [switch] $NoDaemon,
    [switch] $Test,

    [string] $ApiUrl,

    # Jour de compilation inscrit dans les APK (Reglages > Version). Par defaut,
    # le jour courant en UTC ; une publication reprend la date de son tag.
    [string] $BuildDate,

    [ValidateSet('debug', 'release')]
    [string] $CargoProfile = 'release',

    [string] $AdbSerial,

    # Forcer le JDK a utiliser (ex. le JBR d'Android Studio)
    [string] $JavaHome,

    # Installer ce qui manque (cibles rustup, cargo-ndk, paquets du SDK Android)
    [switch] $Bootstrap,

    # Garder la montre eveillee tant qu'elle est sur son chargeur (dev)
    [switch] $KeepAwake,

    # Retablir la veille normale de la montre
    [switch] $RestoreSleep
)

$ErrorActionPreference = 'Stop'
$script:JavaHomeForce = $JavaHome
$script:Manquants = 0
$script:PlateformeOk = $false
$script:NdkTrouve = $false
$script:CargoNdkAbsent = $true
$script:CiblesManquantes = @()
$script:CmakeOk = $false
$racine = $PSScriptRoot
$dossierAndroid = Join-Path $racine 'android'
$dossierJniLibs = Join-Path $dossierAndroid 'core/src/main/jniLibs'
$chrono = [System.Diagnostics.Stopwatch]::StartNew()

function Etape([string] $texte) {
    Write-Host ''
    Write-Host ('=== ' + $texte) -ForegroundColor Cyan
}
function Ok([string] $texte) { Write-Host ('  [ok]      ' + $texte) -ForegroundColor Green }
function Info([string] $texte) { Write-Host ('  [info]    ' + $texte) -ForegroundColor Gray }
function Alerte([string] $texte) { Write-Host ('  [attention] ' + $texte) -ForegroundColor Yellow }
function Echec([string] $texte) {
    Write-Host ('  [manquant] ' + $texte) -ForegroundColor Red
    $script:Manquants++
}

function Test-Commande([string] $nom) {
    return [bool] (Get-Command $nom -ErrorAction SilentlyContinue)
}

function Resolve-SdkAndroid {
    foreach ($candidat in @($env:ANDROID_HOME, $env:ANDROID_SDK_ROOT, (Join-Path $env:LOCALAPPDATA 'Android\Sdk'))) {
        if ($candidat -and (Test-Path (Join-Path $candidat 'platform-tools'))) { return $candidat }
    }
    if ($env:ANDROID_HOME -and (Test-Path $env:ANDROID_HOME)) { return $env:ANDROID_HOME }
    return $null
}

function Test-JavaHome([string] $chemin) {
    if (-not $chemin) { return $false }
    return (Test-Path (Join-Path $chemin 'bin/java.exe')) -or (Test-Path (Join-Path $chemin 'bin/java'))
}

function Resolve-JavaHome {
    if ($script:JavaHomeForce) {
        if (Test-JavaHome $script:JavaHomeForce) { return $script:JavaHomeForce }
        Echec ('JDK indique introuvable : ' + $script:JavaHomeForce)
    }
    if (Test-JavaHome $env:JAVA_HOME) { return $env:JAVA_HOME }

    # Racines a explorer : le dossier lui-meme peut etre un JDK (JBR d'Android Studio),
    # sinon on cherche un sous-dossier contenant bin/java.
    $racines = @(
        'C:\Program Files\Android\Android Studio\jbr',
        'C:\Program Files\Android\Android Studio\jre',
        'C:\Program Files\Android\Android Studio1\jbr',
        "$env:LOCALAPPDATA\Programs\Android Studio\jbr",
        'C:\Program Files\Eclipse Adoptium',
        'C:\Program Files\Microsoft\jdk',
        'C:\Program Files\Java',
        'C:\Program Files\Amazon Corretto',
        "$env:USERPROFILE\.jdks",
        'C:\Program Files\JetBrains'
    )
    foreach ($racine in $racines) {
        if (Test-JavaHome $racine) { return $racine }
        if (Test-Path $racine) {
            $trouve = Get-ChildItem $racine -Directory -ErrorAction SilentlyContinue |
                Where-Object { Test-JavaHome $_.FullName } |
                Sort-Object Name -Descending | Select-Object -First 1
            if ($trouve) { return $trouve.FullName }
        }
    }
    if (Test-Commande 'java') { return 'PATH' }
    return $null
}

function Get-UrlCmdlineTools {
    # L'index officiel donne l'archive la plus recente : indispensable, car une
    # version ancienne ne comprend pas les fichiers XML du depot actuel (v4) et
    # sdkmanager n'installe alors rien (avertissement "SDK XML versions up to 3").
    $index = 'https://dl.google.com/android/repository/repository2-3.xml'
    try {
        $xml = [xml] (Invoke-WebRequest -Uri $index -UseBasicParsing -TimeoutSec 60).Content
        $paquet = @($xml.sdkRepository.remotePackage) | Where-Object { $_.path -eq 'cmdline-tools;latest' } | Select-Object -First 1
        if (-not $paquet) { return $null }
        $archives = @($paquet.archives.archive)
        $windows = $archives | Where-Object { $_.hostOs -eq 'windows' -and $_.hostArch -eq 'x86_64' } | Select-Object -First 1
        if (-not $windows) { $windows = $archives | Where-Object { $_.hostOs -eq 'windows' } | Select-Object -First 1 }
        if (-not $windows) { return $null }
        $url = [string] $windows.complete.url
        if ($url -notmatch '^https?://') { $url = 'https://dl.google.com/android/repository/' + $url }
        return $url
    } catch {
        Alerte ('index SDK illisible : ' + $_.Exception.Message)
        return $null
    }
}

function Resolve-Appareil([string] $adb, [string] $serieForcee) {
    # La montre peut apparaitre deux fois (adresse IP et nom mDNS) : on choisit
    # l'entree IP:port quand elle est unique, sinon -AdbSerial est obligatoire.
    $lignes = & $adb devices 2>&1
    # @(...) est indispensable : avec un seul appareil, le pipeline renvoie une
    # chaine scalaire, et $series[0] donnerait alors son PREMIER CARACTERE
    # (l'installation partait sur le serial "a" et adb repondait "device 'a' not found").
    $series = @($lignes | Where-Object { $_ -match '^\S+\s+device$' } | ForEach-Object { ($_ -split '\s+')[0] })
    if ($serieForcee) { return $serieForcee }
    if ($series.Count -eq 0) { return $null }
    $ip = @($series | Where-Object { $_ -match '^\d+\.\d+\.\d+\.\d+:\d+$' })
    if ($ip.Count -eq 1) { Info ('appareil selectionne : ' + $ip[0]); return $ip[0] }
    if ($series.Count -eq 1) { return $series[0] }
    Echec ('plusieurs appareils connectes : precisez -AdbSerial (disponibles : ' + ($series -join ', ') + ')')
    return $null
}

function Resolve-Adb {
    if ($script:AdbForce) { return $script:AdbForce }
    $surPath = Get-Command 'adb' -ErrorAction SilentlyContinue
    if ($surPath) { return $surPath.Source }
    if ($sdk) {
        foreach ($nom in @('adb.exe', 'adb')) {
            $candidat = Join-Path $sdk ('platform-tools/' + $nom)
            if (Test-Path $candidat) { return $candidat }
        }
    }
    return $null
}

function Install-CmdlineTools([string] $sdk) {
    # Le SDK Android ne contient pas forcement sdkmanager : on recupere les
    # command-line tools officiels, puis on les place dans cmdline-tools/latest.
    $urls = @()
    $recente = Get-UrlCmdlineTools
    if ($recente) { $urls += $recente; Info ('archive la plus recente : ' + $recente) }
    $urls += @(
        'https://dl.google.com/android/repository/commandlinetools-win-13114758_latest.zip',
        'https://dl.google.com/android/repository/commandlinetools-win-11076708_latest.zip'
    )
    $zip = Join-Path $env:TEMP 'mpacer-cmdline-tools.zip'
    $extraction = Join-Path $env:TEMP 'mpacer-cmdline-tools-extract'
    foreach ($url in $urls) {
        Info ('telechargement des command-line tools : ' + $url)
        try {
            Invoke-WebRequest -Uri $url -OutFile $zip -UseBasicParsing -ErrorAction Stop
        } catch {
            Alerte ('telechargement impossible : ' + $_.Exception.Message)
            continue
        }
        if ((Get-Item $zip).Length -lt 1MB) { Alerte 'archive suspecte (trop petite), essai suivant'; continue }
        if (Test-Path $extraction) { Remove-Item -Recurse -Force $extraction }
        Expand-Archive -Path $zip -DestinationPath $extraction -Force
        $dest = Join-Path $sdk 'cmdline-tools/latest'
        New-Item -ItemType Directory -Force -Path $dest | Out-Null
        Copy-Item (Join-Path $extraction 'cmdline-tools/*') $dest -Recurse -Force
        $sdkmanager = Join-Path $dest 'bin/sdkmanager.bat'
        if (Test-Path $sdkmanager) { Ok ('command-line tools installes : ' + $dest); return $sdkmanager }
        Alerte 'sdkmanager.bat absent apres extraction'
    }
    return $null
}

function Invoke-Gradle([string[]] $arguments) {
    Push-Location $dossierAndroid
    try {
        $gradlew = if ($IsWindows -or $env:OS -eq 'Windows_NT') { '.\gradlew.bat' } else { './gradlew' }
        $jh = $script:JavaHomeTrouve
        if (-not $jh) { $jh = $env:JAVA_HOME }
        if ($jh -and $jh -ne 'PATH') {
            $env:JAVA_HOME = $jh
            $env:PATH = (Join-Path $jh 'bin') + [IO.Path]::PathSeparator + $env:PATH
            Info ('JAVA_HOME = ' + $jh)
        } elseif (-not $env:JAVA_HOME) {
            Alerte 'JAVA_HOME non defini : Gradle risque de refuser de demarrer'
        }
        Info ($gradlew + ' ' + ($arguments -join ' '))
        & $gradlew @arguments
        if ($LASTEXITCODE -ne 0) { throw ('Gradle a echoue (code ' + $LASTEXITCODE + ')') }
    } finally { Pop-Location }
}

# ---------------------------------------------------------------- 1. environnement

Etape 'Environnement'
# Date de compilation des APK : Gradle la lit dans MPACER_BUILD_DATE, et
# garmin/build.ps1 dans le meme environnement. Elle est fixee une seule fois,
# ici, pour que les artefacts d'une meme execution portent la meme date, et
# exportee pour les processus enfants.
$env:MPACER_BUILD_DATE = if ($BuildDate) { $BuildDate } else { [DateTime]::UtcNow.ToString('yyyy-MM-dd') }
Info ('date de compilation : ' + $env:MPACER_BUILD_DATE)

$javaHome = Resolve-JavaHome
$script:JavaHomeTrouve = $null
if ($javaHome -eq 'PATH') {
    $version = (& java -version 2>&1 | Select-Object -First 1)
    Ok ('JDK trouve dans le PATH : ' + $version)
} elseif ($javaHome) {
    $ok17 = (Test-Path (Join-Path $javaHome 'bin/java.exe')) -or (Test-Path (Join-Path $javaHome 'bin/java'))
    if ($ok17) {
        Ok ('JDK : ' + $javaHome)
        # Gradle (et sdkmanager) exigent JAVA_HOME : on l'exporte pour tout le script.
        $env:JAVA_HOME = $javaHome
        $script:JavaHomeTrouve = $javaHome
    } else { Echec ('JDK incomplet : ' + $javaHome) }
} else {
    Echec 'Aucun JDK 17+ detecte. Installez-le et definissez JAVA_HOME.'
    Info 'winget install Microsoft.OpenJDK.17   (ou Android Studio qui embarque un JBR)'
}

$sdk = Resolve-SdkAndroid
if ($sdk) {
    Ok ('SDK Android : ' + $sdk)
    $env:ANDROID_HOME = $sdk
    $env:ANDROID_SDK_ROOT = $sdk

    # local.properties est la maniere standard d'indiquer le SDK : cela permet aussi
    # d'appeler gradlew directement, sans ANDROID_HOME dans l'environnement.
    $localProperties = Join-Path $dossierAndroid 'local.properties'
    # Dans un fichier .properties, l'antislash est un caractere d'echappement :
    # le chemin doit utiliser des barres obliques, sinon Gradle voit un chemin invalide.
    $attendu = 'sdk.dir=' + $sdk.Replace('\', '/')
    $actuel = if (Test-Path $localProperties) { Get-Content $localProperties -Raw } else { '' }
    if ($actuel -notmatch [regex]::Escape($attendu)) {
        Set-Content -Path $localProperties -Value $attendu -Encoding ASCII
        Info ('android/local.properties ecrit : ' + $attendu)
    }
    if (Test-Path (Join-Path $sdk 'platforms/android-35')) { Ok 'plateforme android-35 presente'; $script:PlateformeOk = $true }
    else { Echec 'plateforme android-35 absente'; Info ('sdkmanager "platforms;android-35"  (SDK : ' + $sdk + ')') }
    if (Test-Path (Join-Path $sdk 'build-tools')) { Ok 'build-tools presents' }
    else { Echec 'build-tools absents'; Info 'sdkmanager "build-tools;35.0.0"' }
} else {
    Echec 'SDK Android introuvable (ANDROID_HOME / ANDROID_SDK_ROOT).'
    Info 'Installez les command-line tools puis : sdkmanager "platform-tools" "platforms;android-35" "build-tools;35.0.0" "ndk;27.2.12479018" "cmake;3.22.1"'
}

$besoinRust = (-not $SkipRust) -and ($Target -in @('watch', 'phone', 'all', 'rust'))
if ($besoinRust) {
    if (Test-Commande 'cargo') { Ok ('cargo : ' + (cargo --version)) }
    else { Echec 'cargo introuvable'; Info 'https://rustup.rs' }

    if (Test-Commande 'cargo') {
        $ndk = $null
        if ($sdk -and (Test-Path (Join-Path $sdk 'ndk'))) {
            $ndk = Get-ChildItem (Join-Path $sdk 'ndk') -Directory -ErrorAction SilentlyContinue |
                Sort-Object Name -Descending | Select-Object -First 1
        }
        if ($ndk) { Ok ('NDK : ' + $ndk.Name); $script:NdkTrouve = $true }
        else { Echec 'NDK introuvable dans le SDK'; Info 'sdkmanager "ndk;27.2.12479018"' }

        if ($sdk -and (Test-Path (Join-Path $sdk 'cmake'))) { $script:CmakeOk = $true }

        $cibles = (& rustup target list --installed) -join ' '
        foreach ($cible in @('aarch64-linux-android', 'armv7-linux-androideabi', 'x86_64-linux-android')) {
            if ($cibles -match [regex]::Escape($cible)) { Ok ('cible rustup ' + $cible) }
            else {
                Echec ('cible rustup manquante : ' + $cible)
                Info ('rustup target add ' + $cible)
                $script:CiblesManquantes += $cible
            }
        }

        $cargoNdk = ((& cargo ndk --version 2>&1) -join ' ').Trim()
        $cargoNdkAbsent = ($cargoNdk -match 'no such command') -or ($cargoNdk -match '^error')
        $script:CargoNdkAbsent = $cargoNdkAbsent
        if (-not $cargoNdkAbsent) { Ok ('cargo-ndk : ' + $cargoNdk) }
        else { Echec 'cargo-ndk introuvable'; Info 'cargo install cargo-ndk   (ou relancez avec -Bootstrap)' }
    }
} else {
    Info 'etape Rust ignoree (-SkipRust) : les .so deja presents seront reutilises'
}

$gradleWrapper = Join-Path $dossierAndroid 'gradlew.bat'
if (Test-Path $gradleWrapper) { Ok 'wrapper Gradle present (android/gradlew.bat)' }
else { Echec 'wrapper Gradle absent (android/gradlew.bat)' }

if ($Install) {
    if (Test-Commande 'adb') { Ok 'adb present' } else { Echec 'adb introuvable (platform-tools)'; }
}

# ---------------------------------------------------------------- 1 bis. installation

if ($Bootstrap) {
    Etape 'Installation des dependances manquantes (-Bootstrap)'

    if (Test-Commande 'rustup') {
        foreach ($cible in @('aarch64-linux-android', 'armv7-linux-androideabi', 'x86_64-linux-android')) {
            Info ('rustup target add ' + $cible)
            & rustup target add $cible
        }
    } else {
        Alerte 'rustup introuvable : installez Rust depuis https://rustup.rs'
    }

    if (Test-Commande 'cargo') {
        $etat = ((& cargo ndk --version 2>&1) -join ' ')
        if ($etat -match 'no such command') {
            Info 'cargo install cargo-ndk'
            & cargo install cargo-ndk
        } else { Ok 'cargo-ndk deja installe' }
    }

    # Chemin de sdkmanager, toujours sous forme de chaine (Install-CmdlineTools renvoie un chemin)
    $sdkManagerPath = $null
    if ($sdk) {
        $trouve = Get-ChildItem $sdk -Recurse -Depth 3 -Filter 'sdkmanager.bat' -ErrorAction SilentlyContinue |
            Select-Object -First 1
        if ($trouve) { $sdkManagerPath = $trouve.FullName }
        if (-not $sdkManagerPath) { $sdkManagerPath = Install-CmdlineTools $sdk }
    }
    if ($sdkManagerPath) {
        if (-not $env:JAVA_HOME -and $javaHome -and $javaHome -ne 'PATH') { $env:JAVA_HOME = $javaHome }
        Info ('sdkmanager : ' + $sdkManagerPath)
        1..40 | ForEach-Object { 'y' } | & $sdkManagerPath --licenses | Out-Null
        Info 'installation de platform-tools, android-35, build-tools 35, NDK 27 et cmake 3.22.1'
        & $sdkManagerPath 'platform-tools' 'platforms;android-35' 'build-tools;35.0.0' 'ndk;27.2.12479018' 'cmake;3.22.1'
        if ($LASTEXITCODE -ne 0) { Alerte 'sdkmanager a retourne une erreur : relancez la commande a la main' }
        else {
            $plateforme = Test-Path (Join-Path $sdk 'platforms/android-35')
            $ndk = Test-Path (Join-Path $sdk 'ndk')
            if ($plateforme -and $ndk) { Ok 'paquets du SDK Android installes (android-35 + NDK)' }
            else {
                Alerte 'sdkmanager s''est termine sans tout installer (android-35 / NDK absents)'
                Info 'Cause probable : command-line tools trop anciens pour le depot actuel.'
                Info 'Corrigez avec Android Studio (SDK Manager) ou une version recente des command-line tools.'
            }
        }
    } else {
        Alerte 'sdkmanager introuvable et telechargement impossible'
        Info 'Installez les command-line tools a la main :'
        Info '  https://developer.android.com/studio#command-line-tools-only'
        Info '  puis sdkmanager "platform-tools" "platforms;android-35" "build-tools;35.0.0" "ndk;27.2.12479018" "cmake;3.22.1"'
    }
    Info 'Relancez le script sans -Bootstrap pour verifier.'
}

# ---------------------------------------------------------------- 1 quater. montre

if ($KeepAwake -or $RestoreSleep) {
    Etape $(if ($KeepAwake) { 'Montre maintenue eveillee sur son chargeur' } else { 'Retablissement de la veille de la montre' })
    $adb = Resolve-Adb
    if (-not $adb) {
        Echec 'adb introuvable (platform-tools) : impossible de configurer la montre'
    } else {
        $serie = Resolve-Appareil $adb $AdbSerial
        if (-not $serie) {
            Echec 'aucune montre connectee (adb devices) : branchez-la ou connectez-la en sans fil'
        } else {
            $argsAdb = @('-s', $serie)
            $modele = ((& $adb @argsAdb shell getprop ro.product.model 2>&1) -join ' ').Trim()
            Ok ('montre detectee : ' + $modele + ' (' + $serie + ')')
            if ($KeepAwake) {
                # Ecran jamais eteint + maintien en veille active sur tous les types de chargeur
                & $adb @argsAdb shell settings put system screen_off_timeout 2147483647
                & $adb @argsAdb shell settings put global stay_on_while_plugged_in 7
                & $adb @argsAdb shell svc power stayon true
                & $adb @argsAdb shell input keyevent 224
                Info ('screen_off_timeout = ' + ((& $adb @argsAdb shell settings get system screen_off_timeout) -join ''))
                Info ('stay_on_while_plugged_in = ' + ((& $adb @argsAdb shell settings get global stay_on_while_plugged_in) -join ''))
                Info 'la montre reste affichee tant qu elle est sur son chargeur'
            } else {
                & $adb @argsAdb shell settings put system screen_off_timeout 15000
                & $adb @argsAdb shell settings delete global stay_on_while_plugged_in | Out-Null
                & $adb @argsAdb shell svc power stayon false | Out-Null
                Info 'veille normale retablie (15 s)'
            }
        }
    }
}

Etape ('Resume : cible=' + $Target + ' profil=' + $CargoProfile + ' build=' + $(if ($Release) { 'release' } else { 'debug' }))
if ($Check) {
    Info ('Verification demandee (-Check) : arret avant compilation. Duree ' + [math]::Round($chrono.Elapsed.TotalSeconds, 1) + ' s')
    if ($script:Manquants -gt 0) {
        Alerte ($script:Manquants.ToString() + ' prerequis manquant(s) : corrigez-les (ou lancez -Bootstrap) puis relancez -Check')
        exit 1
    }
    Ok 'tous les prerequis sont presents'
    exit 0
}

# -KeepAwake / -RestoreSleep seuls : on s'arrete apres avoir configure la montre.
if (($KeepAwake -or $RestoreSleep) -and (-not $PSBoundParameters.ContainsKey('Target'))) {
    Info 'configuration de la montre terminee (aucune compilation demandee)'
    exit 0
}

# ---------------------------------------------------------------- 1 ter. controle bloquant

if (-not $Check) {
    $bloquants = @()
    if (-not $javaHome) { $bloquants += 'JDK 17+ (JAVA_HOME ou -JavaHome)' }
    if (-not $sdk) { $bloquants += 'SDK Android (ANDROID_HOME)' }
    if ((-not $script:PlateformeOk) -and ($Target -ne 'rust')) { $bloquants += 'plateforme android-35' }
    if ($besoinRust) {
        if (-not (Test-Commande 'cargo')) { $bloquants += 'cargo (https://rustup.rs)' }
        if (-not $script:NdkTrouve) { $bloquants += 'NDK (sdkmanager "ndk;27.2.12479018")' }
        if ($script:CargoNdkAbsent) { $bloquants += 'cargo-ndk (cargo install cargo-ndk)' }
        foreach ($cible in $script:CiblesManquantes) { $bloquants += ('cible rustup ' + $cible) }
    }
    if ($bloquants.Count -gt 0) {
        Etape 'Compilation interrompue'
        foreach ($item in $bloquants) { Echec $item }
        Alerte 'Corrigez ces prerequis, ou lancez : pwsh ./local-ci.ps1 -Bootstrap'
        Info 'Diagnostic seul a tout moment : pwsh ./local-ci.ps1 -Check'
        exit 1
    }
    Ok 'prerequis bloquants : tous presents'
}

# ---------------------------------------------------------------- 1 quater. montre

# ---------------------------------------------------------------- 2. nettoyage

if ($Clean) {
    Etape 'Nettoyage'
    foreach ($chemin in @(
        (Join-Path $dossierAndroid 'app/build'),
        (Join-Path $dossierAndroid 'phone/build'),
        (Join-Path $dossierAndroid 'companion/build'),
        (Join-Path $dossierAndroid 'core/build'),
        (Join-Path $dossierAndroid 'build'),
        (Join-Path $dossierAndroid '.gradle'),
        $dossierJniLibs
    )) {
        if (Test-Path $chemin) { Remove-Item -Recurse -Force $chemin; Info ('supprime : ' + $chemin) }
    }
}

# ---------------------------------------------------------------- 3. tests Rust (option)

if ($Test) {
    Etape 'Tests du coeur Rust'
    Push-Location $racine
    try {
        & cargo test --workspace
        if ($LASTEXITCODE -ne 0) { throw 'les tests Rust ont echoue' }
    } finally { Pop-Location }
}

# ---------------------------------------------------------------- 4. coeur Rust

$rustCompile = $false
if ($besoinRust -and $Target -ne 'rust') {
    Etape 'Coeur Rust (cargo-ndk)'
    New-Item -ItemType Directory -Force -Path $dossierJniLibs | Out-Null
    $arguments = @('ndk', '-t', 'arm64-v8a', '-t', 'armeabi-v7a', '-t', 'x86_64',
                   '-o', $dossierJniLibs, 'build')
    if ($CargoProfile -eq 'release') { $arguments += '--release' }
    $arguments += @('-p', 'mpacer-ffi')
    Push-Location $racine
    try {
        Info ('cargo ' + ($arguments -join ' '))
        & cargo @arguments
        if ($LASTEXITCODE -ne 0) { throw ('cargo ndk a echoue (code ' + $LASTEXITCODE + ')') }
        $rustCompile = $true
    } finally { Pop-Location }

    Get-ChildItem $dossierJniLibs -Recurse -Filter '*.so' -ErrorAction SilentlyContinue |
        ForEach-Object { Info ('  ' + $_.FullName.Replace($racine + '\', '') + '  ' + [math]::Round($_.Length / 1KB) + ' Ko') }
}

# ---------------------------------------------------------------- 5. Gradle

$taches = @()
$prefixe = if ($Release) { 'assembleRelease' } else { 'assembleDebug' }
switch ($Target) {
    'watch' { $taches += (':app:' + $prefixe) }
    'phone' { $taches += (':phone:' + $prefixe) }
    'companion' { $taches += (':companion:' + $prefixe) }
    'all' {
        $taches += (':app:' + $prefixe)
        $taches += (':phone:' + $prefixe)
        $taches += (':companion:' + $prefixe)
    }
    'rust' { Info 'cible rust : aucune tache Gradle' }
}

if ($taches.Count -gt 0) {
    Etape 'Assemblage Gradle'
    $arguments = $taches
    if ($rustCompile -or $SkipRust -or ($Target -in @('phone', 'companion'))) { $arguments += '-Pmpacer.buildRust=false' }
    if ($CargoProfile) { $arguments += ('-Pmpacer.cargoProfile=' + $CargoProfile) }
    if ($ApiUrl) { $arguments += ('-Pmpacer.apiUrl=' + $ApiUrl) }
    if ($NoDaemon) { $arguments += '--no-daemon' }
    Invoke-Gradle $arguments

    Etape 'Artefacts'
    $dossiers = @()
    if ($Target -in @('watch', 'all')) { $dossiers += (Join-Path $dossierAndroid 'app/build/outputs/apk') }
    if ($Target -in @('phone', 'all')) { $dossiers += (Join-Path $dossierAndroid 'phone/build/outputs/apk') }
    if ($Target -in @('companion', 'all')) { $dossiers += (Join-Path $dossierAndroid 'companion/build/outputs/apk') }
    $apks = Get-ChildItem $dossiers -Recurse -Filter '*.apk' -ErrorAction SilentlyContinue
    if (-not $apks) { Alerte 'aucun APK trouve' }
    foreach ($apk in $apks) {
        Ok ($apk.FullName.Replace($racine + '\', '') + '  ' + [math]::Round($apk.Length / 1MB, 1) + ' Mo')
    }
}

# ---------------------------------------------------------------- 6. installation

if ($Install) {
    Etape 'Installation sur la montre'
    $adbExe = Resolve-Adb
    if (-not $adbExe) { throw 'adb introuvable (platform-tools) : impossible d installer' }
    $serie = Resolve-Appareil $adbExe $AdbSerial
    if (-not $serie) { throw 'aucune montre connectee (adb devices)' }
    $argsAdb = @('-s', $serie)
    Info ('appareil : ' + $serie)
    $apk = Get-ChildItem (Join-Path $dossierAndroid 'app/build/outputs/apk') -Recurse -Filter '*.apk' -ErrorAction SilentlyContinue |
        Where-Object { $_.Name -like '*debug*' } | Select-Object -First 1
    if (-not $apk) {
        $apk = Get-ChildItem (Join-Path $dossierAndroid 'app/build/outputs/apk') -Recurse -Filter '*.apk' -ErrorAction SilentlyContinue | Select-Object -First 1
    }
    if (-not $apk) { throw 'aucun APK a installer : lancez le script sans -Check' }
    & $adbExe @argsAdb install -r $apk.FullName
    if ($LASTEXITCODE -ne 0) { throw 'installation adb refusee' }
    Ok ('installe : ' + $apk.Name)
    if ($ApiUrl) {
        Info ('demarrage avec api_url=' + $ApiUrl)
        & $adbExe @argsAdb shell am start -n com.mpacer.watch/.MainActivity --es api_url $ApiUrl | Out-Null
    }
}

Etape ('Termine en ' + [math]::Round($chrono.Elapsed.TotalSeconds, 1) + ' s')
Info 'Rappels : le module montre et le module compagnon doivent etre signes par la meme cle'
Info 'pour que le Data Layer Wear OS fonctionne (android/keystore.properties).'
