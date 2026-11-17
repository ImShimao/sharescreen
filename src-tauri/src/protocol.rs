//! Protocole du serveur scrcpy v5.0.1 (côté Android) et messages de contrôle.
//! Référence : Server.java, DesktopConnection.java, Streamer.java, ControlMessageReader.java.
//! Tous les entiers sont big-endian, sauf mention contraire.

pub const SCRCPY_VERSION: &str = "5.0.1";
pub const CODEC_H264: u32 = 0x6832_3634; // 'h264'
pub const CODEC_OPUS: u32 = 0x6f70_7573; // 'opus'

const PTS_MASK: u64 = (1u64 << 61) - 1;
const FLAG_KEY_FRAME: u64 = 1u64 << 61;
const FLAG_CONFIG: u64 = 1u64 << 62;
const SESSION_BIT: u32 = 0x8000_0000;

#[derive(Debug, Clone, PartialEq)]
pub enum StreamEvent {
    Device(String),
    Codec(u32),
    Session { width: u32, height: u32, client_resize: bool },
    Packet { config: bool, key_frame: bool, pts: u64, data: Vec<u8> },
}

#[derive(Clone, Copy, Debug)]
pub struct ParserOptions {
    /// Le premier octet est factice (uniquement sur le premier socket).
    pub dummy_byte: bool,
    /// Un nom d'appareil de 64 octets précède le codec.
    pub device_meta: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Stage {
    Dummy,
    Device,
    Codec,
    Header,
    Payload,
}

/// Lit un flux scrcpy (vidéo ou audio) à partir de morceaux de taille quelconque.
pub struct StreamParser {
    buf: Vec<u8>,
    stage: Stage,
    pending: Option<(u64, usize)>,
    options: ParserOptions,
}

impl StreamParser {
    pub fn new(options: ParserOptions) -> Self {
        let stage = if options.dummy_byte {
            Stage::Dummy
        } else if options.device_meta {
            Stage::Device
        } else {
            Stage::Codec
        };
        Self { buf: Vec::new(), stage, pending: None, options }
    }

    /// Ajoute des octets reçus et renvoie les événements désormais complets.
    pub fn feed(&mut self, chunk: &[u8]) -> Vec<StreamEvent> {
        self.buf.extend_from_slice(chunk);
        let mut events = Vec::new();
        let mut pos = 0usize;
        loop {
            let avail = self.buf.len() - pos;
            match self.stage {
                Stage::Dummy => {
                    if avail < 1 {
                        break;
                    }
                    pos += 1;
                    self.stage = if self.options.device_meta { Stage::Device } else { Stage::Codec };
                }
                Stage::Device => {
                    if avail < 64 {
                        break;
                    }
                    let raw = &self.buf[pos..pos + 64];
                    let end = raw.iter().position(|&b| b == 0).unwrap_or(64);
                    events.push(StreamEvent::Device(String::from_utf8_lossy(&raw[..end]).into_owned()));
                    pos += 64;
                    self.stage = Stage::Codec;
                }
                Stage::Codec => {
                    if avail < 4 {
                        break;
                    }
                    let mut b = [0u8; 4];
                    b.copy_from_slice(&self.buf[pos..pos + 4]);
                    events.push(StreamEvent::Codec(u32::from_be_bytes(b)));
                    pos += 4;
                    self.stage = Stage::Header;
                }
                Stage::Header => {
                    if avail < 12 {
                        break;
                    }
                    let mut h = [0u8; 12];
                    h.copy_from_slice(&self.buf[pos..pos + 12]);
                    let first = u32::from_be_bytes([h[0], h[1], h[2], h[3]]);
                    if first & SESSION_BIT != 0 {
                        // En-tête de session : flags(4) | largeur(4) | hauteur(4). Bit 0 = redimensionnement client.
                        events.push(StreamEvent::Session {
                            width: u32::from_be_bytes([h[4], h[5], h[6], h[7]]),
                            height: u32::from_be_bytes([h[8], h[9], h[10], h[11]]),
                            client_resize: first & 1 != 0,
                        });
                    } else {
                        let pts = u64::from_be_bytes([h[0], h[1], h[2], h[3], h[4], h[5], h[6], h[7]]);
                        let size = u32::from_be_bytes([h[8], h[9], h[10], h[11]]) as usize;
                        self.pending = Some((pts, size));
                        self.stage = Stage::Payload;
                    }
                    pos += 12;
                }
                Stage::Payload => {
                    let Some((pts, size)) = self.pending else { break };
                    if avail < size {
                        break;
                    }
                    let data = self.buf[pos..pos + size].to_vec();
                    pos += size;
                    events.push(StreamEvent::Packet {
                        config: pts & FLAG_CONFIG != 0,
                        key_frame: pts & FLAG_KEY_FRAME != 0,
                        pts: pts & PTS_MASK,
                        data,
                    });
                    self.pending = None;
                    self.stage = Stage::Header;
                }
            }
        }
        self.buf.drain(..pos);
        events
    }
}

/// Découpe un flux Annex B (00 00 01 / 00 00 00 01) en unités NAL sans codes de début.
pub fn split_annex_b(buf: &[u8]) -> Vec<&[u8]> {
    let mut starts: Vec<usize> = Vec::new();
    let mut i = 0;
    while i + 2 < buf.len() {
        if buf[i] == 0 && buf[i + 1] == 0 && buf[i + 2] == 1 {
            starts.push(i);
            i += 3;
        } else {
            i += 1;
        }
    }
    let mut nals = Vec::new();
    for (k, &at) in starts.iter().enumerate() {
        let from = at + 3;
        let mut end = if k + 1 < starts.len() { starts[k + 1] } else { buf.len() };
        while end > from && buf[end - 1] == 0 {
            end -= 1; // retire le zéro du code 4 octets suivant
        }
        if end > from {
            nals.push(&buf[from..end]);
        }
    }
    nals
}

/// Concatène des NAL au format AVCC (longueur 4 octets big-endian).
pub fn to_avcc(nals: &[&[u8]]) -> Vec<u8> {
    let total: usize = nals.iter().map(|n| n.len() + 4).sum();
    let mut out = Vec::with_capacity(total);
    for nal in nals {
        out.extend_from_slice(&(nal.len() as u32).to_be_bytes());
        out.extend_from_slice(nal);
    }
    out
}

#[derive(Debug, Clone, PartialEq)]
pub struct H264Config {
    /// Ex. "avc1.640033" pour WebCodecs.
    pub codec: String,
    /// Enregistrement avcC.
    pub description: Vec<u8>,
    pub sps: Vec<u8>,
    pub pps: Vec<u8>,
}

/// Extrait SPS et PPS d'un paquet de configuration Annex B et construit l'enregistrement avcC.
pub fn parse_h264_config(config_packet: &[u8]) -> Result<H264Config, String> {
    let nals = split_annex_b(config_packet);
    let sps = nals.iter().find(|n| n[0] & 0x1f == 7).ok_or("SPS absent de la configuration H.264")?;
    let pps = nals.iter().find(|n| n[0] & 0x1f == 8).ok_or("PPS absent de la configuration H.264")?;
    if sps.len() < 4 {
        return Err("SPS H.264 trop court".into());
    }
    let mut description = Vec::with_capacity(11 + sps.len() + pps.len());
    description.push(1); // version
    description.extend_from_slice(&[sps[1], sps[2], sps[3]]); // profil, compatibilité, niveau
    description.push(0xff); // longueurs sur 4 octets
    description.push(0xe1); // un SPS
    description.extend_from_slice(&(sps.len() as u16).to_be_bytes());
    description.extend_from_slice(sps);
    description.push(1); // un PPS
    description.extend_from_slice(&(pps.len() as u16).to_be_bytes());
    description.extend_from_slice(pps);
    Ok(H264Config {
        codec: format!("avc1.{:02x}{:02x}{:02x}", sps[1], sps[2], sps[3]),
        description,
        sps: sps.to_vec(),
        pps: pps.to_vec(),
    })
}

/// Convertit une image Annex B en AVCC ; SPS et PPS sont ajoutés devant les images clés.
pub fn h264_packet_to_chunk(data: &[u8], key_frame: bool, config: Option<&H264Config>) -> Vec<u8> {
    let nals = split_annex_b(data);
    match (key_frame, config) {
        (true, Some(c)) => {
            let mut list: Vec<&[u8]> = vec![c.sps.as_slice(), c.pps.as_slice()];
            list.extend(nals);
            to_avcc(&list)
        }
        _ => to_avcc(&nals),
    }
}

/// Extrait l'en-tête OpusHead d'un paquet de configuration scrcpy (AOPUSHDR, taille natif LE, OpusHead).
pub fn parse_opus_config(packet: &[u8]) -> Result<Vec<u8>, String> {
    if packet.len() < 16 || &packet[0..8] != b"AOPUSHDR" {
        return Err("En-tête Opus introuvable".into());
    }
    let mut b = [0u8; 8];
    b.copy_from_slice(&packet[8..16]);
    let size = u64::from_le_bytes(b) as usize;
    if size == 0 || 16 + size > packet.len() {
        return Err("Taille d'en-tête Opus invalide".into());
    }
    Ok(packet[16..16 + size].to_vec())
}

// ---------------------------------------------------------------------------
// Messages de contrôle (PC -> appareil)
// ---------------------------------------------------------------------------

const TYPE_KEYCODE: u8 = 0;
const TYPE_TEXT: u8 = 1;
const TYPE_TOUCH: u8 = 2;
const TYPE_SCROLL: u8 = 3;
const TYPE_BACK_OR_SCREEN_ON: u8 = 4;
const TYPE_DISPLAY_POWER: u8 = 10;
const TYPE_ROTATE_DEVICE: u8 = 11;

pub const KEY_DOWN: u8 = 0;
pub const KEY_UP: u8 = 1;
pub const MOTION_DOWN: u8 = 0;
pub const MOTION_UP: u8 = 1;
pub const MOTION_MOVE: u8 = 2;
pub const BUTTON_PRIMARY: i32 = 1;
pub const POINTER_ID_MOUSE: i64 = -1;

/// Champ de pression u16 : 1.0 devient 0xffff, comme Binary.u16FixedPointToFloat côté serveur.
pub fn pressure_to_u16(p: f32) -> u16 {
    if p >= 1.0 {
        return 0xffff;
    }
    if p <= 0.0 {
        return 0;
    }
    ((p * 65536.0).floor() as u32).min(0xffff) as u16
}

pub fn encode_key(action: u8, keycode: i32, meta_state: i32) -> Vec<u8> {
    let mut b = Vec::with_capacity(14);
    b.push(TYPE_KEYCODE);
    b.push(action);
    b.extend_from_slice(&keycode.to_be_bytes());
    b.extend_from_slice(&0i32.to_be_bytes()); // répétition
    b.extend_from_slice(&meta_state.to_be_bytes());
    b
}

/// Texte injecté : UTF-8, 300 octets au maximum, coupé sur un caractère complet.
pub fn encode_text(text: &str) -> Vec<u8> {
    let mut bytes = text.as_bytes();
    if bytes.len() > 300 {
        let mut end = 300;
        while end > 0 && (bytes[end] & 0xc0) == 0x80 {
            end -= 1;
        }
        bytes = &bytes[..end];
    }
    let mut b = Vec::with_capacity(5 + bytes.len());
    b.push(TYPE_TEXT);
    b.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
    b.extend_from_slice(bytes);
    b
}

pub struct TouchInput {
    pub action: u8,
    pub pointer_id: i64,
    pub x: f64,
    pub y: f64,
    pub screen_width: u16,
    pub screen_height: u16,
    pub pressure: f32,
    pub action_button: i32,
    pub buttons: i32,
}

pub fn encode_touch(t: &TouchInput) -> Vec<u8> {
    let x = clamp_i32(t.x.round(), t.screen_width as i64 - 1);
    let y = clamp_i32(t.y.round(), t.screen_height as i64 - 1);
    let mut b = Vec::with_capacity(32);
    b.push(TYPE_TOUCH);
    b.push(t.action);
    b.extend_from_slice(&t.pointer_id.to_be_bytes());
    b.extend_from_slice(&x.to_be_bytes());
    b.extend_from_slice(&y.to_be_bytes());
    b.extend_from_slice(&t.screen_width.to_be_bytes());
    b.extend_from_slice(&t.screen_height.to_be_bytes());
    b.extend_from_slice(&pressure_to_u16(t.pressure).to_be_bytes());
    b.extend_from_slice(&t.action_button.to_be_bytes());
    b.extend_from_slice(&t.buttons.to_be_bytes());
    b
}

/// Défilement en crans (≈ ±1 à ±5). Le serveur multiplie la valeur décodée par 16.
pub fn encode_scroll(x: f64, y: f64, screen_width: u16, screen_height: u16, h_scroll: f64, v_scroll: f64) -> Vec<u8> {
    let mut b = Vec::with_capacity(21);
    b.push(TYPE_SCROLL);
    b.extend_from_slice(&clamp_i32(x.round(), screen_width as i64 - 1).to_be_bytes());
    b.extend_from_slice(&clamp_i32(y.round(), screen_height as i64 - 1).to_be_bytes());
    b.extend_from_slice(&screen_width.to_be_bytes());
    b.extend_from_slice(&screen_height.to_be_bytes());
    b.extend_from_slice(&to_i16_fixed(h_scroll / 16.0).to_be_bytes());
    b.extend_from_slice(&to_i16_fixed(v_scroll / 16.0).to_be_bytes());
    b.extend_from_slice(&0i32.to_be_bytes()); // boutons
    b
}

pub fn encode_back_or_screen_on(action: u8) -> Vec<u8> {
    vec![TYPE_BACK_OR_SCREEN_ON, action]
}

pub fn encode_display_power(on: bool) -> Vec<u8> {
    vec![TYPE_DISPLAY_POWER, u8::from(on)]
}

pub fn encode_rotate() -> Vec<u8> {
    vec![TYPE_ROTATE_DEVICE]
}

fn to_i16_fixed(normalized: f64) -> i16 {
    let n = normalized.clamp(-1.0, 1.0);
    if n >= 1.0 {
        return 0x7fff;
    }
    (n * 32768.0).round().clamp(-32768.0, 32767.0) as i16
}

fn clamp_i32(v: f64, max: i64) -> i32 {
    let max = max.max(0);
    (v as i64).clamp(0, max) as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    const CONFIG_FLAG: u64 = 1 << 62;
    const KEY_FLAG: u64 = 1 << 61;
    const SPS: [u8; 8] = [0x67, 0x64, 0x00, 0x33, 0xac, 0xd9, 0x40, 0x78];
    const PPS: [u8; 6] = [0x68, 0xeb, 0xe3, 0xcb, 0x22, 0xc0];
    const IDR: [u8; 5] = [0x65, 0x88, 0x84, 0x00, 0x33];
    const DELTA: [u8; 4] = [0x41, 0x9a, 0x00, 0x11];

    fn annex_b(nals: &[&[u8]]) -> Vec<u8> {
        let mut out = Vec::new();
        for n in nals {
            out.extend_from_slice(&[0, 0, 0, 1]);
            out.extend_from_slice(n);
        }
        out
    }

    fn packet_header(pts_and_flags: u64, size: u32) -> Vec<u8> {
        let mut h = pts_and_flags.to_be_bytes().to_vec();
        h.extend_from_slice(&size.to_be_bytes());
        h
    }

    fn session_header(w: u32, h: u32) -> Vec<u8> {
        let mut b = 0x8000_0000u32.to_be_bytes().to_vec();
        b.extend_from_slice(&w.to_be_bytes());
        b.extend_from_slice(&h.to_be_bytes());
        b
    }

    #[test]
    fn lit_nom_codec_session_et_paquets_en_petits_morceaux() {
        let config = annex_b(&[&SPS, &PPS]);
        let mut name = [0u8; 64];
        name[..6].copy_from_slice(b"Pad 6S");
        let mut stream = vec![0u8]; // octet factice
        stream.extend_from_slice(&name);
        stream.extend_from_slice(b"h264");
        stream.extend(session_header(1080, 2400));
        stream.extend(packet_header(CONFIG_FLAG, config.len() as u32));
        stream.extend(&config);
        stream.extend(packet_header(KEY_FLAG | 1000, IDR.len() as u32));
        stream.extend_from_slice(&IDR);
        stream.extend(packet_header(2000, DELTA.len() as u32));
        stream.extend_from_slice(&DELTA);

        let mut parser = StreamParser::new(ParserOptions { dummy_byte: true, device_meta: true });
        let mut events = Vec::new();
        for chunk in stream.chunks(7) {
            events.extend(parser.feed(chunk));
        }
        assert_eq!(events[0], StreamEvent::Device("Pad 6S".into()));
        assert_eq!(events[1], StreamEvent::Codec(CODEC_H264));
        assert_eq!(events[2], StreamEvent::Session { width: 1080, height: 2400, client_resize: false });
        assert_eq!(
            events[3],
            StreamEvent::Packet { config: true, key_frame: false, pts: 0, data: config.clone() }
        );
        assert_eq!(
            events[4],
            StreamEvent::Packet { config: false, key_frame: true, pts: 1000, data: IDR.to_vec() }
        );
        assert_eq!(
            events[5],
            StreamEvent::Packet { config: false, key_frame: false, pts: 2000, data: DELTA.to_vec() }
        );
        assert_eq!(events.len(), 6);
    }

    #[test]
    fn signale_redimensionnement_client_par_le_bit_zero() {
        let mut header = 0x8000_0001u32.to_be_bytes().to_vec();
        header.extend_from_slice(&1600u32.to_be_bytes());
        header.extend_from_slice(&900u32.to_be_bytes());
        let mut stream = 0x6832_3634u32.to_be_bytes().to_vec(); // codec, sans dummy ni nom
        stream.extend(header);
        let mut parser = StreamParser::new(ParserOptions { dummy_byte: false, device_meta: false });
        let events = parser.feed(&stream);
        assert_eq!(events.last().unwrap(), &StreamEvent::Session { width: 1600, height: 900, client_resize: true });
    }

    #[test]
    fn decoupe_annex_b_et_ignore_le_zero_de_bourrage() {
        let stream = annex_b(&[&SPS, &PPS]);
        let nals = split_annex_b(&stream);
        assert_eq!(nals.len(), 2);
        assert_eq!(nals[0], &SPS[..]);
        assert_eq!(nals[1], &PPS[..]);
    }

    #[test]
    fn construit_codec_webcodecs_et_avcc() {
        let config = parse_h264_config(&annex_b(&[&SPS, &PPS])).unwrap();
        assert_eq!(config.codec, "avc1.640033");
        assert_eq!(config.description[0], 1);
        assert_eq!(config.description[1], 0x64);
        assert_eq!(config.description[3], 0x33);
        assert_eq!(u16::from_be_bytes([config.description[6], config.description[7]]) as usize, SPS.len());
        assert_eq!(config.description[8 + SPS.len()], 1);
        assert_eq!(
            u16::from_be_bytes([config.description[9 + SPS.len()], config.description[10 + SPS.len()]]) as usize,
            PPS.len()
        );
        assert_eq!(config.description.len(), 11 + SPS.len() + PPS.len());
    }

    #[test]
    fn place_sps_et_pps_devant_chaque_image_cle() {
        let config = parse_h264_config(&annex_b(&[&SPS, &PPS])).unwrap();
        let key = h264_packet_to_chunk(&annex_b(&[&IDR]), true, Some(&config));
        assert_eq!(key, to_avcc(&[&SPS, &PPS, &IDR]));
        let delta = h264_packet_to_chunk(&annex_b(&[&DELTA]), false, Some(&config));
        assert_eq!(delta, to_avcc(&[&DELTA]));
    }

    #[test]
    fn refuse_une_configuration_incomplete() {
        assert!(parse_h264_config(&annex_b(&[&SPS])).is_err());
    }

    #[test]
    fn extrait_opus_head() {
        let opus_head: Vec<u8> = [b"OpusHead".as_slice(), &[1, 1, 0x38, 0x01, 0x80, 0xbb, 0, 0, 0, 0, 0]].concat();
        let mut packet = b"AOPUSHDR".to_vec();
        packet.extend_from_slice(&(opus_head.len() as u64).to_le_bytes());
        packet.extend_from_slice(&opus_head);
        packet.extend_from_slice(b"AOPUSDLY");
        assert_eq!(parse_opus_config(&packet).unwrap(), opus_head);
    }

    #[test]
    fn encode_une_touche_sur_14_octets() {
        let b = encode_key(KEY_DOWN, 4, 1);
        assert_eq!(b.len(), 14);
        assert_eq!(b[0], 0);
        assert_eq!(b[1], KEY_DOWN);
        assert_eq!(i32::from_be_bytes([b[2], b[3], b[4], b[5]]), 4);
        assert_eq!(i32::from_be_bytes([b[10], b[11], b[12], b[13]]), 1);
    }

    #[test]
    fn encode_un_toucher_sur_32_octets() {
        let b = encode_touch(&TouchInput {
            action: MOTION_DOWN,
            pointer_id: POINTER_ID_MOUSE,
            x: 100.0,
            y: 200.0,
            screen_width: 1000,
            screen_height: 2000,
            pressure: 1.0,
            action_button: BUTTON_PRIMARY,
            buttons: BUTTON_PRIMARY,
        });
        assert_eq!(b.len(), 32);
        assert_eq!(b[0], 2);
        assert_eq!(i64::from_be_bytes(b[2..10].try_into().unwrap()), -1);
        assert_eq!(i32::from_be_bytes(b[10..14].try_into().unwrap()), 100);
        assert_eq!(i32::from_be_bytes(b[14..18].try_into().unwrap()), 200);
        assert_eq!(u16::from_be_bytes([b[22], b[23]]), 0xffff);
        assert_eq!(i32::from_be_bytes(b[24..28].try_into().unwrap()), BUTTON_PRIMARY);
    }

    #[test]
    fn borne_les_coordonnees_a_l_ecran() {
        let b = encode_touch(&TouchInput {
            action: MOTION_MOVE,
            pointer_id: POINTER_ID_MOUSE,
            x: 5000.0,
            y: -4.0,
            screen_width: 1000,
            screen_height: 2000,
            pressure: 0.0,
            action_button: 0,
            buttons: 0,
        });
        assert_eq!(i32::from_be_bytes(b[10..14].try_into().unwrap()), 999);
        assert_eq!(i32::from_be_bytes(b[14..18].try_into().unwrap()), 0);
    }

    #[test]
    fn un_cran_de_defilement_vaut_un_seizieme_de_plage() {
        let b = encode_scroll(10.0, 20.0, 1000, 2000, 0.0, -16.0);
        assert_eq!(b.len(), 21);
        assert_eq!(b[0], 3);
        assert_eq!(i16::from_be_bytes([b[15], b[16]]), -32768);
        let up = encode_scroll(0.0, 0.0, 10, 10, 0.0, 1.0);
        assert_eq!(i16::from_be_bytes([up[15], up[16]]), 2048);
    }

    #[test]
    fn coupe_le_texte_a_300_octets_sans_casser_un_caractere() {
        let text = format!("a{}", "€".repeat(120)); // '€' = 3 octets
        let b = encode_text(&text);
        assert_eq!(b[0], 1);
        assert_eq!(u32::from_be_bytes([b[1], b[2], b[3], b[4]]), 298);
        assert_eq!(b.len(), 5 + 298);
        assert_eq!(std::str::from_utf8(&b[5..]).unwrap(), format!("a{}", "€".repeat(99)));
    }

    #[test]
    fn convertit_la_pression() {
        assert_eq!(pressure_to_u16(1.0), 0xffff);
        assert_eq!(pressure_to_u16(0.5), 32768);
        assert_eq!(pressure_to_u16(0.0), 0);
    }

    #[test]
    fn encode_retour_et_alimentation() {
        assert_eq!(encode_back_or_screen_on(0), vec![4, 0]);
        assert_eq!(encode_display_power(true), vec![10, 1]);
        assert_eq!(encode_rotate(), vec![11]);
    }
}
