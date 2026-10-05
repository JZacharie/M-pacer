# Construction et publication de l'image M-pacer (podman, sans Docker).
#
#   pwsh deploy/build-image.ps1                     # construit localement (arch courante)
#   pwsh deploy/build-image.ps1 -Push               # construit et pousse sur ghcr.io
#   pwsh deploy/build-image.ps1 -MultiArch -Push    # manifeste amd64 + arm64 (necessite qemu)
#
# Prerequis : podman machine demarree, et pour -Push un jeton GitHub (PAT) avec
# la permission write:packages.

param(
  [string]$Registry = "ghcr.io",
  [string]$Owner = "jzacharie",
  [string]$Name = "mpacer",
  [string]$Tag = "latest",
  [switch]$Push,
  [switch]$MultiArch,
  [string]$Username = "jzacharie"
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
$image = [string]::Format("{0}/{1}/{2}:{3}", $Registry, $Owner, $Name, $Tag)

Write-Host "-> Construction de $image" -ForegroundColor Cyan
if ($MultiArch) {
  # --format docker : conserve le HEALTHCHECK de l'image (perdu avec le format OCI)
  podman build --format docker --platform linux/amd64,linux/arm64 --manifest "$Owner/$Name" -f "$root/deploy/Dockerfile" $root
} else {
  podman build --format docker -t $image -f "$root/deploy/Dockerfile" $root
}

if ($Push) {
  Write-Host "-> Connexion au registre $Registry" -ForegroundColor Cyan
  podman login $Registry --username $Username
  if ($MultiArch) {
    podman manifest push --all "$Owner/$Name" "docker://$image"
  } else {
    podman push $image
  }
  Write-Host "-> Image publiee : $image" -ForegroundColor Green
  Write-Host "   Renseignez image.repository et image.tag dans values-jo3.yaml"
}
