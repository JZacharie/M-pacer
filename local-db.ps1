#Requires -Version 7.0
<#
.SYNOPSIS
    Base PostgreSQL locale de M-pacer (conteneur Podman) pour le backend et les tests.

.DESCRIPTION
    M-pacer a besoin d'un PostgreSQL pour le backend local et pour les tests
    d'integration de mpacer-api (MPACER_TEST_DATABASE_URL). Ce script demarre un
    conteneur jetable, cree les deux bases (dev et test) et affiche l'URL a utiliser.

    Particularite de ce poste : le relais de ports WSL ne publie PAS les ports des
    conteneurs sur 127.0.0.1. La base est donc joignable sur l'IP de la machine
    Podman (eth0), que le script resout a chaque appel. Ne codez pas cette IP en
    dur : elle change au redemarrage de la machine Podman.

    Le conteneur tourne en reseau hote (et non avec -p) : la publication de port de
    netavark reste figee sur l'ancienne IP du conteneur apres un redemarrage de la
    machine, ce qui rend la base injoignable jusqu'a une recreation.

    Deuxieme particularite : un reliquat de regles Docker dans la machine Podman
    laisse la chaine nftables FORWARD en politique DROP, ce qui coupe tout le trafic
    entre conteneurs et vers l'exterieur. Le script verifie et corrige ces regles.

.PARAMETER Action
    Start (defaut) : demarre la base et affiche l'URL.
    Stop           : arrete le conteneur.
    Status         : etat du conteneur, de la machine et des bases.
    Reset          : DETRUIT le volume (toutes les donnees locales) puis recree.
    Tests          : lance les tests d'integration de mpacer-api sur la base de test.
    Run            : lance mpacer-api en mode dev (auth simulee) sur http://localhost:8080.
    Env            : affiche les variables d'environnement a exporter.

.EXAMPLE
    pwsh ./local-db.ps1
    pwsh ./local-db.ps1 -Action Tests
    pwsh ./local-db.ps1 -Action Run
    $env:MPACER_TEST_DATABASE_URL = (pwsh ./local-db.ps1 -Action Env | Select-String TEST).ToString().Split('=')[1]
#>
[CmdletBinding()]
param(
    [ValidateSet('Start', 'Stop', 'Status', 'Reset', 'Tests', 'Run', 'Env')]
    [string] $Action = 'Start',

    [string] $Container = 'mpacer-pg',
    [string] $Volume = 'mpacer-pgdata',
    [string] $Image = 'docker.io/library/postgres:17-alpine',
    [int]    $Port = 5432,
    [string] $DbUser = 'mpacer',
    [string] $DbPassword = 'mpacer',
    [string] $DevDb = 'mpacer',
    [string] $TestDb = 'mpacer_test',

    # Ne pas toucher aux regles nftables de la machine Podman.
    [switch] $NoForwardFix
)

$ErrorActionPreference = 'Stop'
$racine = $PSScriptRoot
$script:VmIp = $null

function Etape([string] $texte) {
    Write-Host ''
    Write-Host ('=== ' + $texte) -ForegroundColor Cyan
}
function Ok([string] $texte) { Write-Host ('  [ok]      ' + $texte) -ForegroundColor Green }
function Info([string] $texte) { Write-Host ('  [info]    ' + $texte) -ForegroundColor Gray }
function Alerte([string] $texte) { Write-Host ('  [attention] ' + $texte) -ForegroundColor Yellow }
function Echec([string] $texte) { Write-Host ('  [echec]   ' + $texte) -ForegroundColor Red; exit 1 }

function Test-Podman {
    if (-not (Get-Command podman -ErrorAction SilentlyContinue)) {
        Echec 'podman introuvable dans le PATH.'
    }
}

function Initialize-Machine {
    Etape 'Machine Podman'
    $liste = podman machine list --format '{{.Name}} {{.Running}}' 2>&1
    if ($liste -notmatch 'true') {
        Alerte 'Machine Podman arretee : demarrage...'
        podman machine start 2>&1 | Out-Null
    }
    for ($i = 0; $i -lt 30; $i++) {
        podman info --format 'ok' 2>$null | Out-Null
        if ($LASTEXITCODE -eq 0) { break }
        Start-Sleep -Milliseconds 1000
    }
    if ($LASTEXITCODE -ne 0) { Echec 'La machine Podman ne repond pas.' }
    Ok 'Machine Podman operationnelle.'
}

function Resolve-VmIp {
    if ($script:VmIp) { return $script:VmIp }
    $ip = ((podman machine ssh "ip -4 -o addr show eth0 | sed -E 's#.*inet ([0-9.]+)/.*#\1#'" 2>&1) | Select-Object -First 1)
    $ip = ($ip -replace '\s', '')
    if ($ip -notmatch '^\d+\.\d+\.\d+\.\d+$') {
        Echec ('IP de la machine Podman illisible : ' + $ip)
    }
    $script:VmIp = $ip
    return $ip
}

# Un reliquat de regles Docker laisse FORWARD en politique DROP : sans ces deux
# regles, les conteneurs Podman ne peuvent plus se parler ni sortir.
function Repair-Forwarding {
    if ($NoForwardFix) { return }
    Etape 'Regles de transfert nftables'
    $regles = 'insert rule ip filter FORWARD iifname "podman0" accept' + [char]10 +
              'insert rule ip filter FORWARD oifname "podman0" accept'
    $b64 = [Convert]::ToBase64String([Text.Encoding]::UTF8.GetBytes($regles))
    $compte = ((podman machine ssh "nft list chain ip filter FORWARD 2>/dev/null | grep -c podman0" 2>&1) | Select-Object -First 1)
    $compte = ($compte -replace '\s', '')
    if ($compte -match '^\d+$' -and [int]$compte -ge 2) {
        Ok 'Transfert podman0 deja autorise.'
        return
    }
    podman machine ssh "echo $b64 | base64 -d > /tmp/mpacer-forward.nft && nft -f /tmp/mpacer-forward.nft" 2>&1 | Out-Null
    $apres = ((podman machine ssh "nft list chain ip filter FORWARD 2>/dev/null | grep -c podman0" 2>&1) | Select-Object -First 1)
    $apres = ($apres -replace '\s', '')
    if ($apres -match '^\d+$' -and [int]$apres -ge 2) {
        Ok 'Transfert podman0 autorise (correction appliquee).'
    } else {
        Alerte 'Correction nftables non confirmee ; les conteneurs peuvent rester injoignables.'
    }
}

function Ensure-Container {
    Etape 'Conteneur PostgreSQL'
    podman volume create $Volume 2>&1 | Out-Null
    $existe = @(podman ps -a --filter ('name=^' + $Container + '$') --format '{{.Names}}' 2>&1) -contains $Container
    if ($existe) {
        # Reseau hote obligatoire ici : avec la publication de port (DNAT netavark),
        # la regle reste figee sur l'ancienne IP du conteneur apres un redemarrage de
        # la machine Podman, et le trafic part vers une IP morte. En reseau hote,
        # PostgreSQL ecoute directement sur l'interface de la VM.
        $mode = ((podman inspect $Container --format '{{.HostConfig.NetworkMode}}' 2>&1) | Select-Object -First 1)
        $mode = ($mode -replace '\s', '')
        if ($mode -ne 'host') {
            Alerte ('Conteneur en reseau "' + $mode + '" : recreation en reseau hote (volume conserve).')
            podman rm -f $Container 2>&1 | Out-Null
            $existe = $false
        }
    }
    if (-not $existe) {
        Info ('Creation du conteneur ' + $Container + ' (' + $Image + ', reseau hote, port ' + $Port + ')')
        $arguments = @(
            'run', '-d', '--name', $Container, '--network', 'host',
            '-e', ('POSTGRES_USER=' + $DbUser),
            '-e', ('POSTGRES_PASSWORD=' + $DbPassword),
            '-e', ('POSTGRES_DB=' + $DevDb),
            '-v', ($Volume + ':/var/lib/postgresql/data'),
            $Image
        )
        podman @arguments 2>&1 | Out-Null
    } else {
        $enMarche = @(podman ps --filter ('name=^' + $Container + '$') --format '{{.Names}}' 2>&1) -contains $Container
        if (-not $enMarche) { podman start $Container 2>&1 | Out-Null }
    }
    for ($i = 0; $i -lt 40; $i++) {
        podman exec $Container pg_isready -U $DbUser -q 2>$null
        if ($LASTEXITCODE -eq 0) { break }
        Start-Sleep -Milliseconds 750
    }
    if ($LASTEXITCODE -ne 0) { Echec 'PostgreSQL ne repond pas dans le conteneur.' }
    $version = ((podman exec $Container psql -U $DbUser -d $DevDb -tAc 'SHOW server_version' 2>&1) | Select-Object -First 1).Trim()
    Ok ('PostgreSQL ' + $version + ' pret dans ' + $Container + '.')
    # Base de test separee : les tests creent et detruisent leurs schemas.
    $bases = ((podman exec $Container psql -U $DbUser -d $DevDb -tAc "SELECT datname FROM pg_database" 2>&1) -join ' ')
    if ($bases -notmatch [regex]::Escape($TestDb)) {
        podman exec $Container psql -U $DbUser -d $DevDb -c ('CREATE DATABASE ' + $TestDb) 2>&1 | Out-Null
        Ok ('Base de test creee : ' + $TestDb)
    } else {
        Ok ('Base de test presente : ' + $TestDb)
    }
}

function Get-Url([string] $base) {
    $ip = Resolve-VmIp
    return 'postgresql://' + $DbUser + ':' + $DbPassword + '@' + $ip + ':' + $Port + '/' + $base + '?sslmode=disable'
}

function Show-Connexion {
    Etape 'Connexion'
    Info 'Le relais WSL ne publie pas 127.0.0.1 : utilisez l''IP de la machine Podman.'
    Write-Host ('  MPACER_DATABASE_URL      = ' + (Get-Url $DevDb))
    Write-Host ('  MPACER_TEST_DATABASE_URL = ' + (Get-Url $TestDb))
    Write-Host ''
    Write-Host '  Backend local :' -ForegroundColor Gray
    Write-Host ('    $env:MPACER_DATABASE_URL = "' + (Get-Url $DevDb) + '"')
    Write-Host '    $env:MPACER_DEV_AUTH = "1"'
    Write-Host '    $env:MPACER_PUBLIC_URL = "http://localhost:8080"'
    Write-Host '    cargo run -p mpacer-api'
}

switch ($Action) {
    'Start' {
        Test-Podman
        Initialize-Machine
        Repair-Forwarding
        Ensure-Container
        Show-Connexion
    }
    'Status' {
        Test-Podman
        Etape 'Etat'
        podman ps -a --filter ('name=^' + $Container + '$') --format 'table {{.Names}}\t{{.Status}}\t{{.Ports}}'
        Initialize-Machine
        Resolve-VmIp | ForEach-Object { Info ('IP machine Podman : ' + $_) }
        podman exec $Container psql -U $DbUser -d $DevDb -tAc 'SELECT datname FROM pg_database ORDER BY 1' 2>&1
    }
    'Stop' {
        Test-Podman
        Etape 'Arret'
        podman stop $Container 2>&1 | Out-Null
        Ok ('Conteneur ' + $Container + ' arrete (volume conserve).')
    }
    'Reset' {
        Test-Podman
        Etape 'Remise a zero (destructif)'
        Alerte 'Toutes les donnees du volume seront perdues.'
        podman rm -f $Container 2>&1 | Out-Null
        podman volume rm -f $Volume 2>&1 | Out-Null
        Ok 'Conteneur et volume supprimes.'
        Initialize-Machine
        Repair-Forwarding
        Ensure-Container
        Show-Connexion
    }
    'Tests' {
        Test-Podman
        Initialize-Machine
        Repair-Forwarding
        Ensure-Container
        $env:MPACER_TEST_DATABASE_URL = Get-Url $TestDb
        Etape 'Tests d''integration mpacer-api'
        Info ('MPACER_TEST_DATABASE_URL = ' + $env:MPACER_TEST_DATABASE_URL)
        Push-Location $racine
        try {
            $chrono = [Diagnostics.Stopwatch]::StartNew()
            cargo test -p mpacer-api
            $code = $LASTEXITCODE
            Write-Host ('  duree : ' + [math]::Round($chrono.Elapsed.TotalSeconds, 1) + ' s') -ForegroundColor Gray
            exit $code
        } finally {
            Pop-Location
        }
    }
    'Run' {
        Test-Podman
        Initialize-Machine
        Repair-Forwarding
        Ensure-Container
        $env:MPACER_DATABASE_URL = Get-Url $DevDb
        $env:MPACER_DEV_AUTH = '1'
        $env:MPACER_PUBLIC_URL = 'http://localhost:8080'
        Etape 'mpacer-api en mode dev'
        Info 'http://localhost:8080 (connexion : POST /auth/dev-login)'
        Push-Location $racine
        try { cargo run -p mpacer-api } finally { Pop-Location }
    }
    'Env' {
        Test-Podman
        Initialize-Machine
        Resolve-VmIp | Out-Null
        Write-Output ('MPACER_DATABASE_URL=' + (Get-Url $DevDb))
        Write-Output ('MPACER_TEST_DATABASE_URL=' + (Get-Url $TestDb))
    }
}
