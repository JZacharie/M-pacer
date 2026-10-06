<#
.SYNOPSIS
    Pre-vol de l'application Garmin Connect IQ (integration continue).

.DESCRIPTION
    Verifie, sans rien installer :
      1. que les fichiers XML du projet (manifeste, ressources, reglages) sont
         bien formes ;
      2. que chaque propriete referencee par settings.xml existe dans
         properties.xml ;
      3. que chaque chaine Rez.Strings.* utilisee par le code est declaree ;
      4. que le projet compile (monkeyc) : syntaxe Monkey C, types (niveau 2),
         appels d'API et ressources.

.EXAMPLE
    pwsh ./garmin/preflight.ps1
    pwsh ./garmin/preflight.ps1 -SdkPath C:\ConnectIQ -NoDownload
#>
[CmdletBinding()]
param(
    [string]$SdkPath,
    [string]$SdkVersion,
    [switch]$NoDownload
)

$ErrorActionPreference = 'Stop'
$ProjectRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$failures = 0

function Test-Check([string]$Name, [bool]$Ok, [string]$Detail) {
    if ($Ok) {
        Write-Host "  [ok]   $Name" -ForegroundColor Green
    } else {
        Write-Host "  [FAIL] $Name $Detail" -ForegroundColor Red
        $script:failures++
    }
}

Write-Host "Pre-vol Connect IQ - $ProjectRoot" -ForegroundColor Cyan

$xmlFiles = @(
    'manifest.xml',
    'resources/strings/strings.xml',
    'resources/drawables/drawables.xml',
    'resources/settings/settings.xml',
    'resources/settings/properties.xml'
)
foreach ($relative in $xmlFiles) {
    $path = Join-Path $ProjectRoot $relative
    try {
        [xml](Get-Content -Raw $path) | Out-Null
        Test-Check $relative $true ''
    } catch {
        Test-Check $relative $false $_.Exception.Message
    }
}

try {
    [xml]$settings = Get-Content -Raw (Join-Path $ProjectRoot 'resources/settings/settings.xml')
    [xml]$properties = Get-Content -Raw (Join-Path $ProjectRoot 'resources/settings/properties.xml')
    $declared = @($properties.properties.property | ForEach-Object { $_.id })
    $missing = @()
    foreach ($setting in @($settings.settings.setting)) {
        $key = $setting.propertyKey
        if ($key -like '@Properties.*') {
            $name = $key.Substring('@Properties.'.Length)
            if ($declared -notcontains $name) { $missing += $name }
        }
    }
    Test-Check 'settings.xml -> properties.xml' ($missing.Count -eq 0) ("proprietes absentes : " + ($missing -join ', '))
} catch {
    Test-Check 'settings.xml -> properties.xml' $false $_.Exception.Message
}

try {
    [xml]$stringsXml = Get-Content -Raw (Join-Path $ProjectRoot 'resources/strings/strings.xml')
    $strings = @($stringsXml.resources.string | ForEach-Object { $_.id })
    $used = @(Get-ChildItem (Join-Path $ProjectRoot 'source') -Filter '*.mc' -Recurse |
        Select-String -Pattern 'Rez\.Strings\.([A-Za-z0-9_]+)' -AllMatches |
        ForEach-Object { $_.Matches } | ForEach-Object { $_.Groups[1].Value } | Sort-Object -Unique)
    $unknown = @($used | Where-Object { $strings -notcontains $_ })
    Test-Check 'Rez.Strings.* declarees' ($unknown.Count -eq 0) ("chaines absentes : " + ($unknown -join ', '))
} catch {
    Test-Check 'Rez.Strings.* declarees' $false $_.Exception.Message
}

Test-Check 'source/MpacerApp.mc' (Test-Path (Join-Path $ProjectRoot 'source/MpacerApp.mc')) ''
Test-Check 'monkey.jungle' ((Get-Content -Raw (Join-Path $ProjectRoot 'monkey.jungle')) -match 'project\.manifest') ''

Write-Host "  ... compilation monkeyc" -ForegroundColor Cyan
$buildArguments = @{ Check = $true }
if ($SdkPath) { $buildArguments.SdkPath = $SdkPath }
if ($SdkVersion) { $buildArguments.SdkVersion = $SdkVersion }
if ($NoDownload) { $buildArguments.NoDownload = $true }
try {
    & (Join-Path $ProjectRoot 'build.ps1') @buildArguments | Out-Host
    Test-Check 'compilation monkeyc' ($LASTEXITCODE -eq 0 -or $null -eq $LASTEXITCODE) ''
} catch {
    Test-Check 'compilation monkeyc' $false $_.Exception.Message
}

Write-Host "  ... compilation des tests unitaires" -ForegroundColor Cyan
$testArguments = @{ Check = $true; Test = $true }
if ($SdkPath) { $testArguments.SdkPath = $SdkPath }
if ($SdkVersion) { $testArguments.SdkVersion = $SdkVersion }
if ($NoDownload) { $testArguments.NoDownload = $true }
try {
    & (Join-Path $ProjectRoot 'build.ps1') @testArguments | Out-Host
    Test-Check 'compilation des tests' ($LASTEXITCODE -eq 0 -or $null -eq $LASTEXITCODE) ''
} catch {
    Test-Check 'compilation des tests' $false $_.Exception.Message
}

if ($failures -gt 0) {
    Write-Host "Pre-vol : $failures controle(s) en echec." -ForegroundColor Red
    exit 1
}
Write-Host "Pre-vol : tout est vert." -ForegroundColor Green