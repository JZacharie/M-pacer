//! Fichiers audio televerses : collecte multipart, ecriture sur disque et
//! nettoyage.
//!
//! Deux points d'entree partagent ce code, avec la meme validation :
//!   * POST /music/upload (navigateur, cookie de session) ;
//!   * POST /api/v1/music/playlists (montre et compagnon, jeton d'appareil).

use crate::error::{AppError, AppResult};
use crate::models::{
    audio_extension, audio_mime, title_from_filename, MusicPlaylistInput, MusicTrackInput,
    MAX_UPLOAD_BYTES,
};
use crate::state::AppState;
use axum::extract::Multipart;

/// Champs utiles d'un corps multipart de televersement.
#[derive(Debug, Default)]
pub struct UploadForm {
    pub name: String,
    pub files: Vec<(String, Vec<u8>)>,
}

/// Collecte le nom de playlist (`name_field`) et les fichiers (`files`).
///
/// Le corps est plafonne par la route (DefaultBodyLimit) ; le total est verifie
/// ici aussi, pour renvoyer un code d'erreur stable plutot qu'une erreur de flux.
pub async fn collect_upload(mut multipart: Multipart, name_field: &str) -> AppResult<UploadForm> {
    let mut form = UploadForm::default();
    let mut total: i64 = 0;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|error| invalid_multipart(&error.to_string()))?
    {
        let nom = field.name().map(str::to_string);
        match nom.as_deref() {
            Some(cle) if cle == name_field => {
                form.name = field
                    .text()
                    .await
                    .map_err(|error| invalid_multipart(&error.to_string()))?;
            }
            Some("files") => {
                let filename = field.file_name().map(str::to_string).unwrap_or_default();
                let bytes = field
                    .bytes()
                    .await
                    .map_err(|error| map_field_error(&error.to_string()))?;
                total += bytes.len() as i64;
                if total > MAX_UPLOAD_BYTES {
                    return Err(payload_too_large());
                }
                if !filename.is_empty() && !bytes.is_empty() {
                    form.files.push((filename, bytes.to_vec()));
                }
            }
            // Champ inconnu : le corps doit tout de meme etre consomme.
            _ => {
                let _ = field.bytes().await;
            }
        }
    }

    form.name = form.name.trim().to_string();
    Ok(form)
}

/// Ecrit les fichiers, cree la playlist et ses titres.
///
/// Renvoie (identifiant de playlist, nombre de titres, taille totale en octets).
/// Un echec en cours de route ne laisse ni fichier orphelin ni playlist vide.
pub async fn import_uploaded_files(
    state: &AppState,
    user_id: &str,
    name: &str,
    files: &[(String, Vec<u8>)],
) -> AppResult<(String, usize, i64)> {
    if name.trim().is_empty() {
        return Err(AppError::InvalidMultipart(
            "le nom de la playlist est obligatoire".to_string(),
        ));
    }
    if files.is_empty() {
        return Err(AppError::InvalidMultipart(
            "aucun fichier audio recu".to_string(),
        ));
    }

    let mut inputs: Vec<MusicTrackInput> = Vec::with_capacity(files.len());
    let mut written: Vec<String> = Vec::new();
    let mut total_bytes: i64 = 0;
    for (position, (filename, bytes)) in files.iter().enumerate() {
        let stored = match store_file(state, user_id, filename, bytes).await {
            Ok(stored) => stored,
            Err(error) => {
                remove_files(state, &written).await;
                return Err(error);
            }
        };
        // Le BPM est lu dans les balises du fichier (ID3 TBPM, Vorbis BPM=,
        // atome MP4 tmpo) : le meme decodeur que la montre.
        let bpm = crate::bpm::bpm_from_bytes(bytes, filename);
        written.push(stored.0.clone());
        total_bytes += stored.2;
        inputs.push(MusicTrackInput {
            position: position as i32,
            title: title_from_filename(filename),
            artist: None,
            album: None,
            duration_s: None,
            bpm,
            bpm_source: bpm.map(|_| "tag".to_string()),
            spotify_uri: None,
            mime: Some(stored.1),
            size_bytes: Some(stored.2),
            storage_path: Some(stored.0),
        });
    }

    let now = state.now_ms();
    let playlist = match crate::db::insert_music_playlist(
        &state.pool,
        user_id,
        &MusicPlaylistInput {
            name: name.chars().take(200).collect(),
            source: "upload".to_string(),
            spotify_id: None,
            cover_url: None,
            target_bpm: None,
        },
        now,
    )
    .await
    {
        Ok(playlist) => playlist,
        Err(error) => {
            remove_files(state, &written).await;
            return Err(error.into());
        }
    };

    for input in &inputs {
        if let Err(error) =
            crate::db::insert_music_track(&state.pool, user_id, &playlist.id, input, now).await
        {
            // La playlist part en cascade ; les fichiers deja ecrits aussi.
            let _ = crate::db::delete_music_playlist(&state.pool, user_id, &playlist.id).await;
            remove_files(state, &written).await;
            return Err(error.into());
        }
    }

    Ok((playlist.id, inputs.len(), total_bytes))
}

/// Ecrit un fichier audio sous `MPACER_MEDIA_DIR`.
///
/// Le nom sur disque est un UUID : le nom d'origine n'est jamais utilise comme
/// chemin, donc aucune traversee de repertoire n'est possible.
async fn store_file(
    state: &AppState,
    user_id: &str,
    filename: &str,
    bytes: &[u8],
) -> AppResult<(String, String, i64)> {
    let Some(extension) = audio_extension(filename) else {
        return Err(AppError::UnsupportedMediaType(format!(
            "« {filename} » n'est pas un fichier audio reconnu (MP3, OGG, M4A, WAV)"
        )));
    };
    let directory = state.config.media_dir.join(user_id);
    tokio::fs::create_dir_all(&directory)
        .await
        .map_err(|error| AppError::internal(format!("creation du dossier media : {error}")))?;
    let nom = format!("{}.{extension}", uuid::Uuid::new_v4());
    let path = directory.join(&nom);
    tokio::fs::write(&path, bytes)
        .await
        .map_err(|error| AppError::internal(format!("ecriture du fichier audio : {error}")))?;
    Ok((
        format!("{user_id}/{nom}"),
        audio_mime(&extension).to_string(),
        bytes.len() as i64,
    ))
}

/// Supprime des fichiers audio du disque (nettoyage apres un echec ou une
/// suppression de playlist). Un fichier deja absent n'est pas une erreur.
pub async fn remove_files(state: &AppState, relatives: &[String]) {
    for relative in relatives {
        if let Some(path) = crate::routes::media_path(&state.config.media_dir, relative) {
            if let Err(error) = tokio::fs::remove_file(&path).await {
                if error.kind() != std::io::ErrorKind::NotFound {
                    tracing::warn!(chemin = %path.display(), error = %error, "fichier audio non supprime");
                }
            }
        }
    }
}

fn invalid_multipart(message: &str) -> AppError {
    AppError::InvalidMultipart(message.to_string())
}

fn payload_too_large() -> AppError {
    AppError::PayloadTooLarge(format!(
        "limite de {} Mo par requete depassee",
        MAX_UPLOAD_BYTES / (1024 * 1024)
    ))
}

/// Une erreur de flux qui parle de taille est un depassement de limite : le
/// code d'erreur doit rester stable (`payload_too_large`, pas `invalid_multipart`).
fn map_field_error(message: &str) -> AppError {
    let lowered = message.to_lowercase();
    if lowered.contains("size") || lowered.contains("limit") {
        payload_too_large()
    } else {
        invalid_multipart(message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn size_limit_errors_keep_their_code() {
        assert_eq!(
            map_field_error("stream size limit exceeded").code(),
            "payload_too_large"
        );
        assert_eq!(
            map_field_error("corps tronque").code(),
            "invalid_multipart"
        );
        assert_eq!(payload_too_large().status().as_u16(), 413);
    }
}
