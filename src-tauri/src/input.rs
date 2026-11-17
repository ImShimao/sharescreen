//! Traduit les événements de l'interface en messages de contrôle scrcpy.
//! Les coordonnées de l'interface sont normalisées (0..1) ; elles deviennent des pixels de l'écran.

use serde::Deserialize;

use crate::protocol::{
    encode_back_or_screen_on, encode_display_power, encode_key, encode_rotate, encode_scroll, encode_text, encode_touch,
    TouchInput, BUTTON_PRIMARY, KEY_DOWN, KEY_UP, MOTION_DOWN, MOTION_MOVE, MOTION_UP, POINTER_ID_MOUSE,
};

#[derive(Debug, Clone, Copy, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum TouchAction {
    Down,
    Move,
    Up,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum InputEvent {
    Touch { action: TouchAction, x: f64, y: f64 },
    Scroll { x: f64, y: f64, h: f64, v: f64 },
    Key { keycode: i32, meta: Option<i32> },
    Text { text: String },
    Back,
    Power { on: bool },
    Rotate,
}

fn clamp01(v: f64) -> f64 {
    if v.is_finite() {
        v.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

/// Messages à envoyer, dans l'ordre, pour un événement donné et une taille d'écran donnée.
pub fn input_to_messages(event: &InputEvent, width: u32, height: u32) -> Vec<Vec<u8>> {
    if width == 0 || height == 0 {
        return Vec::new();
    }
    let (w, h) = (width as u16, height as u16);
    let (fw, fh) = (width as f64, height as f64);
    match event {
        InputEvent::Touch { action, x, y } => {
            let base = TouchInput {
                action: MOTION_DOWN,
                pointer_id: POINTER_ID_MOUSE,
                x: clamp01(*x) * fw,
                y: clamp01(*y) * fh,
                screen_width: w,
                screen_height: h,
                pressure: 1.0,
                action_button: BUTTON_PRIMARY,
                buttons: BUTTON_PRIMARY,
            };
            let msg = match action {
                TouchAction::Down => TouchInput { action: MOTION_DOWN, ..base },
                TouchAction::Move => TouchInput { action: MOTION_MOVE, action_button: 0, ..base },
                TouchAction::Up => TouchInput { action: MOTION_UP, pressure: 0.0, buttons: 0, ..base },
            };
            vec![encode_touch(&msg)]
        }
        InputEvent::Scroll { x, y, h: hs, v: vs } => {
            vec![encode_scroll(clamp01(*x) * fw, clamp01(*y) * fh, w, h, *hs, *vs)]
        }
        InputEvent::Key { keycode, meta } => {
            let meta = meta.unwrap_or(0);
            vec![encode_key(KEY_DOWN, *keycode, meta), encode_key(KEY_UP, *keycode, meta)]
        }
        InputEvent::Text { text } => {
            if text.is_empty() {
                Vec::new()
            } else {
                vec![encode_text(text)]
            }
        }
        InputEvent::Back => vec![encode_back_or_screen_on(0), encode_back_or_screen_on(1)],
        InputEvent::Power { on } => vec![encode_display_power(*on)],
        InputEvent::Rotate => vec![encode_rotate()],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn convertit_des_coordonnees_normalisees_en_pixels() {
        let size = (1000, 2000);
        let down = input_to_messages(&InputEvent::Touch { action: TouchAction::Down, x: 0.5, y: 0.25 }, size.0, size.1);
        assert_eq!(down[0][1], MOTION_DOWN);
        assert_eq!(i32::from_be_bytes(down[0][10..14].try_into().unwrap()), 500);
        assert_eq!(i32::from_be_bytes(down[0][14..18].try_into().unwrap()), 500);

        let mv = input_to_messages(&InputEvent::Touch { action: TouchAction::Move, x: 0.5, y: 0.5 }, size.0, size.1);
        assert_eq!(mv[0][1], MOTION_MOVE);

        let up = input_to_messages(&InputEvent::Touch { action: TouchAction::Up, x: 0.5, y: 0.5 }, size.0, size.1);
        assert_eq!(up[0][1], MOTION_UP);
        assert_eq!(u16::from_be_bytes([up[0][22], up[0][23]]), 0);
    }

    #[test]
    fn ne_produit_rien_sans_taille_ou_sans_texte() {
        let touch = InputEvent::Touch { action: TouchAction::Down, x: 0.0, y: 0.0 };
        assert!(input_to_messages(&touch, 0, 0).is_empty());
        assert!(input_to_messages(&InputEvent::Text { text: String::new() }, 10, 10).is_empty());
    }

    #[test]
    fn envoie_une_touche_en_appui_puis_relachement() {
        let msgs = input_to_messages(&InputEvent::Key { keycode: 66, meta: None }, 10, 10);
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[0][1], KEY_DOWN);
        assert_eq!(msgs[1][1], KEY_UP);
        assert_eq!(i32::from_be_bytes(msgs[0][2..6].try_into().unwrap()), 66);
    }

    #[test]
    fn lit_les_evenements_envoyes_par_l_interface() {
        let e: InputEvent = serde_json::from_str(r#"{"type":"touch","action":"down","x":0.1,"y":0.2}"#).unwrap();
        assert_eq!(e, InputEvent::Touch { action: TouchAction::Down, x: 0.1, y: 0.2 });
        let k: InputEvent = serde_json::from_str(r#"{"type":"key","keycode":4,"meta":1}"#).unwrap();
        assert_eq!(k, InputEvent::Key { keycode: 4, meta: Some(1) });
        let r: InputEvent = serde_json::from_str(r#"{"type":"rotate"}"#).unwrap();
        assert_eq!(r, InputEvent::Rotate);
    }
}
