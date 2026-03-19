// Copyright 2026 Google LLC
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use super::*;

/// Handle the `+draft-get` subcommand: fetch a draft and extract its metadata and body.
pub(super) async fn handle_draft_get(matches: &ArgMatches) -> Result<(), GwsError> {
    let draft_id = matches.get_one::<String>("draft").unwrap();

    let token = auth::get_token(&[GMAIL_READONLY_SCOPE])
        .await
        .map_err(|e| GwsError::Auth(format!("Gmail auth failed: {e}")))?;

    let client = crate::client::build_client()?;

    let url = format!(
        "https://gmail.googleapis.com/gmail/v1/users/me/drafts/{}?format=full",
        crate::validate::encode_path_segment(draft_id)
    );

    let resp = crate::client::send_with_retry(|| client.get(&url).bearer_auth(&token))
        .await
        .map_err(|e| GwsError::Other(anyhow::anyhow!("Failed to fetch draft: {e}")))?;

    if !resp.status().is_success() {
        let status = resp.status().as_u16();
        let body = resp
            .text()
            .await
            .unwrap_or_else(|_| "(error body unreadable)".to_string());
        return Err(build_api_error(
            status,
            &body,
            &format!("Failed to fetch draft {draft_id}"),
        ));
    }

    let draft: Value = resp
        .json()
        .await
        .map_err(|e| GwsError::Other(anyhow::anyhow!("Failed to parse draft response: {e}")))?;

    let message = draft.get("message").unwrap_or(&Value::Null);
    let payload = message.get("payload").unwrap_or(&Value::Null);

    // Extract headers
    let headers = payload
        .get("headers")
        .and_then(|h| h.as_array())
        .cloned()
        .unwrap_or_default();

    let mut subject = String::new();
    let mut to = String::new();
    for header in &headers {
        let name = header.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let value = header.get("value").and_then(|v| v.as_str()).unwrap_or("");
        match name {
            "Subject" => subject = value.to_string(),
            "To" => to = value.to_string(),
            _ => {}
        }
    }

    // Extract body: try payload.body.data first, then first text/plain part
    let body_text = extract_draft_body(payload);

    let output = json!({
        "id": draft_id,
        "to": to,
        "subject": subject,
        "body": body_text,
    });

    println!(
        "{}",
        serde_json::to_string_pretty(&output).context("Failed to serialize draft output")?
    );

    Ok(())
}

/// Extract the body text from a draft payload.
///
/// Tries `payload.body.data` first (single-part messages), then walks
/// `payload.parts` looking for the first `text/plain` part.
fn extract_draft_body(payload: &Value) -> String {
    // Try direct body.data on the payload
    if let Some(data) = payload
        .get("body")
        .and_then(|b| b.get("data"))
        .and_then(|d| d.as_str())
    {
        if let Some(decoded) = decode_base64url(data) {
            return decoded;
        }
    }

    // Walk parts for text/plain
    if let Some(parts) = payload.get("parts").and_then(|p| p.as_array()) {
        for part in parts {
            let mime = part.get("mimeType").and_then(|v| v.as_str()).unwrap_or("");
            if mime == "text/plain" {
                if let Some(data) = part
                    .get("body")
                    .and_then(|b| b.get("data"))
                    .and_then(|d| d.as_str())
                {
                    if let Some(decoded) = decode_base64url(data) {
                        return decoded;
                    }
                }
            }
        }
    }

    String::new()
}

/// Decode a base64url-encoded string, returning `None` on failure.
fn decode_base64url(data: &str) -> Option<String> {
    URL_SAFE
        .decode(data)
        .ok()
        .map(|bytes| String::from_utf8_lossy(&bytes).to_string())
}

/// Handle the `+draft-update` subcommand: update an existing draft with new content.
pub(super) async fn handle_draft_update(matches: &ArgMatches) -> Result<(), GwsError> {
    let draft_id = matches.get_one::<String>("draft").unwrap();
    let to = matches.get_one::<String>("to").unwrap();
    let subject = matches.get_one::<String>("subject").unwrap();
    let body = matches.get_one::<String>("body").unwrap();

    let token = auth::get_token(&[GMAIL_SCOPE])
        .await
        .map_err(|e| GwsError::Auth(format!("Gmail auth failed: {e}")))?;

    let client = crate::client::build_client()?;

    // Build RFC 5322 message
    let raw_message = format!("From: me\r\nTo: {to}\r\nSubject: {subject}\r\n\r\n{body}");
    let encoded = URL_SAFE.encode(raw_message.as_bytes());

    let url = format!(
        "https://gmail.googleapis.com/gmail/v1/users/me/drafts/{}",
        crate::validate::encode_path_segment(draft_id)
    );

    let request_body = json!({
        "message": {
            "raw": encoded
        }
    });

    let resp =
        crate::client::send_with_retry(|| client.put(&url).bearer_auth(&token).json(&request_body))
            .await
            .map_err(|e| GwsError::Other(anyhow::anyhow!("Failed to update draft: {e}")))?;

    if !resp.status().is_success() {
        let status = resp.status().as_u16();
        let resp_body = resp
            .text()
            .await
            .unwrap_or_else(|_| "(error body unreadable)".to_string());
        return Err(build_api_error(
            status,
            &resp_body,
            &format!("Failed to update draft {draft_id}"),
        ));
    }

    let output = json!({
        "id": draft_id,
        "to": to,
        "subject": subject,
    });

    println!(
        "{}",
        serde_json::to_string_pretty(&output).context("Failed to serialize draft-update output")?
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_base64url_valid() {
        let encoded = URL_SAFE.encode(b"Hello, world!");
        let decoded = decode_base64url(&encoded);
        assert_eq!(decoded, Some("Hello, world!".to_string()));
    }

    #[test]
    fn test_decode_base64url_invalid() {
        let decoded = decode_base64url("not-valid-base64@@@");
        assert!(decoded.is_none());
    }

    #[test]
    fn test_extract_draft_body_direct() {
        let encoded = URL_SAFE.encode(b"Draft body text");
        let payload = json!({
            "body": { "data": encoded },
            "mimeType": "text/plain"
        });
        assert_eq!(extract_draft_body(&payload), "Draft body text");
    }

    #[test]
    fn test_extract_draft_body_from_parts() {
        let encoded = URL_SAFE.encode(b"Part body text");
        let payload = json!({
            "body": { "size": 0 },
            "mimeType": "multipart/alternative",
            "parts": [
                {
                    "mimeType": "text/plain",
                    "body": { "data": encoded }
                },
                {
                    "mimeType": "text/html",
                    "body": { "data": "ignored" }
                }
            ]
        });
        assert_eq!(extract_draft_body(&payload), "Part body text");
    }

    #[test]
    fn test_extract_draft_body_empty_payload() {
        let payload = json!({});
        assert_eq!(extract_draft_body(&payload), "");
    }

    #[test]
    fn test_draft_update_rfc5322_format() {
        let to = "alice@example.com";
        let subject = "Test Subject";
        let body = "Hello Alice";
        let raw = format!("From: me\r\nTo: {to}\r\nSubject: {subject}\r\n\r\n{body}");

        assert!(raw.starts_with("From: me\r\n"));
        assert!(raw.contains("To: alice@example.com\r\n"));
        assert!(raw.contains("Subject: Test Subject\r\n"));
        assert!(raw.ends_with("\r\n\r\nHello Alice"));

        // Verify base64url encoding roundtrip
        let encoded = URL_SAFE.encode(raw.as_bytes());
        let decoded_bytes = URL_SAFE.decode(&encoded).unwrap();
        let decoded = String::from_utf8(decoded_bytes).unwrap();
        assert_eq!(decoded, raw);
    }
}
