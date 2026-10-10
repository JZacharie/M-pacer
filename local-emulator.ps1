#Requires -Version 7.0
<#
.SYNOPSIS
    Gere et lance les emulateurs Android virtuels pour tester M-pacer.

.DESCRIPTION
    Ce script pilote les deux appareils virtuels crees pour les tests :
      - Montre Wear OS : mpacer-wear (Wear OS 5, ecran rond)
      - Telephone Android : mpacer-phone (Pixel 6, Google APIs Android 14)

    Il permet de les demarrer en arriere-plan (avec ou sans interface graphique)
    et de verifier leur connexion sous adb.

.PARAMETER Target
    watch (defaut) : lance l'emulateur montre (mpacer-wear).
    phone          : lance l'emulateur telephone (mpacer-phone).
    both           : lance les deux emulateurs.
    list           : liste les AVD disponibles.
    status         : affiche les appareils connectes dans adb.

.PARAMETER Headless
    Lance l'emulateur sans fenetre graphique (-no-window), ideal pour les tests
    automatises en arriere-plan sans surcharger le GPU.

.EXAMPLE
    pwsh ./local-emulator.ps1 -Target watch
    pwsh ./local-emulator.ps1 -Target phone -Headless
    pwsh ./local-emulator.ps1 -Target status
#>
[CmdletBinding()]
param(
    [ValidateSet('watch', 'phone', 'both', 'list', 'status')]
    [string] $Target = 'watch',

    [switch] $Headless
)

$ErrorActionPreference = 'Stop'

function Get-EmulatorPath {
    $cmd = Get-Command emulator.exe -ErrorAction SilentlyContinue
    if ($cmd) { return $cmd.Source }
    $def = "$env:LOCALAPPDATA\Android\Sdk\emulator\emulator.exe"
    if (Test-Path $def) { return $def }
    throw "emulator.exe introuvable dans le PATH ni dans $def"
}

function Start-Avd([string] $name, [switch] $noWindow) {
    Write-Host "[info] Demarrage de l'AVD '$name'..." -ForegroundColor Cyan
    $emu = Get-EmulatorPath
    $arguments = @("-avd", $name)
    if ($noWindow) {
        $arguments += @("-no-window", "-no-audio", "-no-boot-anim")
    }

    Start-Process -FilePath $emu -ArgumentList $arguments
    Write-Host "[ok] Emulateur '$name' lance en arriere-plan." -ForegroundColor Green
}

switch ($Target) {
    'list' {
        Write-Host "=== Appareils virtuels AVD configures ===" -ForegroundColor Cyan
        avdmanager list avd
    }
    'status' {
        Write-Host "=== Appareils connectes sous ADB ===" -ForegroundColor Cyan
        adb devices -l
    }
    'watch' {
        Start-Avd "mpacer-wear" -noWindow:$Headless
    }
    'phone' {
        Start-Avd "mpacer-phone" -noWindow:$Headless
    }
    'both' {
        Start-Avd "mpacer-wear" -noWindow:$Headless
        Start-Avd "mpacer-phone" -noWindow:$Headless
    }
}
