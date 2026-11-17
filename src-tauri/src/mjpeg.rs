//! Extrait les images JPEG d'un flux multipart (go-ios : boundary "BoundaryString", Content-Length).

fn find(hay: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    if from >= hay.len() || needle.is_empty() || hay.len() < needle.len() {
        return None;
    }
    hay[from..].windows(needle.len()).position(|w| w == needle).map(|p| p + from)
}

/// Renvoie les images complètes et la position jusqu'à laquelle le tampon a été consommé.
pub fn extract_jpeg_frames(buf: &[u8]) -> (Vec<&[u8]>, usize) {
    let mut frames = Vec::new();
    let mut pos = 0usize;
    loop {
        let Some(hdr) = find(buf, b"Content-Length:", pos) else { break };
        let Some(line_end) = find(buf, b"\r\n", hdr) else { break };
        let value = String::from_utf8_lossy(&buf[hdr + 15..line_end]);
        let Ok(length) = value.trim().parse::<usize>() else { break };
        let Some(headers_end) = find(buf, b"\r\n\r\n", line_end) else { break };
        let start = headers_end + 4;
        if buf.len() < start + length {
            break;
        }
        frames.push(&buf[start..start + length]);
        pos = start + length;
    }
    (frames, pos)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn part(body: &str) -> Vec<u8> {
        format!(
            "--BoundaryString\r\nContent-type: image/jpg\r\nContent-Length: {}\r\n\r\n{}\r\n\r\n",
            body.len(),
            body
        )
        .into_bytes()
    }

    #[test]
    fn extrait_chaque_image_complete() {
        let stream = [part("AAAA"), part("BBBBBB")].concat();
        let (frames, pos) = extract_jpeg_frames(&stream);
        assert_eq!(frames, vec![b"AAAA".as_slice(), b"BBBBBB".as_slice()]);
        assert_eq!(pos, stream.len() - 4); // reste le CRLF final
    }

    #[test]
    fn garde_une_image_incomplete_jusqu_au_morceau_suivant() {
        let full = [part("CCCCCCCC"), part("DDDDDD")].concat();
        let cut = full.len() - 9;
        let (first, pos) = extract_jpeg_frames(&full[..cut]);
        assert_eq!(first, vec![b"CCCCCCCC".as_slice()]);

        let rest = &full[pos..cut];
        let mut joined = rest.to_vec();
        joined.extend_from_slice(&full[cut..]);
        let (second, _) = extract_jpeg_frames(&joined);
        assert_eq!(second, vec![b"DDDDDD".as_slice()]);
    }

    #[test]
    fn ne_renvoie_rien_sans_en_tete_complet() {
        let (frames, pos) = extract_jpeg_frames(b"--BoundaryString\r\nContent-Length: 4");
        assert!(frames.is_empty());
        assert_eq!(pos, 0);
    }
}
