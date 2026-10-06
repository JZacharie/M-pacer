<#
.SYNOPSIS
    Reecrit la liste <iq:products> du manifeste a partir des appareils installes.

.DESCRIPTION
    Les identifiants d'appareils sont les noms des dossiers du SDK Connect IQ
    (Devices/<id>). Cette commande aligne le manifeste du depot sur les appareils
    reellement installes sur le poste - utile pour ajouter une montre recente
    sans connaitre son identifiant par coeur.

.PARAMETER Add
    Ajoute un identifiant d'appareil (sans le telecharger).

.PARAMETER Remove
    Retire un identifiant d'appareil du manifeste.

.PARAMETER DryRun
    Affiche la liste resultante sans modifier le manifeste.

.EXAMPLE
    pwsh ./garmin/tools/sync-products.ps1
    pwsh ./garmin/tools/sync-products.ps1 -Add fenix847mm
    pwsh ./garmin/tools/sync-products.ps1 -Remove fr255s
#>
[CmdletBinding()]
param(
    [string[]]$Add,
    [string[]]$Remove,
    [string]$SdkPath,
    [switch]$DryRun
)

$ErrorActionPreference = 'Stop'
$ProjectRoot = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
$ManifestPath = Join-Path $ProjectRoot 'manifest.xml'

function Get-DevicesDirectory {
    param([string]$Sdk)
    $candidates = @()
    if ($IsWindows -or $null -eq $IsWindows) {
        $candidates += Join-Path $env:APPDATA 'Garmin\ConnectIQ\Devices'
        $candidates += Join-Path $env:LOCALAPPDATA 'Garmin\ConnectIQ\Devices'
    } else {
        $candidates += Join-Path $HOME '.Garmin/ConnectIQ/Devices'
    }
    if ($Sdk) { $candidates += Join-Path $Sdk 'Devices' }
    foreach ($candidate in $candidates) {
        if (Test-Path $candidate) { return $candidate }
    }
    return $null
}

$devicesDirectory = Get-DevicesDirectory -Sdk $SdkPath
if (!$devicesDirectory -and !$Add -and !$Remove) {
    throw "Aucun dossier Devices trouve. Installez des appareils avec le SDK Manager, ou utilisez -Add <id>."
}

[xml]$manifest = Get-Content -Raw $ManifestPath
$manager = New-Object System.Xml.XmlNamespaceManager($manifest.NameTable)
$manager.AddNamespace('iq', 'http://www.garmin.com/xml/connectiq')
$products = $manifest.SelectSingleNode('//iq:products', $manager)
$current = @($products.SelectNodes('iq:product', $manager) | ForEach-Object { $_.id })

$result = @()
if ($devicesDirectory) {
    $result = @(Get-ChildItem $devicesDirectory -Directory | Select-Object -ExpandProperty Name | Sort-Object)
} else {
    $result = $current
}
foreach ($id in @($Add)) { if ($result -notcontains $id) { $result += $id } }
if ($Remove) { $result = @($result | Where-Object { $Remove -notcontains $_ }) }
$result = @($result | Sort-Object -Unique)

Write-Host "Appareils cibles : $($result.Count)"
Write-Host ("  " + ($result -join ' '))

if ($DryRun) { return }

$products.RemoveAll()
foreach ($id in $result) {
    $node = $manifest.CreateElement('iq', 'product', 'http://www.garmin.com/xml/connectiq')
    $node.SetAttribute('id', $id)
    $products.AppendChild($node) | Out-Null
}
$manifest.Save($ManifestPath)
Write-Host "Manifeste mis a jour : $ManifestPath" -ForegroundColor Green
