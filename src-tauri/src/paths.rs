use std::path::PathBuf;

/// Dossier d'une ressource livrée avec l'application.
/// En développement : dossier `resources` du projet. Installée : à côté de l'exécutable.
fn resource_root() -> PathBuf {
    if cfg!(debug_assertions) {
        return PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("resources");
    }
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."))
}

/// Chemin d'un binaire livré avec l'application (adb, ios, scrcpy-server.jar, wintun.dll).
pub fn bin(name: &str) -> PathBuf {
    resource_root().join("bin").join(name)
}

/// Chemin d'un certificat Apple livré avec l'application (AppleRootCA.cer, AppleServerAuthenticationCA.cer).
pub fn cert(name: &str) -> PathBuf {
    resource_root().join("certs").join(name)
}
