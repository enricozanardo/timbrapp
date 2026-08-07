//! PAdES-B-B PDF signing.
//!
//! Strategy: add an (invisible) signature field to the document, serialize it
//! with fixed-size placeholders for `/ByteRange` and `/Contents`, then patch
//! those bytes in place so the byte offsets stay exact. The CMS/PKCS#7 blob is
//! produced by [`crate::cie::cms`] with the raw signature done on the CIE chip.

use lopdf::{Dictionary, Document, IncrementalDocument, Object, ObjectId, Stream, StringFormat};
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

/// 12-digit placeholder used for each `/ByteRange` entry so there is always
/// room to patch in the real offsets afterwards (covers PDFs up to ~1 TiB).
const BR_PLACEHOLDER: i64 = 100_000_000_000;

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
    let cert_der = signer.certificate_der().to_vec();
    sign_pdf_with(
        pdf,
        &cert_der,
        |tbs| signer.sign(tbs),
        reason,
        location,
        appearance,
    )
}

/// Signer-agnostic PAdES core: everything except where the key lives.
///
/// `sign` receives the DER-encoded CMS signed attributes and must return a
/// PKCS#1 v1.5 signature over their SHA-256.
fn sign_pdf_with<F>(
    pdf: &[u8],
    cert_der: &[u8],
    sign: F,
    reason: Option<&str>,
    location: Option<&str>,
    appearance: Option<SignatureAppearance>,
) -> CieResult<Vec<u8>>
where
    F: FnOnce(&[u8]) -> CieResult<Vec<u8>>,
{
    // A PDF that already carries a signature must be extended with an
    // *incremental update* so the existing signatures' signed byte ranges stay
    // untouched. Signing an unsigned PDF uses the simpler full rewrite.
    let already_signed = check_existing_signatures(pdf)?;
    let mut buf = if already_signed {
        build_incremental_pdf(pdf, reason, location, appearance.as_ref())?.0
    } else {
        build_placeholder_pdf(pdf, reason, location, appearance.as_ref())?
    };

    // Always target the *last* hex `/Contents` signature in the file — that is
    // the placeholder we just appended. Matching an earlier signature (often
    // with a short, already-filled `/ByteRange`) caused "placeholder too small".
    let (contents_lt, contents_gt, br_key) = find_last_signature_span(&buf)?;

    // `/ByteRange` covers everything except the `/Contents` string *including*
    // its `<` and `>` delimiters. Leaving the delimiters inside the signed
    // ranges is still self-consistent (the digest matches, so verifiers that
    // only re-hash report "valid"), but it breaks the universal convention
    // `start2 == len1 + len(contents) * 2 + 2`. Strict validators use that
    // identity to recognise the gap as the signature value; when it is off they
    // classify the coverage as non-standard and can no longer tell which
    // revision the signature covers. The practical symptom is that every
    // earlier signature of a multi-signed document is reported invalid.
    let gap_start = contents_lt; // the `<`
    let gap_end = contents_gt + 1; // one past the `>`
    let br = [
        0i64,
        gap_start as i64,
        gap_end as i64,
        (buf.len() - gap_end) as i64,
    ];
    patch_byte_range_at(&mut buf, br, br_key)?;

    // Digest over the two signed ranges (ByteRange is already final).
    let digest = sha256_ranges(&buf, gap_start, gap_end);

    let cms_der = cms::build_detached_pkcs7(cert_der, &digest, sign)?;

    embed_contents(&mut buf, contents_lt, contents_gt, &cms_der)?;
    Ok(buf)
}

/// Decide whether `pdf` already carries signatures, and refuse to sign when the
/// ones it carries have already been broken.
///
/// Detection parses the document rather than scanning the raw bytes: a signature
/// dictionary may sit inside a compressed object stream, where `/ByteRange`
/// never appears as plain text. Missing those meant falling back to the
/// full-rewrite path, which relocates every object and silently destroys the
/// existing signatures.
///
/// Two states are rejected outright, because signing on top of them can only
/// produce a document whose earlier signature is invalid:
///
/// * a signature dictionary that the raw scan cannot see - it was re-serialized
///   into an object stream by some other tool, so its byte offsets already moved;
/// * a `/ByteRange` whose declared gap no longer lines up with where its own
///   `/Contents` actually is - the file was rewritten after signing.
fn check_existing_signatures(pdf: &[u8]) -> CieResult<bool> {
    // Signature dictionaries as seen after parsing (object streams expanded).
    let parsed = match Document::load_mem(pdf) {
        Ok(doc) => doc
            .objects
            .values()
            .filter(|o| matches!(o, Object::Dictionary(d) if d.has(b"ByteRange")))
            .count(),
        // Unparseable here means the placeholder builders would fail anyway;
        // let them report the real error.
        Err(_) => return Ok(find(pdf, b"/ByteRange", 0).is_some()),
    };
    if parsed == 0 {
        return Ok(false);
    }

    let mut raw = 0usize;
    let mut pos = 0usize;
    while let Some(at) = find(pdf, b"/ByteRange", pos) {
        raw += 1;
        pos = at + 1;
    }
    if raw < parsed {
        return Err(CieError::Pdf(
            "this PDF's existing signature has been re-saved by another tool \
             (it is stored in a compressed object stream), so it is already \
             invalid. Sign the original signed file instead."
                .into(),
        ));
    }

    // Each existing signature's declared gap must still be exactly its own
    // `/Contents` string, delimiters included.
    let mut pos = 0usize;
    while let Some(br_key) = find(pdf, b"/ByteRange", pos) {
        pos = br_key + 1;
        let Some(br) = parse_byte_range(pdf, br_key) else {
            continue;
        };
        let Some(contents) = find(pdf, b"/Contents", br_key) else {
            continue;
        };
        let Some(lt) = find(pdf, b"<", contents) else {
            continue;
        };
        // A hex string, not a nested dictionary (`<<`) or an object reference.
        if pdf.get(lt + 1) == Some(&b'<') {
            continue;
        }
        let Some(gt) = find(pdf, b">", lt) else {
            continue;
        };
        if br[1] as usize != lt || br[2] as usize != gt + 1 {
            return Err(CieError::Pdf(
                "this PDF was modified after it was signed, so its existing \
                 signature is already invalid. Sign the unmodified signed file, \
                 and note that flattening stamps into a signed PDF breaks its \
                 signatures."
                    .into(),
            ));
        }
    }

    Ok(true)
}

/// Read the four `/ByteRange` integers that follow the key at `key`.
fn parse_byte_range(buf: &[u8], key: usize) -> Option<[i64; 4]> {
    let lb = find(buf, b"[", key)?;
    let rb = find(buf, b"]", lb)?;
    let text = std::str::from_utf8(&buf[lb + 1..rb]).ok()?;
    let nums: Vec<i64> = text.split_whitespace().filter_map(|t| t.parse().ok()).collect();
    if nums.len() != 4 {
        return None;
    }
    Some([nums[0], nums[1], nums[2], nums[3]])
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

    // --- Signature dictionary + invisible field / widget ---
    let sig_id = doc.add_object(build_sig_dict(reason, location));
    let field = build_widget_field(sig_id, page_id, rect, appearance_ref, "Signature1");
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

/// Build the (visible or invisible) signature via an **incremental update**: the
/// original bytes are preserved verbatim and only the new/changed objects (the
/// signature dict, its widget, the touched page and AcroForm) are appended. This
/// keeps every existing signature's `/ByteRange` valid.
///
/// Returns the serialized PDF plus the byte offset from which our *new*
/// signature dictionary can be located (i.e. the length of the preserved
/// original, so searches skip pre-existing signatures).
fn build_incremental_pdf(
    pdf: &[u8],
    reason: Option<&str>,
    location: Option<&str>,
    appearance: Option<&SignatureAppearance>,
) -> CieResult<(Vec<u8>, usize)> {
    let prev = Document::load_mem(pdf)?;

    // --- Extract everything we need from `prev` before it is moved. ---
    let root_id = prev
        .trailer
        .get(b"Root")
        .map_err(CieError::from)?
        .as_reference()
        .map_err(CieError::from)?;

    let page_ids: Vec<ObjectId> = prev.get_pages().into_values().collect();
    let target_page = appearance.map(|a| a.page_index).unwrap_or(0);
    let page_id = *page_ids
        .get(target_page)
        .or_else(|| page_ids.first())
        .ok_or_else(|| CieError::Pdf("PDF has no pages".into()))?;

    // How is /Annots stored on the target page? (inline array vs indirect array)
    let annots_ref: Option<ObjectId> = prev
        .get_object(page_id)
        .ok()
        .and_then(|o| o.as_dict().ok())
        .and_then(|d| d.get(b"Annots").ok())
        .and_then(|o| o.as_reference().ok());

    // How is the AcroForm stored on the catalog?
    let catalog = prev
        .get_object(root_id)
        .ok()
        .and_then(|o| o.as_dict().ok());
    let acroform_ref: Option<ObjectId> = catalog
        .and_then(|d| d.get(b"AcroForm").ok())
        .and_then(|o| o.as_reference().ok());
    let acroform_inline = catalog
        .and_then(|d| d.get(b"AcroForm").ok())
        .map(|o| o.as_dict().is_ok())
        .unwrap_or(false);
    // If the AcroForm's /Fields is an indirect array, we edit that object.
    let fields_ref: Option<ObjectId> = acroform_ref
        .and_then(|id| prev.get_object(id).ok())
        .and_then(|o| o.as_dict().ok())
        .and_then(|d| d.get(b"Fields").ok())
        .and_then(|o| o.as_reference().ok());

    let prev_len = pdf.len();
    let mut idoc = IncrementalDocument::create_from(pdf.to_vec(), prev);

    // Build the appearance form XObject (added to the *new* document).
    let appearance_ref = match appearance {
        Some(ap) => Some(build_appearance_xobject(&mut idoc.new_document, ap)?),
        None => None,
    };
    let rect = appearance.map(|a| a.rect).unwrap_or([0.0, 0.0, 0.0, 0.0]);

    // Signature dict + widget field (unique field name to avoid clashing with
    // the existing signature's /T).
    let sig_id = idoc.new_document.add_object(build_sig_dict(reason, location));
    let field_name = format!("Signature{}", prev_len); // just needs to be unique
    let field = build_widget_field(sig_id, page_id, rect, appearance_ref, &field_name);
    let field_id = idoc.new_document.add_object(field);

    // --- Append the widget to the page's /Annots (clone page into the update) ---
    idoc.opt_clone_object_to_new_document(page_id)?;
    if let Some(arr_id) = annots_ref {
        idoc.opt_clone_object_to_new_document(arr_id)?;
        let arr = idoc
            .new_document
            .get_object_mut(arr_id)
            .map_err(CieError::from)?
            .as_array_mut()
            .map_err(CieError::from)?;
        arr.push(Object::Reference(field_id));
    } else {
        let page = idoc
            .new_document
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
            _ => page.set("Annots", Object::Array(vec![Object::Reference(field_id)])),
        }
    }

    // --- Register the field in the AcroForm ---
    if let Some(acro_id) = acroform_ref {
        idoc.opt_clone_object_to_new_document(acro_id)?;
        if let Some(f_id) = fields_ref {
            // /Fields is an indirect array object.
            idoc.opt_clone_object_to_new_document(f_id)?;
            let arr = idoc
                .new_document
                .get_object_mut(f_id)
                .map_err(CieError::from)?
                .as_array_mut()
                .map_err(CieError::from)?;
            arr.push(Object::Reference(field_id));
            let acro = idoc
                .new_document
                .get_object_mut(acro_id)
                .map_err(CieError::from)?
                .as_dict_mut()
                .map_err(CieError::from)?;
            acro.set("SigFlags", Object::Integer(3));
        } else {
            let acro = idoc
                .new_document
                .get_object_mut(acro_id)
                .map_err(CieError::from)?
                .as_dict_mut()
                .map_err(CieError::from)?;
            append_field(acro, field_id);
            acro.set("SigFlags", Object::Integer(3));
        }
    } else if acroform_inline {
        idoc.opt_clone_object_to_new_document(root_id)?;
        let cat = idoc
            .new_document
            .get_object_mut(root_id)
            .map_err(CieError::from)?
            .as_dict_mut()
            .map_err(CieError::from)?;
        let acro = cat
            .get_mut(b"AcroForm")
            .map_err(CieError::from)?
            .as_dict_mut()
            .map_err(CieError::from)?;
        append_field(acro, field_id);
        acro.set("SigFlags", Object::Integer(3));
    } else {
        // No AcroForm at all: create one and point the (cloned) catalog at it.
        let mut acro = Dictionary::new();
        acro.set("Fields", Object::Array(vec![Object::Reference(field_id)]));
        acro.set("SigFlags", Object::Integer(3));
        let acro_id = idoc.new_document.add_object(acro);
        idoc.opt_clone_object_to_new_document(root_id)?;
        let cat = idoc
            .new_document
            .get_object_mut(root_id)
            .map_err(CieError::from)?
            .as_dict_mut()
            .map_err(CieError::from)?;
        cat.set("AcroForm", Object::Reference(acro_id));
    }

    // Defensive: the trailer is cloned from the previous xref stream dict; drop
    // keys that the writer regenerates or that would misdescribe the new stream.
    idoc.new_document.trailer.remove(b"DecodeParms");
    idoc.new_document.trailer.remove(b"Filter");

    let mut out = Vec::new();
    idoc.save_to(&mut out)
        .map_err(|e| CieError::Pdf(e.to_string()))?;
    neutralize_appended_header(&mut out, prev_len);
    Ok((out, prev_len))
}

/// Erase the `%PDF-x.y` header line that lopdf emits at the start of an appended
/// revision.
///
/// A second `%PDF-` marker right after `%%EOF` is not part of an incremental
/// update (PDF 32000-1 §7.5.6: original bytes, added objects, xref section,
/// trailer — no header). Validators that split a file into revisions read it as
/// two concatenated documents and can no longer attribute the earlier revision
/// to the earlier signature, so they report that first signature as invalid even
/// though its signed bytes are untouched.
///
/// The line is overwritten in place with a same-length comment: the appended
/// cross-reference section stores absolute offsets, so the byte count must not
/// change. Called before the `/ByteRange` and digest are computed, so the new
/// signature covers the rewritten bytes.
fn neutralize_appended_header(buf: &mut [u8], appended_from: usize) {
    let Some(start) = find(buf, b"%PDF-", appended_from) else {
        return;
    };
    // Only the writer's own header, which sits at the very start of the appended
    // region (optionally after a separating newline) - never a later match.
    if start > appended_from + 1 {
        return;
    }
    let end = find(buf, b"\n", start).unwrap_or(buf.len());
    buf[start] = b'%';
    for b in &mut buf[start + 1..end] {
        *b = b' ';
    }
}

/// Append a field reference to an AcroForm's `/Fields` (inline array form).
fn append_field(acro: &mut Dictionary, field_id: ObjectId) {
    match acro.get(b"Fields") {
        Ok(Object::Array(existing)) => {
            let mut arr = existing.clone();
            arr.push(Object::Reference(field_id));
            acro.set("Fields", Object::Array(arr));
        }
        _ => acro.set("Fields", Object::Array(vec![Object::Reference(field_id)])),
    }
}

/// The PAdES signature dictionary with fixed-size `/ByteRange` and `/Contents`
/// placeholders that are patched after serialization.
fn build_sig_dict(reason: Option<&str>, location: Option<&str>) -> Dictionary {
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
    sig
}

/// The signature widget annotation (visible if `appearance_ref` is set).
fn build_widget_field(
    sig_id: ObjectId,
    page_id: ObjectId,
    rect: [f32; 4],
    appearance_ref: Option<ObjectId>,
    field_name: &str,
) -> Dictionary {
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
        Object::String(field_name.as_bytes().to_vec(), StringFormat::Literal),
    );
    field.set("V", Object::Reference(sig_id));
    field.set("P", Object::Reference(page_id));
    field.set("F", Object::Integer(132)); // Print (4) + Locked (128)
    if let Some(xobj_id) = appearance_ref {
        let mut ap = Dictionary::new();
        ap.set("N", Object::Reference(xobj_id));
        field.set("AP", Object::Dictionary(ap));
    }
    field
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

/// Locate the last PAdES signature placeholder in `buf`.
///
/// Returns `(contents_lt, contents_gt, byterange_key_offset)` for the last
/// `/ByteRange` that is followed by a hex `/Contents <...>`. Page content
/// streams (`/Contents 5 0 R`) are skipped because they have no `<` hex string
/// immediately after `/Contents`.
fn find_last_signature_span(buf: &[u8]) -> CieResult<(usize, usize, usize)> {
    let mut pos = 0;
    let mut last: Option<(usize, usize, usize)> = None;
    while let Some(byte_range) = find(buf, b"/ByteRange", pos) {
        pos = byte_range + 1;
        let Some(contents) = find(buf, b"/Contents", byte_range) else {
            continue;
        };
        // Only accept hex string Contents (signature blob), not object refs.
        let Some(lt) = find(buf, b"<", contents) else {
            continue;
        };
        // Reject `/Contents 5 0 R` cases where `<` belongs to a later object:
        // the hex open must sit before any other PDF token that starts a value.
        // A real sig dict has `/Contents<` or `/Contents <` with only whitespace.
        let between = &buf[contents + b"/Contents".len()..lt];
        if !between.iter().all(|b| b.is_ascii_whitespace()) {
            continue;
        }
        let Some(gt) = find(buf, b">", lt) else {
            continue;
        };
        last = Some((lt, gt, byte_range));
    }
    last.ok_or_else(|| CieError::Pdf("no signature /ByteRange+/Contents placeholder found".into()))
}

/// Overwrite the `/ByteRange` array at `key` in place, preserving its byte length.
fn patch_byte_range_at(buf: &mut [u8], br: [i64; 4], key: usize) -> CieResult<()> {
    let lb = find(buf, b"[", key).ok_or_else(|| CieError::Pdf("ByteRange '[' not found".into()))?;
    let rb = find(buf, b"]", lb).ok_or_else(|| CieError::Pdf("ByteRange ']' not found".into()))?;

    let interior_len = rb - (lb + 1);
    let mut replacement = format!("{} {} {} {}", br[0], br[1], br[2], br[3]).into_bytes();
    if replacement.len() > interior_len {
        return Err(CieError::Pdf(format!(
            "ByteRange placeholder too small (need {} bytes, have {} for values {:?})",
            replacement.len(),
            interior_len,
            br
        )));
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A minimal, valid single-page PDF.
    fn minimal_pdf() -> Vec<u8> {
        let mut doc = Document::with_version("1.5");
        let pages_id = doc.new_object_id();
        let mut page = Dictionary::new();
        page.set("Type", Object::Name(b"Page".to_vec()));
        page.set("Parent", Object::Reference(pages_id));
        page.set(
            "MediaBox",
            Object::Array(vec![
                0.into(),
                0.into(),
                612.into(),
                792.into(),
            ]),
        );
        let page_id = doc.add_object(page);

        let mut pages = Dictionary::new();
        pages.set("Type", Object::Name(b"Pages".to_vec()));
        pages.set("Kids", Object::Array(vec![Object::Reference(page_id)]));
        pages.set("Count", Object::Integer(1));
        doc.objects.insert(pages_id, Object::Dictionary(pages));

        let mut catalog = Dictionary::new();
        catalog.set("Type", Object::Name(b"Catalog".to_vec()));
        catalog.set("Pages", Object::Reference(pages_id));
        let catalog_id = doc.add_object(catalog);
        doc.trailer.set("Root", Object::Reference(catalog_id));

        let mut out = Vec::new();
        doc.save_to(&mut out).unwrap();
        out
    }

    /// Count how many signature dictionaries a PDF carries.
    fn count_byteranges(buf: &[u8]) -> usize {
        let mut n = 0;
        let mut pos = 0;
        while let Some(i) = find(buf, b"/ByteRange", pos) {
            n += 1;
            pos = i + 1;
        }
        n
    }

    #[test]
    fn incremental_update_preserves_first_signature_bytes() {
        // Simulate a first signature by building a placeholder PDF (it contains
        // a `/ByteRange`, so `sign_pdf` would treat it as already-signed).
        let base = minimal_pdf();
        let first = build_placeholder_pdf(&base, Some("first"), None, None).unwrap();
        assert_eq!(count_byteranges(&first), 1);

        // Now add a second signature via the incremental path.
        let (out, prev_len) = build_incremental_pdf(&first, Some("second"), None, None).unwrap();

        // The original bytes must be preserved verbatim (byte-for-byte prefix),
        // otherwise the first signature's ByteRange would break.
        assert_eq!(prev_len, first.len());
        assert_eq!(&out[..first.len()], &first[..]);

        // The result must now carry two signatures and still parse.
        assert_eq!(count_byteranges(&out), 2);
        let reparsed = Document::load_mem(&out).expect("incremental PDF must parse");
        // AcroForm must list two fields.
        let root = reparsed
            .trailer
            .get(b"Root")
            .unwrap()
            .as_reference()
            .unwrap();
        let acro_ref = reparsed
            .get_object(root)
            .unwrap()
            .as_dict()
            .unwrap()
            .get(b"AcroForm")
            .unwrap()
            .as_reference()
            .unwrap();
        let fields = reparsed
            .get_object(acro_ref)
            .unwrap()
            .as_dict()
            .unwrap()
            .get(b"Fields")
            .unwrap()
            .as_array()
            .unwrap();
        assert_eq!(fields.len(), 2, "both signature fields must be registered");

        // The new signature's placeholder must be locatable and patchable —
        // always the *last* hex Contents span.
        let (lt, gt, br_key) = find_last_signature_span(&out).unwrap();
        assert!(lt >= prev_len && gt > lt);
        assert!(br_key >= prev_len);
        let mut patched = out.clone();
        let br = [0i64, (lt + 1) as i64, gt as i64, (patched.len() - gt) as i64];
        patch_byte_range_at(&mut patched, br, br_key).expect("second ByteRange must fit");
    }

    /// Generate a self-signed RSA key + cert with openssl, returning
    /// (key_pem_path, cert_der).
    fn make_test_signer(tag: &str) -> Option<(String, Vec<u8>)> {
        use std::process::Command;
        let dir = std::env::temp_dir().join(format!("timbrapp-pades-{tag}"));
        std::fs::create_dir_all(&dir).ok()?;
        let key = dir.join("key.pem");
        let crt = dir.join("cert.der");
        let ok = Command::new("openssl")
            .args([
                "req",
                "-x509",
                "-newkey",
                "rsa:2048",
                "-keyout",
                key.to_str()?,
                "-out",
                crt.to_str()?,
                "-outform",
                "DER",
                "-days",
                "3650",
                "-nodes",
                "-subj",
                &format!("/C=IT/CN=Test Signer {tag}"),
            ])
            .output()
            .ok()?;
        if !ok.status.success() {
            return None;
        }
        Some((key.to_str()?.to_string(), std::fs::read(&crt).ok()?))
    }

    /// PKCS#1 v1.5 signature over SHA-256 of `tbs` — same shape the card produces.
    fn openssl_sign(key_pem: &str, tbs: &[u8]) -> CieResult<Vec<u8>> {
        use std::process::Command;
        let dir = std::path::Path::new(key_pem).parent().unwrap();
        let tbs_path = dir.join("tbs.bin");
        let sig_path = dir.join("sig.bin");
        std::fs::write(&tbs_path, tbs).map_err(|e| CieError::Other(e.to_string()))?;
        let out = Command::new("openssl")
            .args([
                "dgst",
                "-sha256",
                "-sign",
                key_pem,
                "-out",
                sig_path.to_str().unwrap(),
                tbs_path.to_str().unwrap(),
            ])
            .output()
            .map_err(|e| CieError::Other(e.to_string()))?;
        if !out.status.success() {
            return Err(CieError::Other(String::from_utf8_lossy(&out.stderr).into()));
        }
        std::fs::read(&sig_path).map_err(|e| CieError::Other(e.to_string()))
    }

    /// End-to-end: produce a genuinely signed 1-sig and 2-sig PDF with software
    /// keys and write them to /tmp so `pdfsig` can validate them.
    #[test]
    fn produce_real_single_and_double_signed_pdfs() {
        let Some((key_a, cert_a)) = make_test_signer("a") else {
            eprintln!("openssl unavailable — skipping");
            return;
        };
        let Some((key_b, cert_b)) = make_test_signer("b") else {
            return;
        };

        // A real-world PDF (real xref tables, object streams, trailer /ID)
        // exercises far more of the incremental path than the synthetic one.
        let base = match std::env::var("TIMBRAPP_TEST_PDF") {
            Ok(p) => std::fs::read(p).expect("TIMBRAPP_TEST_PDF unreadable"),
            Err(_) => minimal_pdf(),
        };
        let one = sign_pdf_with(
            &base,
            &cert_a,
            |tbs| openssl_sign(&key_a, tbs),
            Some("first signer"),
            None,
            None,
        )
        .expect("first signature");
        std::fs::write("/tmp/timbrapp-sig1.pdf", &one).unwrap();

        let two = sign_pdf_with(
            &one,
            &cert_b,
            |tbs| openssl_sign(&key_b, tbs),
            Some("second signer"),
            None,
            None,
        )
        .expect("second signature");
        std::fs::write("/tmp/timbrapp-sig2.pdf", &two).unwrap();

        // Original revision must be preserved byte-for-byte.
        assert_eq!(&two[..one.len()], &one[..]);
        assert_eq!(count_byteranges(&two), 2);

        // The appended revision must not contain a second `%PDF-` header, which
        // would make validators read the file as two concatenated documents.
        assert!(
            find(&two, b"%PDF-", one.len()).is_none(),
            "incremental update must not restate a PDF header"
        );

        // Re-verify the FIRST signature's digest over its own byte ranges in the
        // *double-signed* file: this is exactly what a validator recomputes.
        let first_br = find(&two, b"/ByteRange", 0).unwrap();
        let lb = find(&two, b"[", first_br).unwrap();
        let rb = find(&two, b"]", lb).unwrap();
        let text = String::from_utf8_lossy(&two[lb + 1..rb]);
        let nums: Vec<usize> = text
            .split_whitespace()
            .map(|t| t.parse().unwrap())
            .collect();
        assert_eq!(nums.len(), 4, "ByteRange must have 4 entries: {text}");
        let (o1, l1, o2, l2) = (nums[0], nums[1], nums[2], nums[3]);
        assert_eq!(o1, 0);

        // The gap between the signed ranges must be exactly the `/Contents`
        // string *with* its `<` and `>`. Validators rely on this identity to
        // recognise the excluded bytes as the signature value.
        let lt = find(&two, b"<", first_br).unwrap();
        let gt = find(&two, b">", lt).unwrap();
        let contents_hex = gt - (lt + 1);
        assert_eq!(
            o2,
            l1 + contents_hex + 2,
            "signed ranges must exclude /Contents including its delimiters"
        );

        assert_eq!(
            o2 + l2,
            one.len(),
            "first signature's signed range must end at the original EOF, \
             so appending a second signature cannot disturb it"
        );
        // The bytes it covers must be identical in both files.
        assert_eq!(&two[o1..o1 + l1], &one[o1..o1 + l1]);
        assert_eq!(&two[o2..o2 + l2], &one[o2..o2 + l2]);

        // A third signature stacks an increment on top of an increment.
        let Some((key_c, cert_c)) = make_test_signer("c") else {
            return;
        };
        let three = sign_pdf_with(
            &two,
            &cert_c,
            |tbs| openssl_sign(&key_c, tbs),
            Some("third signer"),
            None,
            None,
        )
        .expect("third signature");
        std::fs::write("/tmp/timbrapp-sig3.pdf", &three).unwrap();
        assert_eq!(&three[..two.len()], &two[..]);
        assert_eq!(count_byteranges(&three), 3);
    }

    /// Signing a document that was re-saved after signing (what flattening
    /// stamps used to do) must fail loudly instead of producing a file whose
    /// earlier signature is silently broken.
    #[test]
    fn refuses_to_sign_a_document_altered_after_signing() {
        let Some((key, cert)) = make_test_signer("d") else {
            eprintln!("openssl unavailable — skipping");
            return;
        };
        let one = sign_pdf_with(
            &minimal_pdf(),
            &cert,
            |tbs| openssl_sign(&key, tbs),
            None,
            None,
            None,
        )
        .expect("first signature");
        assert!(
            check_existing_signatures(&one).expect("intact signed pdf is signable"),
            "a signed PDF must be recognised as already signed"
        );

        // Re-serialize the way a PDF library does when flattening stamps: every
        // object is rewritten, so the signature's byte offsets move.
        let mut doc = Document::load_mem(&one).unwrap();
        let mut resaved = Vec::new();
        doc.save_to(&mut resaved).unwrap();

        let err = check_existing_signatures(&resaved)
            .expect_err("a re-saved signed PDF must be rejected");
        let msg = err.to_string();
        assert!(
            msg.contains("modified after it was signed") || msg.contains("object stream"),
            "unexpected error: {msg}"
        );
    }

    /// Regression: a short first `/ByteRange` (as many third-party signers
    /// write) must not be selected when patching our newly added placeholder.
    #[test]
    fn last_signature_span_skips_short_earlier_byterange() {
        let base = minimal_pdf();
        let first = build_placeholder_pdf(&base, Some("first"), None, None).unwrap();
        // Shrink the first signature's ByteRange interior to something that
        // cannot hold a large second-signature offset list.
        let key = find(&first, b"/ByteRange", 0).unwrap();
        let lb = find(&first, b"[", key).unwrap();
        let rb = find(&first, b"]", lb).unwrap();
        let mut short = first.clone();
        let tiny = b"0 1 2 3";
        assert!(tiny.len() < rb - (lb + 1));
        short[lb + 1..lb + 1 + tiny.len()].copy_from_slice(tiny);
        for b in short[lb + 1 + tiny.len()..rb].iter_mut() {
            *b = b' ';
        }

        let (out, prev_len) = build_incremental_pdf(&short, Some("second"), None, None).unwrap();
        assert_eq!(&out[..prev_len], &short[..]);

        let (lt, gt, br_key) = find_last_signature_span(&out).unwrap();
        assert!(br_key >= prev_len, "must select the new signature, not the short first one");
        let mut patched = out.clone();
        let br = [0i64, (lt + 1) as i64, gt as i64, (patched.len() - gt) as i64];
        patch_byte_range_at(&mut patched, br, br_key)
            .expect("patching the last (wide) placeholder must succeed");
    }
}
