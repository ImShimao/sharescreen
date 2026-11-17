//! Règles de compatibilité selon la version de l'appareil. Fonctions pures, testées.

/// Plus bas niveau d'API supporté par scrcpy v5 (Android 5.0).
pub const MIN_ANDROID_API: u32 = 21;

/// La capture du son demande Android 11 (API 30) ; en dessous, la vidéo seule est affichée.
pub fn android_audio_supported(api: u32) -> bool {
    api >= 30
}

pub fn android_supported(api: u32) -> bool {
    api >= MIN_ANDROID_API
}

/// Xiaomi, Redmi et POCO demandent une option de sécurité supplémentaire pour injecter les touches.
pub fn is_xiaomi_family(manufacturer: &str) -> bool {
    let m = manufacturer.to_lowercase();
    m.contains("xiaomi") || m.contains("redmi") || m.contains("poco")
}

/// Numéro majeur d'une version : "18.2" -> 18, "" -> 0.
pub fn major_version(version: &str) -> u32 {
    version
        .trim()
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .collect::<String>()
        .parse()
        .unwrap_or(0)
}

/// iOS 17 et plus : l'affichage passe par un tunnel (droits administrateur, wintun.dll).
pub fn ios_needs_tunnel(version: &str) -> bool {
    major_version(version) >= 17
}

/// Le Mode développeur existe à partir d'iOS 16.
pub fn ios_has_developer_mode(version: &str) -> bool {
    major_version(version) >= 16
}

#[derive(Debug, Clone, Default)]
pub struct AndroidProps {
    pub market_name: String,
    pub manufacturer: String,
    pub model: String,
    pub release: String,
    pub api: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AndroidDescription {
    pub name: String,
    pub detail: String,
    pub hint: Option<String>,
}

pub fn describe_android(p: &AndroidProps) -> AndroidDescription {
    let fallback = [p.manufacturer.as_str(), p.model.as_str()]
        .iter()
        .filter(|s| !s.is_empty())
        .copied()
        .collect::<Vec<_>>()
        .join(" ");
    let name = if !p.market_name.is_empty() {
        p.market_name.clone()
    } else if !fallback.is_empty() {
        fallback
    } else {
        "Tablette Android".to_string()
    };
    let release = if p.release.is_empty() { "?" } else { p.release.as_str() };
    let api = if p.api == 0 { "?".to_string() } else { p.api.to_string() };
    let hint = is_xiaomi_family(&p.manufacturer).then(|| {
        "Xiaomi, Redmi, POCO : pour le contrôle, activez aussi « Débogage USB (paramètres de sécurité) ».".to_string()
    });
    AndroidDescription { name, detail: format!("Android {release} · API {api}"), hint }
}

/// Dernière erreur émise par le serveur, pour expliquer un échec à l'utilisateur.
pub fn server_failure_hint(lines: &[String]) -> String {
    const KEYWORDS: [&str; 4] = ["error", "exception", "failed", "unsupported"];
    let is_error = |l: &&String| {
        let low = l.to_lowercase();
        KEYWORDS.iter().any(|k| low.contains(k))
    };
    let line = lines
        .iter()
        .filter(is_error)
        .last()
        .or_else(|| lines.last())
        .map(|l| l.trim().to_string())
        .unwrap_or_default();
    if line.is_empty() {
        String::new()
    } else {
        let short: String = line.chars().take(220).collect();
        format!("Détail : {short}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn android_refuse_sous_5_0_et_demande_le_son_a_partir_de_11() {
        assert!(!android_supported(20));
        assert!(android_supported(21));
        assert!(!android_audio_supported(29));
        assert!(android_audio_supported(30));
    }

    #[test]
    fn reconnait_les_marques_a_reglage_de_securite() {
        assert!(is_xiaomi_family("Xiaomi"));
        assert!(is_xiaomi_family("REDMI"));
        assert!(is_xiaomi_family("POCO"));
        assert!(!is_xiaomi_family("samsung"));
    }

    #[test]
    fn decrit_un_appareil_avec_le_nom_commercial_en_priorite() {
        let d = describe_android(&AndroidProps {
            market_name: "Xiaomi Pad 6S Pro".into(),
            manufacturer: "Xiaomi".into(),
            model: "23XX".into(),
            release: "14".into(),
            api: 34,
        });
        assert_eq!(d.name, "Xiaomi Pad 6S Pro");
        assert_eq!(d.detail, "Android 14 · API 34");
        assert!(d.hint.unwrap().contains("Débogage USB (paramètres de sécurité)"));
    }

    #[test]
    fn retombe_sur_fabricant_et_modele_sans_conseil_pour_les_autres_marques() {
        let d = describe_android(&AndroidProps {
            manufacturer: "samsung".into(),
            model: "SM-X710".into(),
            release: "13".into(),
            api: 33,
            ..Default::default()
        });
        assert_eq!(d.name, "samsung SM-X710");
        assert!(d.hint.is_none());
    }

    #[test]
    fn explique_un_refus_avec_sa_derniere_erreur() {
        let lines = vec!["INFO: starting".to_string(), "ERROR: Could not create encoder".to_string(), "INFO: stopping".to_string()];
        assert_eq!(server_failure_hint(&lines), "Détail : ERROR: Could not create encoder");
        assert_eq!(server_failure_hint(&[]), "");
    }

    #[test]
    fn lit_le_numero_majeur_de_version() {
        assert_eq!(major_version("18.2"), 18);
        assert_eq!(major_version(" 26.1 "), 26);
        assert_eq!(major_version(""), 0);
    }

    #[test]
    fn demande_le_tunnel_a_partir_d_ios_17_et_le_mode_developpeur_a_partir_de_16() {
        assert!(!ios_needs_tunnel("16.7"));
        assert!(ios_needs_tunnel("17.0"));
        assert!(ios_needs_tunnel("26.1"));
        assert!(!ios_has_developer_mode("15.8"));
        assert!(ios_has_developer_mode("16.0"));
    }
}
