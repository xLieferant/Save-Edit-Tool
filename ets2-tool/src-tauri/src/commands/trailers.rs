use crate::utils::sii_parser::parse_trailers_from_sii;
use crate::utils::decrypt::decrypt_if_needed;
use crate::models::trailers::ParsedTrailer;
use crate::utils::regex_helper::cragex;
use crate::log;
use tauri::command;
use std::path::Path;

#[command]
pub async fn get_player_trailer(profile_path: String) -> Result<ParsedTrailer, String> {
    log!("get_player_trailer: Profil {}", profile_path);

    let path = format!("{}/save/quicksave/game.sii", profile_path);

    let content = decrypt_if_needed(Path::new(&path)).map_err(|e| {
        log!("Decrypt Fehler: {}", e);
        e
    })?;

    let trailers = parse_trailers_from_sii(&content);

    // my_trailer aus Player block extrahieren
    let re_player_trailer = cragex(
        r"player\s*:\s*[A-Za-z0-9._]+\s*\{[^}]*?my_trailer\s*:\s*([A-Za-z0-9._]+)"
    ).map_err(|e| format!("Regex Fehler: {}", e))?;

    let trailer_id = re_player_trailer
        .captures(&content)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
        .ok_or("my_trailer nicht gefunden".to_string())?;

    let id_clean = trailer_id.trim().to_lowercase();

    let base = trailers
        .into_iter()
        .find(|t| t.trailer_id.to_lowercase() == id_clean)
        .ok_or("Player Trailer nicht gefunden".to_string())?;

    Ok(base)
}

#[command]
pub async fn get_all_trailers(profile_path: String) -> Result<Vec<ParsedTrailer>, String> {
    log!("get_all_trailers: Profil {}", profile_path);

    let path = format!("{}/save/quicksave/game.sii", profile_path);

    let content = decrypt_if_needed(Path::new(&path)).map_err(|e| {
        log!("Decrypt Fehler: {}", e);
        e
    })?;

    let trailers = parse_trailers_from_sii(&content);
    log!("{} Trailer gefunden", trailers.len());

    Ok(trailers)
}
