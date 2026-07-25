//! PAdES-B-B PDF signing.
//!
//! Strategy: add an (invisible) signature field to the document, serialize it
//! with fixed-size placeholders for `/ByteRange` and `/Contents`, then patch
//! those bytes in place so the byte offsets stay exact. The CMS/PKCS#7 blob is
//! produced by [`crate::cie::cms`] with the raw signature done on the CIE chip.

use lopdf::{Dictionary, Document, Object, Stream, StringFormat};
use serde::Deserialize;

use crate::cie::cms;
use crate::cie::error::{CieError, CieResult};
use crate::cie::pkcs11::CardSigner;

/// A visible signature appearance: where to draw the "stamp" and what text to
/// show. Coordinates are in PDF points with origin at the bottom-left of the
/// page (the same space the frontend stores placements in).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignatureAppearance {
    /// 0-based page index the box sits on.
    pub page_index: usize,
    /// `[x0, y0, x1, y1]` rectangle in PDF points (bottom-left origin).
    pub rect: [f32; 4],
    /// Text lines drawn top-to-bottom inside the box.
    pub lines: Vec<String>,
}

/// Reserved size (in bytes) for the CMS blob inside `/Contents`. Encoded as
/// twice as many hex characters. A CIE PAdES-B-B signature (one RSA-2048 signer
/// + one certificate) is a few KB, so 16 KiB is comfortably safe.
const CONTENTS_BYTES: usize = 16384;

/// 10-digit placeholder used for each of the last three `/ByteRange` entries so
/// there is always room to patch in the real offsets afterwards.
const BR_PLACEHOLDER: i64 = 1_000_000_000;

/// Sign `pdf` with the CIE and return the signed PDF bytes.
pub fn sign_pdf(
    pdf: &[u8],
    module_path: &str,
    slot_id: u64,
    cert_id_hex: &str,
    pin: &str,
    reason: Option<&str>,
    location: Option<&str>,
    appearance: Option<SignatureAppearance>,
) -> CieResult<Vec<u8>> {
    let signer = CardSigner::open(module_path, slot_id, cert_id_hex, pin)?;

    let mut buf = build_placeholder_pdf(pdf, reason, location, appearance.as_ref())?;
    let (contents_lt, contents_gt) = find_contents_span(&buf)?;

    // ByteRange covers everything except the hex between < and >.
    let br = [
        0i64,
        (contents_lt + 1) as i64,
        contents_gt as i64,
        (buf.len() - contents_gt) as i64,
    ];
    patch_byte_range(&mut buf, br)?;

    // Digest over the two signed ranges (ByteRange is already final).
    let digest = sha256_ranges(&buf, contents_lt + 1, contents_gt);

    let cms_der = cms::build_detached_pkcs7(signer.certificate_der(), &digest, |tbs| signer.sign(tbs))?;

    embed_contents(&mut buf, contents_lt, contents_gt, &cms_der)?;
    Ok(buf)
}

fn build_placeholder_pdf(
    pdf: &[u8],
    reason: Option<&str>,
    location: Option<&str>,
    appearance: Option<&SignatureAppearance>,
) -> CieResult<Vec<u8>> {
    let mut doc = Document::load_mem(pdf)?;

    let root_id = doc
        .trailer
        .get(b"Root")
        .map_err(CieError::from)?
        .as_reference()
        .map_err(CieError::from)?;

    // Pages in display order (BTreeMap keyed by 1-based page number).
    let page_ids: Vec<lopdf::ObjectId> = doc.get_pages().into_values().collect();
    // Attach the widget to the page carrying the visible box (default: first).
    let target_page = appearance.map(|a| a.page_index).unwrap_or(0);
    let page_id = *page_ids
        .get(target_page)
        .or_else(|| page_ids.first())
        .ok_or_else(|| CieError::Pdf("PDF has no pages".into()))?;

    // Build the visible appearance form XObject up-front (needs its own object
    // ids), so we can reference it from the widget's /AP below.
    let appearance_ref = match appearance {
        Some(ap) => Some(build_appearance_xobject(&mut doc, ap)?),
        None => None,
    };
    // Widget rectangle: the visible box, or a zero-size (invisible) rect.
    let rect = appearance
        .map(|a| a.rect)
        .unwrap_or([0.0, 0.0, 0.0, 0.0]);

    // --- Signature dictionary ---
    let mut sig = Dictionary::new();
    sig.set("Type", Object::Name(b"Sig".to_vec()));
    sig.set("Filter", Object::Name(b"Adobe.PPKLite".to_vec()));
    sig.set("SubFilter", Object::Name(b"ETSI.CAdES.detached".to_vec()));
    sig.set(
        "ByteRange",
        Object::Array(vec![
            Object::Integer(0),
            Object::Integer(BR_PLACEHOLDER),
            Object::Integer(BR_PLACEHOLDER),
            Object::Integer(BR_PLACEHOLDER),
        ]),
    );
    sig.set(
        "Contents",
        Object::String(vec![0u8; CONTENTS_BYTES], StringFormat::Hexadecimal),
    );
    sig.set(
        "M",
        Object::String(pdf_date_now().into_bytes(), StringFormat::Literal),
    );
    if let Some(r) = reason {
        sig.set("Reason", Object::String(r.as_bytes().to_vec(), StringFormat::Literal));
    }
    if let Some(l) = location {
        sig.set("Location", Object::String(l.as_bytes().to_vec(), StringFormat::Literal));
    }
    let sig_id = doc.add_object(sig);

    // --- Invisible signature field / widget ---
    let mut field = Dictionary::new();
    field.set("Type", Object::Name(b"Annot".to_vec()));
    field.set("Subtype", Object::Name(b"Widget".to_vec()));
    field.set("FT", Object::Name(b"Sig".to_vec()));
    field.set(
        "Rect",
        Object::Array(vec![
            Object::Real(rect[0]),
            Object::Real(rect[1]),
            Object::Real(rect[2]),
            Object::Real(rect[3]),
        ]),
    );
    field.set(
        "T",
        Object::String(b"Signature1".to_vec(), StringFormat::Literal),
    );
    field.set("V", Object::Reference(sig_id));
    field.set("P", Object::Reference(page_id));
    field.set("F", Object::Integer(132)); // Print (4) + Locked (128)
    if let Some(xobj_id) = appearance_ref {
        let mut ap = Dictionary::new();
        ap.set("N", Object::Reference(xobj_id));
        field.set("AP", Object::Dictionary(ap));
    }
    let field_id = doc.add_object(field);

    // --- Attach widget to the page's /Annots ---
    {
        let page = doc
            .get_object_mut(page_id)
            .map_err(CieError::from)?
            .as_dict_mut()
            .map_err(CieError::from)?;
        match page.get(b"Annots") {
            Ok(Object::Array(existing)) => {
                let mut arr = existing.clone();
                arr.push(Object::Reference(field_id));
                page.set("Annots", Object::Array(arr));
            }
            _ => {
                page.set("Annots", Object::Array(vec![Object::Reference(field_id)]));
            }
        }
    }

    // --- AcroForm on the catalog (merge if one already exists) ---
    {
        let existing_acroform = doc
            .get_object(root_id)
            .ok()
            .and_then(|o| o.as_dict().ok())
            .and_then(|d| d.get(b"AcroForm").ok())
            .and_then(|o| o.as_reference().ok());

        if let Some(acro_id) = existing_acroform {
            let acro = doc
                .get_object_mut(acro_id)
                .map_err(CieError::from)?
                .as_dict_mut()
                .map_err(CieError::from)?;
            match acro.get(b"Fields") {
                Ok(Object::Array(existing)) => {
                    let mut arr = existing.clone();
                    arr.push(Object::Reference(field_id));
                    acro.set("Fields", Object::Array(arr));
                }
                _ => acro.set("Fields", Object::Array(vec![Object::Reference(field_id)])),
            }
            acro.set("SigFlags", Object::Integer(3));
        } else {
            let mut acro = Dictionary::new();
            acro.set("Fields", Object::Array(vec![Object::Reference(field_id)]));
            acro.set("SigFlags", Object::Integer(3));
            let acro_id = doc.add_object(acro);
            let catalog = doc
                .get_object_mut(root_id)
                .map_err(CieError::from)?
                .as_dict_mut()
                .map_err(CieError::from)?;
            catalog.set("AcroForm", Object::Reference(acro_id));
        }
    }

    let mut out = Vec::new();
    doc.save_to(&mut out)
        .map_err(|e| CieError::Pdf(e.to_string()))?;
    Ok(out)
}

/// Build a Form XObject that draws the visible signature box (a thin border
/// plus the given text lines in Helvetica) and return its object id, ready to
/// be referenced from the widget's `/AP /N`.
fn build_appearance_xobject(
    doc: &mut Document,
    ap: &SignatureAppearance,
) -> CieResult<lopdf::ObjectId> {
    let w = (ap.rect[2] - ap.rect[0]).abs().max(1.0);
    let h = (ap.rect[3] - ap.rect[1]).abs().max(1.0);

    // Helvetica (standard 14 font — no embedding needed).
    let mut font = Dictionary::new();
    font.set("Type", Object::Name(b"Font".to_vec()));
    font.set("Subtype", Object::Name(b"Type1".to_vec()));
    font.set("BaseFont", Object::Name(b"Helvetica".to_vec()));
    let font_id = doc.add_object(font);

    // Fit the lines vertically inside the box.
    let n = ap.lines.len().max(1) as f32;
    let leading = ((h - 6.0) / n).clamp(6.0, 16.0);
    let size = (leading * 0.82).clamp(5.0, 12.0);

    let mut content = String::new();
    content.push_str("q\n");
    // Thin slate-grey border just inside the box edge.
    content.push_str("0.30 0.36 0.46 RG\n0.8 w\n");
    content.push_str(&format!("0.5 0.5 {:.2} {:.2} re\nS\n", w - 1.0, h - 1.0));
    // Text.
    content.push_str("0.12 0.16 0.24 rg\n");
    content.push_str("BT\n");
    content.push_str(&format!("/F1 {size:.2} Tf\n"));
    let first_baseline = h - 3.0 - size;
    content.push_str(&format!("1 0 0 1 4.00 {first_baseline:.2} Tm\n"));
    for (i, line) in ap.lines.iter().enumerate() {
        if i > 0 {
            content.push_str(&format!("0 {:.2} Td\n", -leading));
        }
        content.push_str(&format!("({}) Tj\n", escape_pdf_text(line)));
    }
    content.push_str("ET\nQ\n");

    let mut fonts = Dictionary::new();
    fonts.set("F1", Object::Reference(font_id));
    let mut resources = Dictionary::new();
    resources.set("Font", Object::Dictionary(fonts));

    let mut xdict = Dictionary::new();
    xdict.set("Type", Object::Name(b"XObject".to_vec()));
    xdict.set("Subtype", Object::Name(b"Form".to_vec()));
    xdict.set("FormType", Object::Integer(1));
    xdict.set(
        "BBox",
        Object::Array(vec![
            Object::Real(0.0),
            Object::Real(0.0),
            Object::Real(w),
            Object::Real(h),
        ]),
    );
    xdict.set("Resources", Object::Dictionary(resources));

    let stream = Stream::new(xdict, content.into_bytes());
    Ok(doc.add_object(Object::Stream(stream)))
}

/// Escape a string for use inside a PDF literal string `( ... )`.
fn escape_pdf_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '(' => out.push_str("\\("),
            ')' => out.push_str("\\)"),
            '\r' => out.push_str("\\r"),
            '\n' => out.push_str("\\n"),
            // Non-ASCII: PDFDocEncoding/WinAnsi is a superset of Latin-1 for the
            // common accented characters we expect in Italian names.
            c if (c as u32) <= 0xFF => out.push(c),
            _ => out.push('?'),
        }
    }
    out
}

/// Locate the `<` and `>` byte offsets of the signature `/Contents` hex string.
///
/// Anchored to the unique `/ByteRange` key (present only in the signature dict,
/// which we just added), so a page's `/Contents 5 0 R` is never mistaken for it.
/// Our sig dict serializes `/ByteRange` immediately before `/Contents`.
fn find_contents_span(buf: &[u8]) -> CieResult<(usize, usize)> {
    let byte_range = find(buf, b"/ByteRange", 0)
        .ok_or_else(|| CieError::Pdf("/ByteRange not found in serialized PDF".into()))?;
    let contents = find(buf, b"/Contents", byte_range)
        .ok_or_else(|| CieError::Pdf("/Contents not found after /ByteRange".into()))?;
    let lt = find(buf, b"<", contents)
        .ok_or_else(|| CieError::Pdf("Contents opening '<' not found".into()))?;
    let gt = find(buf, b">", lt)
        .ok_or_else(|| CieError::Pdf("Contents closing '>' not found".into()))?;
    Ok((lt, gt))
}

/// Overwrite the `/ByteRange` array in place, preserving its byte length.
fn patch_byte_range(buf: &mut [u8], br: [i64; 4]) -> CieResult<()> {
    let key = find(buf, b"/ByteRange", 0)
        .ok_or_else(|| CieError::Pdf("/ByteRange not found".into()))?;
    let lb = find(buf, b"[", key).ok_or_else(|| CieError::Pdf("ByteRange '[' not found".into()))?;
    let rb = find(buf, b"]", lb).ok_or_else(|| CieError::Pdf("ByteRange ']' not found".into()))?;

    let interior_len = rb - (lb + 1);
    let mut replacement = format!("{} {} {} {}", br[0], br[1], br[2], br[3]).into_bytes();
    if replacement.len() > interior_len {
        return Err(CieError::Pdf("ByteRange placeholder too small".into()));
    }
    replacement.resize(interior_len, b' ');
    buf[lb + 1..rb].copy_from_slice(&replacement);
    Ok(())
}

/// SHA-256 over `buf[0..end1]` concatenated with `buf[start2..]`.
fn sha256_ranges(buf: &[u8], end1: usize, start2: usize) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(&buf[0..end1]);
    h.update(&buf[start2..]);
    h.finalize().into()
}

/// Write the CMS blob (hex, zero-padded) between the `<` and `>` of `/Contents`.
fn embed_contents(buf: &mut [u8], lt: usize, gt: usize, cms_der: &[u8]) -> CieResult<()> {
    let hex_str = hex::encode(cms_der);
    let capacity = gt - (lt + 1);
    if hex_str.len() > capacity {
        return Err(CieError::PlaceholderTooSmall {
            needed: hex_str.len(),
            available: capacity,
        });
    }
    let mut bytes = hex_str.into_bytes();
    bytes.resize(capacity, b'0');
    buf[lt + 1..gt].copy_from_slice(&bytes);
    Ok(())
}

fn pdf_date_now() -> String {
    use chrono::{Local, Offset};
    let now = Local::now();
    let offset_seconds = now.offset().fix().local_minus_utc();
    let sign = if offset_seconds >= 0 { '+' } else { '-' };
    let off = offset_seconds.abs();
    format!(
        "D:{}{}{:02}'{:02}'",
        now.format("%Y%m%d%H%M%S"),
        sign,
        off / 3600,
        (off % 3600) / 60
    )
}

/// First index of `needle` in `hay` at or after `from`.
fn find(hay: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    if needle.is_empty() || from >= hay.len() {
        return None;
    }
    hay[from..]
        .windows(needle.len())
        .position(|w| w == needle)
        .map(|p| p + from)
}
