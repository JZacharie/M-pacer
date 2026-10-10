<#
.SYNOPSIS
    Construit (et verifie) l'application Garmin Connect IQ de M-pacer.

.DESCRIPTION
    Le script decouvre le SDK Connect IQ (installation locale ou telechargement
    direct depuis developer.garmin.com), genere une cle developpeur si besoin,
    puis compile le projet garmin/ avec monkeyc.

    Trois usages :

      pwsh ./garmin/build.ps1 -Check          # pre-vol : XML, appareils, compilation
      pwsh ./garmin/build.ps1 -Device fr965   # .prg pour une montre precise
      pwsh ./garmin/build.ps1 -Device fr965 -Install

    Les appareils (profils de compilation) ne sont pas fournis par l'archive du
    SDK : ils sont telecharges par le SDK Manager de Garmin (compte developpeur).
    Sans profil d'appareil, la compilation generale reste possible : elle valide
    la syntaxe, les types, les ressources et le manifeste - c'est ce que fait
    -Check en integration continue.

.PARAMETER Device
    Identifiant de l'appareil cible (ex. fr965, fenix7, venu3). Sans cette option,
    le premier appareil installe est utilise.

.PARAMETER SdkVersion
    Version du SDK a telecharger si aucun SDK n'est trouve (defaut : la derniere).

.PARAMETER SdkPath
    Racine d'un SDK deja installe (contient bin/monkeybrains.jar).

.PARAMETER Check
    Mode pre-vol : valide les fichiers, liste les appareils manquants et compile.

.PARAMETER Release
    Compilation release (-r : sans informations de debogage).

.PARAMETER Package
    Produit un paquet .iq signe, destine a la Connect IQ Store (monkeyc -e).

.PARAMETER Run
    Lance le simulateur Connect IQ puis l'application (monkeydo).

.PARAMETER Install
    Copie le .prg dans GARMIN/APPS de la montre connectee en USB.

.PARAMETER WatchPath
    Chemin de la montre (ex. E:\) si la detection automatique echoue.

.PARAMETER NoDownload
    Interdit le telechargement du SDK (echec explicite s'il est absent).

.PARAMETER Test
    Compile les tests unitaires Monkey C (tests.jungle, option -t) et les
    execute dans le simulateur quand un appareil est disponible.
#>
[CmdletBinding()]
param(
    [string]$Device,
    [string]$SdkVersion,
    [string]$SdkPath,
    [switch]$Check,
    [switch]$Release,
    [switch]$Package,
    [switch]$Run,
    [switch]$Install,
    [string]$WatchPath,
    [switch]$NoDownload,
    [switch]$Warnings,
    [switch]$Test
)

$ErrorActionPreference = 'Stop'
$ProjectRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$BuildDir = Join-Path $ProjectRoot 'build'
$ManifestPath = Join-Path $ProjectRoot 'manifest.xml'
$JunglePath = Join-Path $ProjectRoot 'monkey.jungle'
$KeyPath = Join-Path $ProjectRoot 'developer_key.der'

function Write-Step([string]$Text) { Write-Host "==> $Text" -ForegroundColor Cyan }
function Write-Ok([string]$Text) { Write-Host "    $Text" -ForegroundColor Green }
function Write-Note([string]$Text) { Write-Host "    $Text" -ForegroundColor Yellow }
# --- Version et date de compilation -----------------------------------------
# L'ecran Reglages de la montre affiche la version et le jour de compilation.
# Le numero vient du manifeste (le meme que celui envoye a la Connect IQ Store)
# et la date de SOURCE_DATE_EPOCH quand la chaine de construction la fournit
# (date de la release), sinon de l'horloge, en UTC - comme build.rs cote Rust
# et build.gradle.kts cote Android.
function Get-BuildDate {
    if ($env:MPACER_BUILD_DATE) { return $env:MPACER_BUILD_DATE.Trim() }
    if ($env:SOURCE_DATE_EPOCH) {
        $epoch = 0
        if ([long]::TryParse($env:SOURCE_DATE_EPOCH.Trim(), [ref] $epoch)) {
            return [DateTimeOffset]::FromUnixTimeSeconds($epoch).UtcDateTime.ToString('yyyy-MM-dd')
        }
    }
    return [DateTime]::UtcNow.ToString('yyyy-MM-dd')
}

function Get-ManifestVersion {
    [xml]$manifest = Get-Content -Raw $ManifestPath
    $manager = New-Object System.Xml.XmlNamespaceManager($manifest.NameTable)
    $manager.AddNamespace('iq', 'http://www.garmin.com/xml/connectiq')
    $node = $manifest.SelectSingleNode('//iq:application', $manager)
    if ($node -and $node.HasAttribute('version')) { return $node.GetAttribute('version') }
    return '0.0.0'
}

function New-VersionSource {
    param(
        [string]$OutputDirectory,
        [string]$Version,
        [string]$Date
    )
    New-Item -ItemType Directory -Force -Path $OutputDirectory | Out-Null
    $path = Join-Path $OutputDirectory 'MpacerBuildInfo.mc'
    $quotedVersion = [char]34 + $Version + [char]34
    $quotedDate = [char]34 + $Date + [char]34
    $ligneVersion = '        return ' + $quotedVersion + ';'
    $ligneDate = '        return ' + $quotedDate + ';'
    $lines = @(
        '// Fichier genere par garmin/build.ps1 : ne pas modifier a la main.',
        '// Version (manifeste) et jour de compilation, affiches dans les reglages.',
        'class MpacerBuildInfo {',
        '    static function version() {',
        $ligneVersion,
        '    }',
        '',
        '    static function buildDate() {',
        $ligneDate,
        '    }',
        '}',
        ''
    )
    Set-Content -Path $path -Value $lines -Encoding UTF8
    return $path
}


function Resolve-Java {
    $java = Get-Command java -ErrorAction SilentlyContinue
    if ($java) { return $java.Source }
    if ($IsWindows -or $null -eq $IsWindows) {
        $candidates = @(
            "$env:ProgramFiles\Android\Android Studio\jbr\bin\java.exe",
            "$env:LOCALAPPDATA\Programs\Android Studio\jbr\bin\java.exe",
            "$env:ProgramFiles\Eclipse Adoptium\jdk-17*\bin\java.exe",
            "$env:ProgramFiles\Java\jdk-17*\bin\java.exe"
        )
        foreach ($candidate in $candidates) {
            $found = Get-Item $candidate -ErrorAction SilentlyContinue | Select-Object -First 1
            if ($found) { return $found.FullName }
        }
    }
    throw "Java est introuvable. Installez un JRE/JDK 11+ (ou le JBR d'Android Studio) et relancez."
}

function Resolve-Sdk {
    param([switch]$AllowDownload)

    if ($SdkPath) {
        if (!(Test-Path (Join-Path $SdkPath 'bin'))) { throw "SDK invalide : $SdkPath (bin/ absent)." }
        return (Resolve-Path $SdkPath).Path
    }
    if ($env:CONNECT_IQ_SDK -and (Test-Path (Join-Path $env:CONNECT_IQ_SDK 'bin'))) {
        return (Resolve-Path $env:CONNECT_IQ_SDK).Path
    }
    $roots = @()
    if ($IsWindows -or $null -eq $IsWindows) {
        $roots += Join-Path $env:APPDATA 'Garmin\ConnectIQ\Sdks'
        $roots += Join-Path $env:LOCALAPPDATA 'Garmin\ConnectIQ\Sdks'
    } else {
        $roots += Join-Path $HOME '.Garmin/ConnectIQ/Sdks'
    }
    foreach ($root in $roots) {
        if (Test-Path $root) {
            $candidate = Get-ChildItem $root -Directory -ErrorAction SilentlyContinue |
                Where-Object { Test-Path (Join-Path $_.FullName 'bin') } |
                Sort-Object Name -Descending | Select-Object -First 1
            if ($candidate) { return $candidate.FullName }
        }
    }

    if (!$AllowDownload -or $NoDownload) {
        throw "Aucun SDK Connect IQ trouve. Installez-le avec le SDK Manager de Garmin, ou relancez sans -NoDownload (telechargement direct)."
    }

    $catalog = Invoke-RestMethod -Uri 'https://developer.garmin.com/downloads/connect-iq/sdks/sdks.json' -TimeoutSec 60
    $entry = $catalog | Where-Object { $_.version -eq $SdkVersion } | Select-Object -First 1
    if (!$entry) { $entry = $catalog | Select-Object -Last 1 }

    $platform = 'windows'
    if ($IsLinux) { $platform = 'linux' }
    if ($IsMacOS) { $platform = 'mac' }
    $fileName = $entry.$platform
    if (!$fileName) { throw "Le catalogue ne propose pas de SDK pour la plateforme $platform." }

    # RUNNER_TEMP (GitHub Actions) permet de mettre le SDK en cache entre deux jobs.
    $tempRoot = [System.IO.Path]::GetTempPath()
    if ($env:RUNNER_TEMP) { $tempRoot = $env:RUNNER_TEMP }
    $cacheRoot = Join-Path $tempRoot "mpacer-connectiq-$($entry.version)"
    $sdkDirectory = Join-Path $cacheRoot 'sdk'
    if (!(Test-Path (Join-Path $sdkDirectory 'bin'))) {
        New-Item -ItemType Directory -Force -Path $cacheRoot | Out-Null
        $archive = Join-Path $cacheRoot $fileName
        if (!(Test-Path $archive)) {
            Write-Step "Telechargement du SDK Connect IQ $($entry.version) ($platform)"
            Invoke-WebRequest -Uri "https://developer.garmin.com/downloads/connect-iq/sdks/$fileName" -OutFile $archive -TimeoutSec 3600
        }
        Write-Step "Extraction du SDK"
        Expand-Archive -Path $archive -DestinationPath $sdkDirectory -Force
    }
    return $sdkDirectory
}

function Get-DevicesDirectory {
    param([string]$Sdk)
    $candidates = @()
    if ($IsWindows -or $null -eq $IsWindows) {
        $candidates += Join-Path $env:APPDATA 'Garmin\ConnectIQ\Devices'
        $candidates += Join-Path $env:LOCALAPPDATA 'Garmin\ConnectIQ\Devices'
    } else {
        $candidates += Join-Path $HOME '.Garmin/ConnectIQ/Devices'
    }
    $candidates += Join-Path $Sdk 'Devices'
    foreach ($candidate in $candidates) {
        if (Test-Path $candidate) { return $candidate }
    }
    return $null
}

function Ensure-DeveloperKey {
    if (Test-Path $KeyPath) { return }
    Write-Step "Generation d'une cle developpeur RSA 4096 (developer_key.der)"
    $rsa = [System.Security.Cryptography.RSA]::Create(4096)
    try {
        [System.IO.File]::WriteAllBytes($KeyPath, $rsa.ExportPkcs8PrivateKey())
    } finally {
        $rsa.Dispose()
    }
    Write-Note "La cle est ignoree par git : conservez-la, elle signe les mises a jour."
}

function Get-ManifestProducts {
    [xml]$manifest = Get-Content -Raw $ManifestPath
    $manager = New-Object System.Xml.XmlNamespaceManager($manifest.NameTable)
    $manager.AddNamespace('iq', 'http://www.garmin.com/xml/connectiq')
    @($manifest.SelectNodes('//iq:product', $manager) | ForEach-Object { $_.id })
}

function New-BuildManifest {
    param([string]$DevicesDirectory)
    [xml]$manifest = Get-Content -Raw $ManifestPath
    $installed = Get-ChildItem $DevicesDirectory -Directory | Select-Object -ExpandProperty Name
    $manager = New-Object System.Xml.XmlNamespaceManager($manifest.NameTable)
    $manager.AddNamespace('iq', 'http://www.garmin.com/xml/connectiq')
    $products = $manifest.SelectSingleNode('//iq:products', $manager)
    $kept = @()
    foreach ($node in @($products.SelectNodes('iq:product', $manager))) {
        if ($installed -contains $node.id) { $kept += $node.id }
    }
    if ($kept.Count -eq 0) { return $ManifestPath }
    $products.RemoveAll()
    foreach ($id in $kept) {
        $node = $manifest.CreateElement('iq', 'product', 'http://www.garmin.com/xml/connectiq')
        $node.SetAttribute('id', $id)
        $products.AppendChild($node) | Out-Null
    }
    $path = Join-Path $BuildDir 'manifest.xml'
    $manifest.Save($path)
    return $path
}

Write-Step "Projet Garmin Connect IQ"
$sdk = Resolve-Sdk -AllowDownload
Write-Ok "SDK : $sdk"
$java = Resolve-Java
Write-Ok "Java : $java"

$devicesDirectory = Get-DevicesDirectory -Sdk $sdk
if ($devicesDirectory) {
    $installedDevices = @(Get-ChildItem $devicesDirectory -Directory | Select-Object -ExpandProperty Name | Sort-Object)
    Write-Ok "$($installedDevices.Count) appareil(s) disponible(s) dans $devicesDirectory"
} else {
    $installedDevices = @()
    Write-Note "Aucun profil d'appareil installe (dossier Devices absent)."
    Write-Note "Compilation de verification possible ; pour un .prg : SDK Manager > onglet Devices."
}

$manifestProducts = @(Get-ManifestProducts)
$missing = @($manifestProducts | Where-Object { $installedDevices -notcontains $_ })
if ($missing.Count -gt 0 -and $installedDevices.Count -gt 0) {
    Write-Note "$($missing.Count) appareil(s) du manifeste ne sont pas installes sur ce poste :"
    Write-Note ("   " + (($missing | Select-Object -First 12) -join ', '))
    Write-Note "   -> SDK Manager (onglet Devices), ou pwsh ./garmin/tools/sync-products.ps1"
}

New-Item -ItemType Directory -Force -Path $BuildDir | Out-Null
Ensure-DeveloperKey

$buildDate = Get-BuildDate
$appVersion = Get-ManifestVersion
$versionSource = New-VersionSource -OutputDirectory (Join-Path $BuildDir 'gen') -Version $appVersion -Date $buildDate
Write-Ok "Version $appVersion du $buildDate ($versionSource)"

$targetDevice = $Device
if (!$targetDevice -and $installedDevices.Count -gt 0) { $targetDevice = $installedDevices[0] }
if ($targetDevice -and $installedDevices.Count -gt 0 -and ($installedDevices -notcontains $targetDevice)) {
    throw "Appareil inconnu : $targetDevice (installez son profil dans le SDK Manager)."
}

$buildJungle = $JunglePath
if ($Test) { $buildJungle = Join-Path $ProjectRoot 'tests.jungle' }
if ($installedDevices.Count -gt 0 -and !$Package) {
    $buildManifest = New-BuildManifest -DevicesDirectory $devicesDirectory
    if ($buildManifest -ne $ManifestPath) {
        $buildJungle = Join-Path $BuildDir 'monkey.jungle'
        Set-Content -Path $buildJungle -Encoding UTF8 -Value @(
            'project.manifest = manifest.xml',
            'base.sourcePath = ../source',
            'base.resourcePath = ../resources'
        )
        Write-Ok "Manifeste de travail : build/manifest.xml ($($installedDevices.Count) appareils installes)"
    }
}

$monkeyc = Join-Path $sdk 'bin/monkeybrains.jar'
if (!(Test-Path $monkeyc)) { throw "monkeybrains.jar introuvable dans $sdk/bin." }

$output = Join-Path $BuildDir 'mpacer.prg'
if ($targetDevice) { $output = Join-Path $BuildDir "mpacer-$targetDevice.prg" }
if ($Package) { $output = Join-Path $BuildDir 'mpacer.iq' }
if ($Test) { $output = Join-Path $BuildDir 'mpacer-tests.prg' }

$arguments = @(
    '-Dfile.encoding=UTF-8',
    '-cp', $monkeyc,
    'com.garmin.monkeybrains.Monkeybrains',
    '-f', $buildJungle,
    '-o', $output,
    '-y', $KeyPath,
    '-l', '2'
)
if ($Warnings) { $arguments += '-w' }
if ($targetDevice) { $arguments += @('-d', $targetDevice) }
if ($Release) { $arguments += '-r' }
if ($Package) { $arguments += '-e' }
if ($Test) { $arguments += '-t' }

Write-Step "Compilation (monkeyc -l 2)"
# monkeyc resout les chemins du jungle relativement au dossier courant.
Push-Location $ProjectRoot
try {
    & $java @arguments
    $exitCode = $LASTEXITCODE
} finally {
    Pop-Location
}
if ($exitCode -ne 0) { throw "La compilation a echoue (code $exitCode)." }
if (!(Test-Path $output)) { throw "La compilation n'a produit aucun fichier : $output" }
Write-Ok "Fichier produit : $output ($([math]::Round((Get-Item $output).Length / 1KB, 0)) Ko)"

if ($Check) { Write-Ok "Pre-vol termine."; exit 0 }

if ($Test) {
    $monkeydo = Join-Path $sdk 'bin/monkeydo.bat'
    if (!(Test-Path $monkeydo)) { $monkeydo = Join-Path $sdk 'bin/monkeydo' }
    if (!$targetDevice) {
        Write-Note "Tests compiles. Installez un profil d'appareil (SDK Manager > Devices)"
        Write-Note "puis relancez avec -Device <id> pour les executer dans le simulateur."
        exit 0
    }
    $simulator = Join-Path $sdk 'bin/connectiq.bat'
    if (!(Test-Path $simulator)) { $simulator = Join-Path $sdk 'bin/connectiq' }
    Write-Step "Tests unitaires dans le simulateur (monkeydo -t)"
    Start-Process -FilePath $simulator
    Start-Sleep -Seconds 5
    & $monkeydo $output $targetDevice -t
    exit $LASTEXITCODE
}

if ($Run) {
    if (!$targetDevice) { throw "-Run exige -Device <appareil>." }
    $simulator = Join-Path $sdk 'bin/connectiq.bat'
    if (!(Test-Path $simulator)) { $simulator = Join-Path $sdk 'bin/connectiq' }
    Write-Step "Simulateur (connectiq puis monkeydo)"
    Start-Process -FilePath $simulator
    Start-Sleep -Seconds 5
    $monkeydo = Join-Path $sdk 'bin/monkeydo.bat'
    if (!(Test-Path $monkeydo)) { $monkeydo = Join-Path $sdk 'bin/monkeydo' }
    & $monkeydo $output $targetDevice
}

if ($Install) {
    if (!$targetDevice) { throw "-Install exige -Device <appareil>." }
    $watch = $WatchPath
    if (!$watch) {
        $watch = Get-PSDrive -PSProvider FileSystem |
            Where-Object { Test-Path (Join-Path $_.Root 'GARMIN\APPS') } |
            Select-Object -First 1 -ExpandProperty Root
    }
    if (!$watch) {
        throw "Montre introuvable. Branchez-la en USB puis indiquez -WatchPath E:\ (ou copiez $output dans GARMIN/APPS)."
    }
    $destination = Join-Path (Join-Path $watch 'GARMIN\APPS') 'MPACER.PRG'
    Copy-Item $output $destination -Force
    Write-Ok "Application copiee : $destination"
    Write-Note "Debranchez la montre : elle installe l'application au prochain demarrage."
}