use std::path::PathBuf;

/// Chemin d'un binaire livré avec l'application (adb, ios, scrcpy-server.jar).
/// En développement : resources/bin à la racine du projet. Installée : dossier bin à côté de l'exécutable.
pub fn bin(name: &str) -> PathBuf {
    if cfg!(debug_assertions) {
        return PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("resources")
            .join("bin")
            .join(name);
    }
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join("bin").join(name)))
        .unwrap_or_else(|| PathBuf::from(name))
}
