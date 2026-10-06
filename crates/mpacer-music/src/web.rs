//! Page HTML autonome de l'interface locale : aucun framework JS, aucune
//! ressource externe. Les appels se font vers l'API JSON de `crate::server`.

/// Page servie sur `/`.
pub const PAGE: &str = r##"<!doctype html>
<html lang="fr">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>mpacer-music</title>
<style>
:root { --fond: #10151c; --carte: #1a2230; --bord: #2b3648; --texte: #e8edf5; --doux: #93a1b5; --accent: #3d8bfd; --ok: #35c07a; --erreur: #e5484d; }
* { box-sizing: border-box; }
body { margin: 0; background: var(--fond); color: var(--texte); font-family: system-ui, "Segoe UI", sans-serif; }
header { padding: 20px 24px; border-bottom: 1px solid var(--bord); }
h1 { margin: 0; font-size: 20px; }
h2 { margin: 0 0 10px; font-size: 15px; color: var(--doux); text-transform: uppercase; letter-spacing: .06em; }
main { max-width: 1080px; margin: 0 auto; padding: 20px 24px 60px; display: grid; gap: 16px; }
section { background: var(--carte); border: 1px solid var(--bord); border-radius: 10px; padding: 16px; }
.row { display: flex; flex-wrap: wrap; gap: 8px; align-items: center; }
input[type=text] { flex: 1 1 320px; min-width: 220px; background: #0d1219; border: 1px solid var(--bord); color: var(--texte); padding: 9px 11px; border-radius: 8px; }
button { background: #223047; color: var(--texte); border: 1px solid var(--bord); padding: 9px 14px; border-radius: 8px; cursor: pointer; }
button:hover { border-color: var(--accent); }
button.primary { background: var(--accent); border-color: var(--accent); color: #04101f; font-weight: 600; }
button.danger { border-color: var(--erreur); color: var(--erreur); }
button.dir { padding: 5px 9px; font-size: 13px; }
button:disabled { opacity: .5; cursor: default; }
.muted { color: var(--doux); margin: 8px 0 0; }
.browser { display: flex; flex-wrap: wrap; gap: 6px; margin-top: 10px; }
table { width: 100%; border-collapse: collapse; margin-top: 10px; font-size: 14px; }
th, td { text-align: left; padding: 6px 8px; border-bottom: 1px solid var(--bord); }
th { color: var(--doux); font-weight: 500; }
.progress { height: 10px; background: #0d1219; border: 1px solid var(--bord); border-radius: 6px; overflow: hidden; margin-top: 12px; }
#bar { height: 100%; width: 0; background: var(--accent); transition: width .2s; }
.logs { background: #0d1219; border: 1px solid var(--bord); border-radius: 8px; padding: 10px; min-height: 40px; max-height: 220px; overflow: auto; font-size: 12px; color: var(--doux); white-space: pre-wrap; }
</style>
</head>
<body>
<header>
  <h1>mpacer-music</h1>
  <p class="muted">Transfere les fichiers audio d'un dossier du PC vers la montre M-pacer, par USB (adb push).</p>
</header>
<main>
<section>
  <h2>1. Playlist</h2>
  <div class="row">
    <input id="manifest" type="text" placeholder="chemin du manifeste, par exemple D:\\Musique\\run-170.json">
    <button onclick="inspectNow()">Analyser</button>
  </div>
  <p id="playlist" class="muted">aucun manifeste analyse</p>
</section>
<section>
  <h2>2. Dossier des audio</h2>
  <div class="row">
    <input id="folder" type="text" placeholder="dossier des fichiers audio">
    <button onclick="browseNow()">Parcourir</button>
    <button onclick="inspectNow()">Analyser</button>
  </div>
  <div id="browser" class="browser"></div>
  <p id="folderinfo" class="muted"></p>
</section>
<section>
  <h2>3. Montre</h2>
  <p id="devices" class="muted">recherche d'une montre par adb...</p>
</section>
<section>
  <h2>4. Appariement</h2>
  <p id="summary" class="muted">lancez une analyse pour apparie les fichiers aux pistes</p>
  <table id="matches">
    <thead><tr><th>#</th><th>Titre</th><th>Fichier</th><th>BPM</th><th>Duree</th><th>Score</th></tr></thead>
    <tbody></tbody>
  </table>
</section>
<section>
  <h2>5. Transfert</h2>
  <div class="row">
    <button class="primary" onclick="startTransfer(false)">Transferer sur la montre</button>
    <button onclick="startTransfer(true)">Simulation</button>
    <button onclick="copyReport()">Copier le rapport</button>
    <button id="cancel" class="danger" onclick="cancelTransfer()" disabled>Annuler</button>
  </div>
  <div class="progress"><div id="bar"></div></div>
  <p id="progress" class="muted">aucun transfert en cours</p>
  <pre id="logs" class="logs"></pre>
</section>
</main>
<script>
var currentJob = null;
var lastInspection = null;

function el(id) { return document.getElementById(id); }

function humanBytes(value) {
  var units = ['o', 'Ko', 'Mo', 'Go', 'To'];
  var number = Number(value) || 0;
  var unit = 0;
  while (number >= 1024 && unit + 1 < units.length) { number = number / 1024; unit = unit + 1; }
  if (unit === 0) { return number + ' o'; }
  return number.toFixed(1) + ' ' + units[unit];
}

function callApi(path, options) {
  return fetch(path, options).then(function (response) {
    return response.json().catch(function () { return {}; }).then(function (data) {
      if (!response.ok) { throw new Error(data.error || ('HTTP ' + response.status)); }
      return data;
    });
  });
}

function showError(error) {
  el('summary').textContent = 'erreur : ' + error.message;
}

function cell(text) {
  var td = document.createElement('td');
  td.textContent = text;
  return td;
}

function formatDuration(seconds) {
  if (!seconds) { return ''; }
  var total = Math.round(seconds);
  var minutes = Math.floor(total / 60);
  var rest = total % 60;
  return minutes + ':' + (rest < 10 ? '0' : '') + rest;
}

function dirButton(label, path) {
  var button = document.createElement('button');
  button.className = 'dir';
  button.textContent = label;
  button.onclick = function () { el('folder').value = path; browseNow(); };
  return button;
}

function browseNow() {
  var folder = el('folder').value.trim();
  var url = '/api/browse';
  if (folder) { url = url + '?path=' + encodeURIComponent(folder); }
  return callApi(url).then(function (data) {
    el('folder').value = data.path;
    el('folderinfo').textContent = data.audio_count + ' fichier(s) audio dans ce dossier';
    var box = el('browser');
    box.innerHTML = '';
    if (data.parent) { box.appendChild(dirButton('.. (dossier parent)', data.parent)); }
    data.dirs.forEach(function (dir) { box.appendChild(dirButton(dir.name, dir.path)); });
  }).catch(showError);
}

function renderInspection(data) {
  lastInspection = data;
  el('playlist').textContent = data.playlist.name + ' (' + data.playlist.id + ') - ' + data.playlist.track_count + ' titre(s)';
  var found = data.matches.filter(function (match) { return !!match.file; }).length;
  el('summary').textContent = found + '/' + data.playlist.track_count + ' titres trouves - ' + data.missing.length + ' manquant(s) - ' + data.unused_files.length + ' fichier(s) ignore(s) - ' + humanBytes(data.total_bytes);
  var body = el('matches').querySelector('tbody');
  body.innerHTML = '';
  data.matches.forEach(function (match) {
    var row = document.createElement('tr');
    row.appendChild(cell(String(match.position)));
    row.appendChild(cell(match.title));
    row.appendChild(cell(match.file || '-- manquant --'));
    row.appendChild(cell(match.bpm ? String(Math.round(match.bpm)) : ''));
    row.appendChild(cell(formatDuration(match.duration_s)));
    row.appendChild(cell(match.score ? match.score.toFixed(2) : ''));
    body.appendChild(row);
  });
}

function inspectNow() {
  var body = { folder: el('folder').value.trim(), manifest_path: el('manifest').value.trim() || null };
  if (!body.folder) { el('summary').textContent = 'choisissez d abord le dossier des fichiers audio'; return Promise.resolve(); }
  return callApi('/api/inspect', {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(body)
  }).then(renderInspection).catch(showError);
}

function startTransfer(dryRun) {
  var body = {
    folder: el('folder').value.trim(),
    manifest_path: el('manifest').value.trim() || null,
    dry_run: dryRun,
    prune: false
  };
  if (!body.folder) { el('progress').textContent = 'choisissez d abord le dossier des fichiers audio'; return Promise.resolve(); }
  return callApi('/api/transfer', {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(body)
  }).then(function (data) {
    currentJob = data.job_id;
    el('cancel').disabled = false;
    el('logs').textContent = '';
    el('progress').textContent = 'transfert demarre';
    pollJob();
  }).catch(showError);
}

function pollJob() {
  if (!currentJob) { return; }
  callApi('/api/transfer/' + currentJob).then(function (job) {
    var percent = job.total > 0 ? Math.round(100 * job.current / job.total) : 0;
    el('bar').style.width = percent + '%';
    el('progress').textContent = job.step + ' - ' + job.current + '/' + job.total + ' - ' + humanBytes(job.bytes_sent);
    el('logs').textContent = (job.logs || []).join('\n');
    if (job.state === 'running') {
      setTimeout(pollJob, 500);
      return;
    }
    el('cancel').disabled = true;
    el('progress').textContent = job.state + ' - ' + job.step + (job.error ? (' : ' + job.error) : '');
    currentJob = null;
    if (job.state === 'done') { inspectNow(); }
  }).catch(showError);
}

function cancelTransfer() {
  if (!currentJob) { return; }
  callApi('/api/transfer/' + currentJob + '/cancel', { method: 'POST' }).catch(showError);
}

function loadDevices() {
  return callApi('/api/devices').then(function (data) {
    if (data.adb === 'absent') {
      el('devices').textContent = 'adb absent : installer les platform-tools Android';
      return;
    }
    if (!data.devices || data.devices.length === 0) {
      el('devices').textContent = 'aucune montre detectee (adb : ' + data.adb + ') - la simulation et le dossier cible restent utilisables';
      return;
    }
    el('devices').textContent = data.devices.map(function (device) {
      var space = device.free_bytes ? ('libre ' + humanBytes(device.free_bytes) + ' / ' + humanBytes(device.total_bytes)) : 'espace libre inconnu';
      return (device.model || device.serial) + ' (' + device.state + ', ' + space + ')';
    }).join(' | ');
  }).catch(showError);
}

function copyReport() {
  if (!lastInspection) { return; }
  var lines = [lastInspection.playlist.name + ' (' + lastInspection.playlist.id + ')'];
  lastInspection.matches.forEach(function (match) {
    lines.push(match.position + ' - ' + match.title + ' - ' + (match.file || 'manquant'));
  });
  lines.push('total : ' + humanBytes(lastInspection.total_bytes));
  if (navigator.clipboard) { navigator.clipboard.writeText(lines.join('\n')); }
}

loadDevices();
inspectNow();
</script>
</body>
</html>
"##;
