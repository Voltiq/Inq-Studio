//! Password protection: what a file is written with on save.
//!
//! A protected file that is opened and saved again keeps its protection
//! exactly: the same file key and the same password entries are written
//! back, so both the open password and the permissions password stay valid
//! even though only one of them was typed. Setting new protection derives a
//! fresh key (AES-256, or AES-128 for older readers); removing it writes the
//! file in the clear.

use lopdf::encryption::crypt_filters::{Aes128CryptFilter, Aes256CryptFilter, CryptFilter};
use lopdf::{Document, EncryptionState, EncryptionVersion, Object, Permissions, StringFormat};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::Arc;

/// What someone who opens the file without the permissions password may do.
#[derive(Deserialize, Serialize, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Allowed {
    pub print: bool,
    pub print_high: bool,
    pub modify: bool,
    pub copy: bool,
    pub annotate: bool,
    pub fill: bool,
    pub assemble: bool,
    pub accessibility: bool,
}

impl Default for Allowed {
    fn default() -> Self {
        Allowed { print: true, print_high: true, modify: true, copy: true, annotate: true, fill: true, assemble: true, accessibility: true }
    }
}

impl Allowed {
    fn from_bits(p: Permissions) -> Allowed {
        Allowed {
            print: p.contains(Permissions::PRINTABLE),
            print_high: p.contains(Permissions::PRINTABLE_IN_HIGH_QUALITY),
            modify: p.contains(Permissions::MODIFIABLE),
            copy: p.contains(Permissions::COPYABLE),
            annotate: p.contains(Permissions::ANNOTABLE),
            fill: p.contains(Permissions::FILLABLE),
            assemble: p.contains(Permissions::ASSEMBLABLE),
            accessibility: p.contains(Permissions::COPYABLE_FOR_ACCESSIBILITY),
        }
    }

    fn bits(&self) -> Permissions {
        let mut p = Permissions::empty();
        let mut set = |on: bool, flag: Permissions| {
            if on {
                p |= flag;
            }
        };
        set(self.print, Permissions::PRINTABLE);
        set(self.print && self.print_high, Permissions::PRINTABLE_IN_HIGH_QUALITY);
        set(self.modify, Permissions::MODIFIABLE);
        set(self.copy, Permissions::COPYABLE);
        set(self.annotate, Permissions::ANNOTABLE);
        set(self.fill || self.annotate, Permissions::FILLABLE);
        set(self.assemble || self.modify, Permissions::ASSEMBLABLE);
        // Screen readers are always let in, as PDF 2.0 asks.
        set(true, Permissions::COPYABLE_FOR_ACCESSIBILITY);
        p
    }
}

#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ProtectRequest {
    /// Needed to open the file; empty for a file anyone can open.
    #[serde(default)]
    pub open_password: String,
    /// Needed to change the permissions; falls back to the open password.
    #[serde(default)]
    pub owner_password: String,
    #[serde(default)]
    pub allowed: Allowed,
    /// AES-256 (PDF 1.7 extension 3 / PDF 2.0) rather than AES-128.
    #[serde(default = "yes")]
    pub aes256: bool,
}

fn yes() -> bool {
    true
}

impl ProtectRequest {
    pub fn owner(&self) -> &str {
        if self.owner_password.is_empty() {
            &self.open_password
        } else {
            &self.owner_password
        }
    }

    /// A password that opens the written file with full rights.
    pub fn reopen_password(&self) -> &str {
        self.owner()
    }

    pub fn check(&self) -> Result<(), String> {
        if self.open_password.is_empty() && self.owner_password.is_empty() {
            return Err("Enter a password to open the document, a permissions password, or both.".into());
        }
        if self.open_password.is_empty() && self.allowed == Allowed::default() {
            return Err("Without an open password, restrict at least one permission; otherwise the protection does nothing.".into());
        }
        Ok(())
    }
}

#[derive(Clone)]
pub enum Protection {
    None,
    /// Written back exactly as the file had it.
    Keep(EncryptionState),
    /// Applied on the next save.
    New(ProtectRequest),
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SecurityInfo {
    pub protected: bool,
    /// New protection that the next save will write.
    pub pending: bool,
    /// "AES-256", "AES-128", "RC4 128-bit" …
    pub method: String,
    pub allowed: Allowed,
    /// Opening needs a password (only known for protection set in this session).
    pub open_password: Option<bool>,
}

pub fn info(p: &Protection) -> SecurityInfo {
    match p {
        Protection::None => SecurityInfo { protected: false, pending: false, method: String::new(), allowed: Allowed::default(), open_password: None },
        Protection::Keep(state) => {
            let method = match state.get_stream_filter().method() {
                b"AESV3" => "AES-256".to_string(),
                b"AESV2" => "AES-128".to_string(),
                b"Identity" | b"None" => "none".to_string(),
                _ => {
                    if state.revision() <= 2 {
                        "RC4 40-bit".to_string()
                    } else {
                        "RC4 128-bit".to_string()
                    }
                }
            };
            SecurityInfo { protected: true, pending: false, method, allowed: Allowed::from_bits(state.permissions()), open_password: None }
        }
        Protection::New(req) => SecurityInfo {
            protected: true,
            pending: true,
            method: if req.aes256 { "AES-256".into() } else { "AES-128".into() },
            allowed: req.allowed,
            open_password: Some(!req.open_password.is_empty()),
        },
    }
}

fn random(buf: &mut [u8]) -> Result<(), String> {
    getrandom::fill(buf).map_err(|e| format!("The system gave no random numbers: {e}"))
}

/// Gives the document the file identifier and version the protection needs.
pub fn prepare(doc: &mut Document, req: &ProtectRequest) {
    let has_id = matches!(doc.trailer.get(b"ID"), Ok(Object::Array(a)) if a.len() == 2);
    if !has_id {
        let mut id = [0u8; 16];
        if random(&mut id).is_ok() {
            let s = Object::String(id.to_vec(), StringFormat::Hexadecimal);
            doc.trailer.set("ID", Object::Array(vec![s.clone(), s]));
        }
    }
    let need = if req.aes256 { "1.7" } else { "1.6" };
    if doc.version.as_str() < need {
        doc.version = need.to_string();
    }
}

/// The encryption a request asks for, for `doc` (whose identifier keys AES-128).
pub fn build(req: &ProtectRequest, doc: &Document) -> Result<EncryptionState, String> {
    req.check()?;
    let permissions = req.allowed.bits();
    let failed = |e: lopdf::Error| format!("The protection could not be set up: {e}");
    if req.aes256 {
        let mut key = [0u8; 32];
        random(&mut key)?;
        let filter: Arc<dyn CryptFilter> = Arc::new(Aes256CryptFilter);
        EncryptionState::try_from(EncryptionVersion::V5 {
            encrypt_metadata: true,
            crypt_filters: BTreeMap::from([(b"StdCF".to_vec(), filter)]),
            file_encryption_key: &key,
            stream_filter: b"StdCF".to_vec(),
            string_filter: b"StdCF".to_vec(),
            owner_password: req.owner(),
            user_password: &req.open_password,
            permissions,
        })
        .map_err(failed)
    } else {
        let filter: Arc<dyn CryptFilter> = Arc::new(Aes128CryptFilter);
        EncryptionState::try_from(EncryptionVersion::V4 {
            document: doc,
            encrypt_metadata: true,
            crypt_filters: BTreeMap::from([(b"StdCF".to_vec(), filter)]),
            stream_filter: b"StdCF".to_vec(),
            string_filter: b"StdCF".to_vec(),
            owner_password: req.owner(),
            user_password: &req.open_password,
            permissions,
        })
        .map_err(failed)
    }
}
