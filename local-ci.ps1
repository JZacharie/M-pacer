#Requires -Version 7.0
<#
.SYNOPSIS
    Compile l'application montre M-pacer (et le module compagnon) en local.

.DESCRIPTION
    Chaine complete, sans CI :
      1. verification de l'environnement (JDK 17, SDK Android 35, NDK r27, cargo-ndk, cibles rustup)
      2. compilation du coeur Rust pour les trois ABI (arm64-v8a, armeabi-v7a, x86_64)
      3. assemblage Gradle du module :app (montre) et/ou :companion (telephone)
      4. installation sur une montre ou un emulateur connecte (-Install)

.PARAMETER Target
    watch (defaut) : la montre. companion : le telephone. all : les deux. rust : le coeur Rust seul.

.PARAMETER ApiUrl
    URL du backend inscrite dans l'APK (BuildConfig.DEFAULT_API_URL).
    Par defaut : http://10.0.2.2:8080 (l'hote vu depuis l'emulateur Android).

.PARAMETER CargoProfile
    Profil cargo du coeur Rust : release (defaut) ou debug.

.EXAMPLE
    pwsh ./local-ci.ps1 -Check
    pwsh ./local-ci.ps1
    pwsh ./local-ci.ps1 -Target all -ApiUrl http://192.168.0.152:8080 -Install
    pwsh ./local-ci.ps1 -Release -CargoProfile release -Test
#>
[CmdletBinding()]
param(
    [ValidateSet('watch', 'companion', 'all', 'rust')]
    [string] $Target = 'watch',

    [switch] $Release,
    [switch] $Clean,
    [switch] $Check,
    [switch] $Install,
    [switch] $SkipRust,
    [switch] $NoDaemon,
    [switch] $Test,

    [string] $ApiUrl,
    [ValidateSet('debug', 'release')]
    [string] $CargoProfile = 'release',

    [string] $AdbSerial,

    # Forcer le JDK a utiliser (ex. le JBR d'Android Studio)
    [string] $JavaHome,

    # Installer ce qui manque (cibles rustup, cargo-ndk, paquets du SDK Android)
    [switch] $Bootstrap
)

$ErrorActionPreference = 'Stop'
$script:JavaHomeForce = $JavaHome
$script:Manquants = 0
$racine = $PSScriptRoot
$dossierAndroid = Join-Path $racine 'android'
$dossierJniLibs = Join-Path $dossierAndroid 'app/src/main/jniLibs'
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

function Invoke-Gradle([string[]] $arguments) {
    Push-Location $dossierAndroid
    try {
        $gradlew = if ($IsWindows -or $env:OS -eq 'Windows_NT') { '.\gradlew.bat' } else { './gradlew' }
        if ($env:JAVA_HOME -and $env:JAVA_HOME -ne 'PATH') {
            $env:PATH = (Join-Path $env:JAVA_HOME 'bin') + [IO.Path]::PathSeparator + $env:PATH
        }
        Info ($gradlew + ' ' + ($arguments -join ' '))
        & $gradlew @arguments
        if ($LASTEXITCODE -ne 0) { throw ('Gradle a echoue (code ' + $LASTEXITCODE + ')') }
    } finally { Pop-Location }
}

# ---------------------------------------------------------------- 1. environnement

Etape 'Environnement'
$javaHome = Resolve-JavaHome
if ($javaHome -eq 'PATH') {
    $version = (& java -version 2>&1 | Select-Object -First 1)
    Ok ('JDK trouve dans le PATH : ' + $version)
} elseif ($javaHome) {
    $ok17 = Test-Path (Join-Path $javaHome 'bin/java.exe')
    if ($ok17) { Ok ('JDK : ' + $javaHome) } else { Echec ('JDK incomplet : ' + $javaHome) }
} else {
    Echec 'Aucun JDK 17+ detecte. Installez-le et definissez JAVA_HOME.'
    Info 'winget install Microsoft.OpenJDK.17   (ou Android Studio qui embarque un JBR)'
}

$sdk = Resolve-SdkAndroid
if ($sdk) {
    Ok ('SDK Android : ' + $sdk)
    $env:ANDROID_HOME = $sdk
    $env:ANDROID_SDK_ROOT = $sdk
    if (Test-Path (Join-Path $sdk 'platforms/android-35')) { Ok 'plateforme android-35 presente' }
    else { Echec 'plateforme android-35 absente'; Info ('sdkmanager "platforms;android-35"  (SDK : ' + $sdk + ')') }
    if (Test-Path (Join-Path $sdk 'build-tools')) { Ok 'build-tools presents' }
    else { Echec 'build-tools absents'; Info 'sdkmanager "build-tools;35.0.0"' }
} else {
    Echec 'SDK Android introuvable (ANDROID_HOME / ANDROID_SDK_ROOT).'
    Info 'Installez les command-line tools puis : sdkmanager "platform-tools" "platforms;android-35" "build-tools;35.0.0" "ndk;27.2.12479018" "cmake;3.22.1"'
}

$besoinRust = (-not $SkipRust) -and ($Target -in @('watch', 'all', 'rust'))
if ($besoinRust) {
    if (Test-Commande 'cargo') { Ok ('cargo : ' + (cargo --version)) }
    else { Echec 'cargo introuvable'; Info 'https://rustup.rs' }

    if (Test-Commande 'cargo') {
        $ndk = $null
        if ($sdk -and (Test-Path (Join-Path $sdk 'ndk'))) {
            $ndk = Get-ChildItem (Join-Path $sdk 'ndk') -Directory -ErrorAction SilentlyContinue |
                Sort-Object Name -Descending | Select-Object -First 1
        }
        if ($ndk) { Ok ('NDK : ' + $ndk.Name) }
        else { Echec 'NDK introuvable dans le SDK'; Info 'sdkmanager "ndk;27.2.12479018"' }

        $cibles = (& rustup target list --installed) -join ' '
        foreach ($cible in @('aarch64-linux-android', 'armv7-linux-androideabi', 'x86_64-linux-android')) {
            if ($cibles -match [regex]::Escape($cible)) { Ok ('cible rustup ' + $cible) }
            else { Echec ('cible rustup manquante : ' + $cible); Info ('rustup target add ' + $cible) }
        }

        $cargoNdk = ((& cargo ndk --version 2>&1) -join ' ').Trim()
        $cargoNdkAbsent = ($cargoNdk -match 'no such command') -or ($cargoNdk -match '^error')
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

    $sdkManager = $null
    if ($sdk) {
        $sdkManager = Get-ChildItem $sdk -Recurse -Depth 3 -Filter 'sdkmanager.bat' -ErrorAction SilentlyContinue |
            Select-Object -First 1
    }
    if ($sdkManager) {
        Info ('sdkmanager : ' + $sdkManager.FullName)
        1..40 | ForEach-Object { 'y' } | & $sdkManager.FullName --licenses | Out-Null
        & $sdkManager.FullName 'platform-tools' 'platforms;android-35' 'build-tools;35.0.0' 'ndk;27.2.12479018' 'cmake;3.22.1'
        if ($LASTEXITCODE -ne 0) { Alerte 'sdkmanager a retourne une erreur : relancez la commande a la main' }
        else { Ok 'paquets du SDK Android installes' }
    } else {
        Alerte 'sdkmanager introuvable : le SDK Android ne contient pas les command-line tools'
        Info 'Telechargez-les puis relancez -Bootstrap :'
        Info '  https://developer.android.com/studio#command-line-tools-only'
        Info '  puis sdkmanager "platform-tools" "platforms;android-35" "build-tools;35.0.0" "ndk;27.2.12479018" "cmake;3.22.1"'
    }
    Info 'Relancez le script sans -Bootstrap pour verifier.'
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

# ---------------------------------------------------------------- 2. nettoyage

if ($Clean) {
    Etape 'Nettoyage'
    foreach ($chemin in @(
        (Join-Path $dossierAndroid 'app/build'),
        (Join-Path $dossierAndroid 'companion/build'),
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
    'companion' { $taches += (':companion:' + $prefixe) }
    'all' { $taches += (':app:' + $prefixe); $taches += (':companion:' + $prefixe) }
    'rust' { Info 'cible rust : aucune tache Gradle' }
}

if ($taches.Count -gt 0) {
    Etape 'Assemblage Gradle'
    $arguments = $taches
    if ($rustCompile -or $SkipRust -or $Target -eq 'companion') { $arguments += '-Pmpacer.buildRust=false' }
    if ($CargoProfile) { $arguments += ('-Pmpacer.cargoProfile=' + $CargoProfile) }
    if ($ApiUrl) { $arguments += ('-Pmpacer.apiUrl=' + $ApiUrl) }
    if ($NoDaemon) { $arguments += '--no-daemon' }
    Invoke-Gradle $arguments

    Etape 'Artefacts'
    $apks = Get-ChildItem (Join-Path $dossierAndroid 'app/build/outputs/apk'), (Join-Path $dossierAndroid 'companion/build/outputs/apk') -Recurse -Filter '*.apk' -ErrorAction SilentlyContinue
    if (-not $apks) { Alerte 'aucun APK trouve' }
    foreach ($apk in $apks) {
        Ok ($apk.FullName.Replace($racine + '\', '') + '  ' + [math]::Round($apk.Length / 1MB, 1) + ' Mo')
    }
}

# ---------------------------------------------------------------- 6. installation

if ($Install) {
    Etape 'Installation sur la montre'
    $adb = @('adb')
    if ($AdbSerial) { $adb += @('-s', $AdbSerial) }
    $appareils = (& adb devices) -join ' '
    Info ('appareils : ' + ($appareils -replace 'List of devices attached', '').Trim())
    $apk = Get-ChildItem (Join-Path $dossierAndroid 'app/build/outputs/apk') -Recurse -Filter '*.apk' -ErrorAction SilentlyContinue |
        Where-Object { $_.Name -like '*debug*' } | Select-Object -First 1
    if (-not $apk) {
        $apk = Get-ChildItem (Join-Path $dossierAndroid 'app/build/outputs/apk') -Recurse -Filter '*.apk' -ErrorAction SilentlyContinue | Select-Object -First 1
    }
    if (-not $apk) { throw 'aucun APK a installer : lancez le script sans -Check' }
    & adb @('-s', $AdbSerial) install -r $apk.FullName 2>$null
    if ($LASTEXITCODE -ne 0) {
        & adb install -r $apk.FullName
        if ($LASTEXITCODE -ne 0) { throw 'installation adb refusee' }
    }
    Ok ('installe : ' + $apk.Name)
    if ($ApiUrl) {
        Info ('demarrage avec api_url=' + $ApiUrl)
        & adb shell am start -n com.mpacer.watch/.MainActivity --es api_url $ApiUrl | Out-Null
    }
}

Etape ('Termine en ' + [math]::Round($chrono.Elapsed.TotalSeconds, 1) + ' s')
Info 'Rappels : le module montre et le module compagnon doivent etre signes par la meme cle'
Info 'pour que le Data Layer Wear OS fonctionne (android/keystore.properties).'
