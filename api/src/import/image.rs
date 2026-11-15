//! Photos d'import (PRD §8) : vérification du type réel par nombres magiques
//! (jamais le nom du fichier), puis ré-encodage systématique — supprime
//! EXIF/métadonnées, neutralise les charges utiles cachées et borne la taille
//! envoyée au fournisseur d'extraction.

use std::io::Cursor;

use image::codecs::jpeg::JpegEncoder;
use image::{DynamicImage, ImageDecoder, ImageReader, Limits};

const MAX_SIDE: u32 = 1600;
const JPEG_QUALITY: u8 = 80;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageKind {
    Jpeg,
    Png,
    Webp,
}

pub fn sniff(data: &[u8]) -> Option<ImageKind> {
    if data.len() > 3 && data[..3] == [0xFF, 0xD8, 0xFF] {
        return Some(ImageKind::Jpeg);
    }
    if data.len() > 8 && data[..4] == [0x89, 0x50, 0x4E, 0x47] {
        return Some(ImageKind::Png);
    }
    if data.len() > 12 && &data[..4] == b"RIFF" && &data[8..12] == b"WEBP" {
        return Some(ImageKind::Webp);
    }
    None
}

/// Plafonds de décodage (bombes de décompression, PNG 10000×10000 très
/// compressible…) : l'image décodée tient en mémoire sous la limite du
/// conteneur (deploy/serveur-perso/service.security.yml). 50 Mpx couvre les
/// capteurs de téléphone usuels.
const MAX_DIMENSION: u32 = 12_000;
const MAX_PIXELS: u64 = 50_000_000;
const MAX_DECODE_BYTES: u64 = 160 * 1024 * 1024;

/// Décode (borné), réduit à 1600 px max, applique l'orientation EXIF et
/// ré-encode en JPEG qualité 80 (calcul coûteux : hors du runtime async).
pub fn reencode(data: &[u8]) -> Result<Vec<u8>, String> {
    let mut reader = ImageReader::new(Cursor::new(data))
        .with_guessed_format()
        .map_err(|e| e.to_string())?;
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_DIMENSION);
    limits.max_image_height = Some(MAX_DIMENSION);
    limits.max_alloc = Some(MAX_DECODE_BYTES);
    reader.limits(limits);
    let mut decoder = reader.into_decoder().map_err(|e| e.to_string())?;
    let (width, height) = decoder.dimensions();
    if u64::from(width) * u64::from(height) > MAX_PIXELS {
        return Err("image trop grande".to_string());
    }
    let orientation = decoder.orientation().map_err(|e| e.to_string())?;
    let mut picture = DynamicImage::from_decoder(decoder).map_err(|e| e.to_string())?;

    // Réduction AVANT rotation : la rotation porte alors sur la petite image.
    // `thumbnail` (moyenne par zones, entière) n'alloue que la sortie — pas de
    // tampon f32 pleine largeur comme un rééchantillonnage Lanczos.
    if picture.width() > MAX_SIDE || picture.height() > MAX_SIDE {
        picture = picture.thumbnail(MAX_SIDE, MAX_SIDE);
    }
    picture.apply_orientation(orientation);

    let rgb = picture.to_rgb8();
    let mut output = Vec::new();
    JpegEncoder::new_with_quality(&mut output, JPEG_QUALITY)
        .encode_image(&rgb)
        .map_err(|e| e.to_string())?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageFormat, RgbImage};

    fn png(width: u32, height: u32) -> Vec<u8> {
        let mut bytes = Vec::new();
        RgbImage::new(width, height)
            .write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png)
            .unwrap();
        bytes
    }

    #[test]
    fn nombres_magiques() {
        assert_eq!(sniff(&png(2, 2)), Some(ImageKind::Png));
        assert_eq!(sniff(&[0xFF, 0xD8, 0xFF, 0xE0, 0, 0]), Some(ImageKind::Jpeg));
        assert_eq!(sniff(b"RIFF\0\0\0\0WEBPVP8 "), Some(ImageKind::Webp));
        assert_eq!(sniff(b"<svg onload=alert(1)>"), None);
    }

    #[test]
    fn reencodage_en_jpeg_reduit() {
        let output = reencode(&png(3200, 800)).unwrap();
        assert_eq!(sniff(&output), Some(ImageKind::Jpeg));
        let decoded = image::load_from_memory(&output).unwrap();
        assert_eq!((decoded.width(), decoded.height()), (1600, 400));
    }

    #[test]
    fn pas_d_agrandissement() {
        let decoded = image::load_from_memory(&reencode(&png(100, 50)).unwrap()).unwrap();
        assert_eq!((decoded.width(), decoded.height()), (100, 50));
    }

    #[test]
    fn image_corrompue_refusee() {
        assert!(reencode(&[0x89, 0x50, 0x4E, 0x47, 0, 0, 0, 0, 0, 0]).is_err());
    }

    #[test]
    fn bombe_de_decompression_refusee() {
        // 56 Mpx unis : quelques Ko en PNG, 168 Mo une fois décodés en RGB.
        let mut bytes = Vec::new();
        image::GrayImage::new(8_000, 7_000)
            .write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png)
            .unwrap();
        assert!(bytes.len() < 1_000_000);
        assert!(reencode(&bytes).is_err());
    }

    #[test]
    fn photo_portrait_reduite_puis_orientee() {
        let decoded = image::load_from_memory(&reencode(&png(1200, 4800)).unwrap()).unwrap();
        assert_eq!((decoded.width(), decoded.height()), (400, 1600));
    }
}
