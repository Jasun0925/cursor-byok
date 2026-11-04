//! Reassemble bounded Exec responses without trusting partial or changed pages.

use base64::{engine::general_purpose::STANDARD, Engine};
use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

const PAGE_BYTES: usize = 256 * 1024;
const MAX_RESPONSE_BYTES: usize = 5 * 1024 * 1024;

#[derive(Default)]
pub(super) struct Transfer {
    bytes: Vec<u8>,
    identity: Option<(String, usize)>,
}

#[derive(Deserialize)]
struct Page {
    offset: usize,
    total: usize,
    sha256: String,
    data: String,
}

impl Transfer {
    pub(super) fn request(&self, request: &mut Value) {
        request["offset"] = self.bytes.len().into();
        request["response_sha256"] = self
            .identity
            .as_ref()
            .map(|(digest, _)| Value::String(digest.clone()))
            .unwrap_or(Value::Null);
    }

    pub(super) fn accept(&mut self, output: &str) -> Result<Option<Vec<u8>>, String> {
        let value: Value = serde_json::from_str(output)
            .map_err(|e| format!("Invalid workspace response page: {e}"))?;
        if let Some(error) = value.get("error").and_then(Value::as_str) {
            return Err(error.into());
        }
        let page: Page = serde_json::from_value(value).map_err(|e| e.to_string())?;
        if page.offset != self.bytes.len()
            || page.total == 0
            || page.total > MAX_RESPONSE_BYTES
            || page.sha256.len() != 64
            || !page.sha256.bytes().all(|b| b.is_ascii_hexdigit())
            || self
                .identity
                .as_ref()
                .is_some_and(|(digest, total)| *digest != page.sha256 || *total != page.total)
        {
            return Err("Workspace response page is out of order, changed or oversized".into());
        }
        let bytes = STANDARD.decode(page.data).map_err(|e| e.to_string())?;
        if bytes.is_empty()
            || bytes.len() > PAGE_BYTES
            || self.bytes.len() + bytes.len() > page.total
            || (self.bytes.len() + bytes.len() < page.total && bytes.len() != PAGE_BYTES)
        {
            return Err("Invalid workspace response page length".into());
        }
        self.bytes.extend(bytes);
        if self.bytes.len() < page.total {
            self.identity = Some((page.sha256, page.total));
            return Ok(None);
        }
        if hex::encode(Sha256::digest(&self.bytes)) != page.sha256 {
            return Err("Workspace response checksum mismatch".into());
        }
        self.identity = None;
        Ok(Some(std::mem::take(&mut self.bytes)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn page(payload: &[u8], offset: usize) -> Value {
        json!({"offset":offset,"total":payload.len(),
            "sha256":hex::encode(Sha256::digest(payload)),
            "data":STANDARD.encode(&payload[offset..payload.len().min(offset+PAGE_BYTES)])})
    }

    #[test]
    fn pages_must_be_complete_ordered_and_bound_to_one_payload() {
        let payload = vec![b'x'; PAGE_BYTES + 13];
        let first = page(&payload, 0);
        let last = page(&payload, PAGE_BYTES);
        for invalid in [
            first.clone(),
            {
                let mut v = last.clone();
                v["total"] = json!(payload.len() + 1);
                v
            },
            {
                let mut v = last.clone();
                v["sha256"] = json!("a".repeat(64));
                v
            },
            {
                let mut v = last.clone();
                v["data"] = json!(STANDARD.encode(b"wrong bytes!!"));
                v
            },
        ] {
            let mut transfer = Transfer::default();
            assert!(transfer.accept(&first.to_string()).unwrap().is_none());
            assert!(transfer.accept(&invalid.to_string()).is_err());
        }
        let mut transfer = Transfer::default();
        assert!(transfer.accept(&first.to_string()).unwrap().is_none());
        let mut request = json!({});
        transfer.request(&mut request);
        assert_eq!(request["offset"], PAGE_BYTES);
        assert_eq!(request["response_sha256"], first["sha256"]);
        assert_eq!(
            transfer.accept(&last.to_string()).unwrap().unwrap(),
            payload
        );
        transfer.request(&mut request);
        assert_eq!(request["offset"], 0);
        assert!(request["response_sha256"].is_null());
    }

    #[test]
    fn invalid_lengths_and_corrupt_single_pages_are_rejected() {
        let valid = page(b"small", 0);
        for invalid in [
            {
                let mut v = valid.clone();
                v["total"] = json!(MAX_RESPONSE_BYTES + 1);
                v
            },
            {
                let mut v = valid.clone();
                v["data"] = json!("");
                v
            },
            {
                let mut v = valid.clone();
                v["total"] = json!(6);
                v
            },
            {
                let mut v = valid.clone();
                v["data"] = json!(STANDARD.encode(b"other"));
                v
            },
        ] {
            assert!(Transfer::default().accept(&invalid.to_string()).is_err());
        }
    }
}
