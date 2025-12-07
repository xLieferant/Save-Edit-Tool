use std::path::Path;
use crate::log;
use crate::utils::decrypt::decrypt_if_needed;

/// Lädt und entschlüsselt die game.sii eines Profils
pub fn load_game_sii(profile_path: &str) -> Result<String, String> {
    let game_sii_path_str = format!("{}/save/quicksave/game.sii", profile_path);
    let game_sii_path = Path::new(&game_sii_path_str);
    log!("Versuche game.sii zu laden/entschlüsseln: {}", game_sii_path.display());

    let content = decrypt_if_needed(game_sii_path)?;
    log!("Inhalt erfolgreich aus game.sii extrahiert und entschlüsselt.");
    Ok(content)
}
