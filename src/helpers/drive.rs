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

use super::Helper;
use crate::auth;
use crate::error::GwsError;
use crate::executor;
use clap::{Arg, ArgMatches, Command};
use serde_json::{json, Value};
use std::future::Future;
use std::path::Path;
use std::pin::Pin;

pub struct DriveHelper;

impl Helper for DriveHelper {
    fn inject_commands(
        &self,
        mut cmd: Command,
        _doc: &crate::discovery::RestDescription,
    ) -> Command {
        cmd = cmd.subcommand(
            Command::new("+upload")
                .about("[Helper] Upload a file with automatic metadata")
                .arg(
                    Arg::new("file")
                        .help("Path to file to upload")
                        .required(true)
                        .index(1),
                )
                .arg(
                    Arg::new("parent")
                        .long("parent")
                        .help("Parent folder ID")
                        .value_name("ID"),
                )
                .arg(
                    Arg::new("name")
                        .long("name")
                        .help("Target filename (defaults to source filename)")
                        .value_name("NAME"),
                )
                .after_help(
                    "\
EXAMPLES:
  gws drive +upload ./report.pdf
  gws drive +upload ./report.pdf --parent FOLDER_ID
  gws drive +upload ./data.csv --name 'Sales Data.csv'

TIPS:
  MIME type is detected automatically.
  Filename is inferred from the local path unless --name is given.",
                ),
        );
        // --- soul-codes helpers ---
        // Add these subcommands inside inject_commands(), before the closing `cmd`

        cmd = cmd.subcommand(
            Command::new("+download")
                .about("[Helper] Download a file by ID")
                .arg(
                    Arg::new("file")
                        .long("file")
                        .help("Drive file ID to download")
                        .required(true)
                        .value_name("ID"),
                )
                .arg(
                    Arg::new("dest")
                        .long("dest")
                        .help("Destination path (default: /tmp/{fileId})")
                        .value_name("PATH"),
                )
                .after_help(
                    "\
EXAMPLES:
  gws drive +download --file 1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgVE2upms
  gws drive +download --file 1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgVE2upms --dest ./report.pdf

TIPS:
  Downloads the file content (binary export).
  For Google Docs/Sheets/Slides, use the export endpoint instead.",
                ),
        );

        cmd = cmd.subcommand(
            Command::new("+revision-get")
                .about("[Helper] Download a specific revision of a file")
                .arg(
                    Arg::new("file")
                        .long("file")
                        .help("Drive file ID")
                        .required(true)
                        .value_name("ID"),
                )
                .arg(
                    Arg::new("revision")
                        .long("revision")
                        .help("Revision ID to download")
                        .required(true)
                        .value_name("ID"),
                )
                .arg(
                    Arg::new("dest")
                        .long("dest")
                        .help("Destination path (default: /tmp/{fileId}-{revisionId})")
                        .value_name("PATH"),
                )
                .after_help(
                    "\
EXAMPLES:
  gws drive +revision-get --file FILE_ID --revision REV_ID
  gws drive +revision-get --file FILE_ID --revision REV_ID --dest ./old-version.pdf

TIPS:
  Use `gws drive revisions list --fileId FILE_ID` to find revision IDs.
  Not all file types support revision downloads.",
                ),
        );
        cmd
    }

    fn handle<'a>(
        &'a self,
        doc: &'a crate::discovery::RestDescription,
        matches: &'a ArgMatches,
        _sanitize_config: &'a crate::helpers::modelarmor::SanitizeConfig,
    ) -> Pin<Box<dyn Future<Output = Result<bool, GwsError>> + Send + 'a>> {
        Box::pin(async move {
            if let Some(matches) = matches.subcommand_matches("+upload") {
                let file_path = matches.get_one::<String>("file").unwrap();
                let parent_id = matches.get_one::<String>("parent");
                let name_arg = matches.get_one::<String>("name");

                // Determine filename
                let filename = determine_filename(file_path, name_arg.map(|s| s.as_str()))?;

                // Find method: files.create
                let files_res = doc
                    .resources
                    .get("files")
                    .ok_or_else(|| GwsError::Discovery("Resource 'files' not found".to_string()))?;
                let create_method = files_res.methods.get("create").ok_or_else(|| {
                    GwsError::Discovery("Method 'files.create' not found".to_string())
                })?;

                // Build metadata
                let metadata = build_metadata(&filename, parent_id.map(|s| s.as_str()));

                let body_str = metadata.to_string();

                let scopes: Vec<&str> = create_method.scopes.iter().map(|s| s.as_str()).collect();
                let (token, auth_method) = match auth::get_token(&scopes).await {
                    Ok(t) => (Some(t), executor::AuthMethod::OAuth),
                    Err(_) if matches.get_flag("dry-run") => (None, executor::AuthMethod::None),
                    Err(e) => return Err(GwsError::Auth(format!("Drive auth failed: {e}"))),
                };

                executor::execute_method(
                    doc,
                    create_method,
                    None,
                    Some(&body_str),
                    token.as_deref(),
                    auth_method,
                    None,
                    Some(executor::UploadSource::File {
                        path: file_path,
                        content_type: None,
                    }),
                    matches.get_flag("dry-run"),
                    &executor::PaginationConfig::default(),
                    None,
                    &crate::helpers::modelarmor::SanitizeMode::Warn,
                    &crate::formatter::OutputFormat::default(),
                    false,
                )
                .await?;

                return Ok(true);
            }
            // Add these blocks inside handle(), before the final `Ok(false)`

            if let Some(matches) = matches.subcommand_matches("+download") {
                handle_download(matches).await?;
                return Ok(true);
            }

            if let Some(matches) = matches.subcommand_matches("+revision-get") {
                handle_revision_get(matches).await?;
                return Ok(true);
            }

            Ok(false)
        })
    }
}

fn determine_filename(file_path: &str, name_arg: Option<&str>) -> Result<String, GwsError> {
    if let Some(n) = name_arg {
        Ok(n.to_string())
    } else {
        Path::new(file_path)
            .file_name()
            .and_then(|n| n.to_str())
            .map(|s| s.to_string())
            .ok_or_else(|| GwsError::Validation("Invalid file path".to_string()))
    }
}

fn build_metadata(filename: &str, parent_id: Option<&str>) -> Value {
    let mut metadata = json!({
        "name": filename
    });

    if let Some(parent) = parent_id {
        metadata["parents"] = json!([parent]);
    }

    metadata
}

// Add these after the existing `build_metadata` function, before `#[cfg(test)]`

const DRIVE_READONLY_SCOPE: &str = "https://www.googleapis.com/auth/drive.readonly";

async fn handle_download(matches: &ArgMatches) -> Result<(), GwsError> {
    let file_id = matches.get_one::<String>("file").unwrap();
    let default_dest = format!("/tmp/{file_id}");
    let dest_path = matches
        .get_one::<String>("dest")
        .map(|s| s.as_str())
        .unwrap_or(&default_dest);

    let token = auth::get_token(&[DRIVE_READONLY_SCOPE])
        .await
        .map_err(|e| GwsError::Auth(format!("Drive auth failed: {e}")))?;

    let client = crate::client::build_client()?;

    let url = format!(
        "https://www.googleapis.com/drive/v3/files/{}?alt=media",
        crate::validate::encode_path_segment(file_id)
    );

    let resp = crate::client::send_with_retry(|| client.get(&url).bearer_auth(&token))
        .await
        .map_err(|e| GwsError::Other(anyhow::anyhow!("Failed to download file: {e}")))?;

    if !resp.status().is_success() {
        let status = resp.status().as_u16();
        let body = resp
            .text()
            .await
            .unwrap_or_else(|_| "(error body unreadable)".to_string());
        return Err(GwsError::Api {
            code: status,
            message: format!("Failed to download file {file_id}: {body}"),
            reason: "downloadFailed".to_string(),
            enable_url: None,
        });
    }

    let bytes = resp
        .bytes()
        .await
        .map_err(|e| GwsError::Other(anyhow::anyhow!("Read error: {e}")))?;

    let mut file = tokio::fs::File::create(dest_path)
        .await
        .map_err(|e| GwsError::Other(anyhow::anyhow!("Cannot create file {dest_path}: {e}")))?;

    use tokio::io::AsyncWriteExt;
    file.write_all(&bytes)
        .await
        .map_err(|e| GwsError::Other(anyhow::anyhow!("Write error: {e}")))?;

    let output = json!({
        "downloaded": file_id,
        "dest": dest_path,
        "bytes": bytes.len(),
    });

    println!(
        "{}",
        serde_json::to_string_pretty(&output)
            .map_err(|e| GwsError::Other(anyhow::anyhow!("JSON serialization error: {e}")))?
    );

    Ok(())
}

async fn handle_revision_get(matches: &ArgMatches) -> Result<(), GwsError> {
    let file_id = matches.get_one::<String>("file").unwrap();
    let revision_id = matches.get_one::<String>("revision").unwrap();
    let default_dest = format!("/tmp/{file_id}-{revision_id}");
    let dest_path = matches
        .get_one::<String>("dest")
        .map(|s| s.as_str())
        .unwrap_or(&default_dest);

    let token = auth::get_token(&[DRIVE_READONLY_SCOPE])
        .await
        .map_err(|e| GwsError::Auth(format!("Drive auth failed: {e}")))?;

    let client = crate::client::build_client()?;

    let url = format!(
        "https://www.googleapis.com/drive/v3/files/{}/revisions/{}?alt=media",
        crate::validate::encode_path_segment(file_id),
        crate::validate::encode_path_segment(revision_id)
    );

    let resp = crate::client::send_with_retry(|| client.get(&url).bearer_auth(&token))
        .await
        .map_err(|e| GwsError::Other(anyhow::anyhow!("Failed to download revision: {e}")))?;

    if !resp.status().is_success() {
        let status = resp.status().as_u16();
        let body = resp
            .text()
            .await
            .unwrap_or_else(|_| "(error body unreadable)".to_string());
        return Err(GwsError::Api {
            code: status,
            message: format!("Failed to download revision {revision_id} of file {file_id}: {body}"),
            reason: "downloadFailed".to_string(),
            enable_url: None,
        });
    }

    let bytes = resp
        .bytes()
        .await
        .map_err(|e| GwsError::Other(anyhow::anyhow!("Read error: {e}")))?;

    let mut file = tokio::fs::File::create(dest_path)
        .await
        .map_err(|e| GwsError::Other(anyhow::anyhow!("Cannot create file {dest_path}: {e}")))?;

    use tokio::io::AsyncWriteExt;
    file.write_all(&bytes)
        .await
        .map_err(|e| GwsError::Other(anyhow::anyhow!("Write error: {e}")))?;

    let output = json!({
        "downloaded": file_id,
        "revision": revision_id,
        "dest": dest_path,
        "bytes": bytes.len(),
    });

    println!(
        "{}",
        serde_json::to_string_pretty(&output)
            .map_err(|e| GwsError::Other(anyhow::anyhow!("JSON serialization error: {e}")))?
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_determine_filename_explicit() {
        assert_eq!(
            determine_filename("path/to/file.txt", Some("custom.txt")).unwrap(),
            "custom.txt"
        );
    }

    #[test]
    fn test_determine_filename_from_path() {
        assert_eq!(
            determine_filename("path/to/file.txt", None).unwrap(),
            "file.txt"
        );
    }

    #[test]
    fn test_determine_filename_invalid_path() {
        assert!(determine_filename("", None).is_err());
        assert!(determine_filename("/", None).is_err()); // Root has no filename component usually
    }

    #[test]
    fn test_build_metadata_no_parent() {
        let meta = build_metadata("file.txt", None);
        assert_eq!(meta["name"], "file.txt");
        assert!(meta.get("parents").is_none());
    }

    #[test]
    fn test_build_metadata_with_parent() {
        let meta = build_metadata("file.txt", Some("folder123"));
        assert_eq!(meta["name"], "file.txt");
        assert_eq!(meta["parents"][0], "folder123");
    }

    // --- soul-codes helper tests ---
    // Add these inside the existing `#[cfg(test)] mod tests { ... }` block

    #[test]
    fn test_download_default_dest() {
        let file_id = "abc123";
        let default_dest = format!("/tmp/{file_id}");
        assert_eq!(default_dest, "/tmp/abc123");
    }

    #[test]
    fn test_revision_get_default_dest() {
        let file_id = "abc123";
        let revision_id = "rev456";
        let default_dest = format!("/tmp/{file_id}-{revision_id}");
        assert_eq!(default_dest, "/tmp/abc123-rev456");
    }
}
