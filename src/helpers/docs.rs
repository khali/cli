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
use std::pin::Pin;

pub struct DocsHelper;

impl Helper for DocsHelper {
    fn inject_commands(
        &self,
        mut cmd: Command,
        _doc: &crate::discovery::RestDescription,
    ) -> Command {
        cmd = cmd.subcommand(
            Command::new("+write")
                .about("[Helper] Append text to a document")
                .arg(
                    Arg::new("document")
                        .long("document")
                        .help("Document ID")
                        .required(true)
                        .value_name("ID"),
                )
                .arg(
                    Arg::new("text")
                        .long("text")
                        .help("Text to append (plain text)")
                        .required(true)
                        .value_name("TEXT"),
                )
                .after_help(
                    "\
EXAMPLES:
  gws docs +write --document DOC_ID --text 'Hello, world!'

TIPS:
  Text is inserted at the end of the document body.
  For rich formatting, use the raw batchUpdate API instead.",
                ),
        );
        cmd = cmd.subcommand(
            Command::new("+revisions")
                .about("[Helper] List revision history of a document")
                .arg(
                    Arg::new("document")
                        .long("document")
                        .help("Document ID")
                        .required(true)
                        .value_name("ID"),
                )
                .arg(
                    Arg::new("limit")
                        .long("limit")
                        .help("Maximum number of revisions to return (default: 20)")
                        .value_name("N")
                        .value_parser(clap::value_parser!(u32)),
                )
                .after_help(
                    "\
EXAMPLES:
  gws docs +revisions --document DOC_ID
  gws docs +revisions --document DOC_ID --limit 5
  gws docs +revisions --document DOC_ID --format table

TIPS:
  The document ID is the long string in the Google Docs URL.
  Returns metadata for each revision: ID, modified time, author, and
  whether the revision is kept forever.
  Note: the full content of past revisions is not accessible via the
  Google API for native Docs files. Use the Google Docs UI (File →
  Version history) to view or restore specific versions.",
                ),
        );
        // --- soul-codes helpers ---
        // Add these inside inject_commands() after the existing "+revisions" subcommand block,
        // before the final `cmd` return statement:

        cmd = cmd.subcommand(
            Command::new("+find-replace")
                .about("[Helper] Find and replace text in a document")
                .arg(
                    Arg::new("document")
                        .long("document")
                        .help("Document ID")
                        .required(true)
                        .value_name("ID"),
                )
                .arg(
                    Arg::new("find")
                        .long("find")
                        .help("Text to find")
                        .required(true)
                        .value_name("TEXT"),
                )
                .arg(
                    Arg::new("replace")
                        .long("replace")
                        .help("Replacement text")
                        .required(true)
                        .value_name("TEXT"),
                )
                .arg(
                    Arg::new("match-case")
                        .long("match-case")
                        .help("Match case when searching (default: false)")
                        .action(clap::ArgAction::SetTrue),
                )
                .after_help(
                    "\
EXAMPLES:
  gws docs +find-replace --document DOC_ID --find 'old text' --replace 'new text'
  gws docs +find-replace --document DOC_ID --find 'Hello' --replace 'Hi' --match-case

TIPS:
  Returns JSON with the number of occurrences changed.
  Without --match-case, search is case-insensitive.",
                ),
        );
        cmd = cmd.subcommand(
            Command::new("+insert-link")
                .about("[Helper] Add a hyperlink to existing text in a document")
                .arg(
                    Arg::new("document")
                        .long("document")
                        .help("Document ID")
                        .required(true)
                        .value_name("ID"),
                )
                .arg(
                    Arg::new("text")
                        .long("text")
                        .help("Text to find in the document and link")
                        .required(true)
                        .value_name("TEXT"),
                )
                .arg(
                    Arg::new("url")
                        .long("url")
                        .help("URL to link to")
                        .required(true)
                        .value_name("URL"),
                )
                .after_help(
                    "\
EXAMPLES:
  gws docs +insert-link --document DOC_ID --text 'click here' --url 'https://example.com'

TIPS:
  Finds the first occurrence of the text in the document body and applies a hyperlink.
  Returns error if the text is not found.",
                ),
        );
        cmd = cmd.subcommand(
            Command::new("+set-style")
                .about(
                    "[Helper] Set paragraph style (heading level) for paragraphs containing text",
                )
                .arg(
                    Arg::new("document")
                        .long("document")
                        .help("Document ID")
                        .required(true)
                        .value_name("ID"),
                )
                .arg(
                    Arg::new("find")
                        .long("find")
                        .help("Text contained in the target paragraph(s)")
                        .required(true)
                        .value_name("TEXT"),
                )
                .arg(
                    Arg::new("style")
                        .long("style")
                        .help("Named style to apply")
                        .required(true)
                        .value_name("STYLE")
                        .value_parser([
                            "NORMAL_TEXT",
                            "HEADING_1",
                            "HEADING_2",
                            "HEADING_3",
                            "HEADING_4",
                            "HEADING_5",
                            "HEADING_6",
                            "TITLE",
                            "SUBTITLE",
                        ]),
                )
                .after_help(
                    "\
EXAMPLES:
  gws docs +set-style --document DOC_ID --find 'Introduction' --style HEADING_1
  gws docs +set-style --document DOC_ID --find 'Summary' --style HEADING_2

TIPS:
  Finds all paragraphs containing the search text and applies the named style.
  Valid styles: NORMAL_TEXT, HEADING_1..6, TITLE, SUBTITLE.",
                ),
        );
        cmd = cmd.subcommand(
            Command::new("+table-read")
                .about("[Helper] Read a table from a document as JSON")
                .arg(
                    Arg::new("document")
                        .long("document")
                        .help("Document ID")
                        .required(true)
                        .value_name("ID"),
                )
                .arg(
                    Arg::new("table-index")
                        .long("table-index")
                        .help("0-based index of the table in the document (default: 0)")
                        .value_name("N")
                        .value_parser(clap::value_parser!(usize)),
                )
                .after_help(
                    "\
EXAMPLES:
  gws docs +table-read --document DOC_ID
  gws docs +table-read --document DOC_ID --table-index 2

TIPS:
  Returns JSON with rows (array of arrays), rowCount, and columnCount.
  Use +table-list first to see all tables and pick the right index.",
                ),
        );
        cmd = cmd.subcommand(
            Command::new("+table-list")
                .about("[Helper] List all tables in a document")
                .arg(
                    Arg::new("document")
                        .long("document")
                        .help("Document ID")
                        .required(true)
                        .value_name("ID"),
                )
                .after_help(
                    "\
EXAMPLES:
  gws docs +table-list --document DOC_ID

TIPS:
  Returns a JSON array with metadata for each table: index, rows, cols, and header texts.
  Use the index value with +table-read to read a specific table.",
                ),
        );
        cmd = cmd.subcommand(
            Command::new("+table-add-column")
                .about("[Helper] Add a column to a table in a document")
                .arg(
                    Arg::new("document")
                        .long("document")
                        .help("Document ID")
                        .required(true)
                        .value_name("ID"),
                )
                .arg(
                    Arg::new("table-index")
                        .long("table-index")
                        .help("0-based index of the table (default: 0)")
                        .value_name("N")
                        .value_parser(clap::value_parser!(usize)),
                )
                .arg(
                    Arg::new("after-col")
                        .long("after-col")
                        .help("Insert after this 0-based column index (default: last column)")
                        .value_name("N")
                        .value_parser(clap::value_parser!(usize)),
                )
                .after_help(
                    "\
EXAMPLES:
  gws docs +table-add-column --document DOC_ID
  gws docs +table-add-column --document DOC_ID --table-index 1 --after-col 0

TIPS:
  Inserts a new column to the right of the specified column index.
  If --after-col is omitted, the column is added after the last existing column.",
                ),
        );
        // Add these inside DocsHelper::inject_commands(), after the existing +revisions subcommand:

        cmd = cmd.subcommand(
            Command::new("+table-write")
                .about("[Helper] Write text to a specific table cell")
                .arg(
                    Arg::new("document")
                        .long("document")
                        .help("Document ID")
                        .required(true)
                        .value_name("ID"),
                )
                .arg(
                    Arg::new("table-index")
                        .long("table-index")
                        .help("0-based index of the table in the document (default: 0)")
                        .value_name("N")
                        .value_parser(clap::value_parser!(usize))
                        .default_value("0"),
                )
                .arg(
                    Arg::new("row")
                        .long("row")
                        .help("0-based row index")
                        .required(true)
                        .value_name("N")
                        .value_parser(clap::value_parser!(usize)),
                )
                .arg(
                    Arg::new("col")
                        .long("col")
                        .help("0-based column index")
                        .required(true)
                        .value_name("N")
                        .value_parser(clap::value_parser!(usize)),
                )
                .arg(
                    Arg::new("text")
                        .long("text")
                        .help("Text to write into the cell")
                        .required(true)
                        .value_name("TEXT"),
                )
                .after_help(
                    "\
EXAMPLES:
  gws docs +table-write --document DOC_ID --row 0 --col 1 --text 'New value'
  gws docs +table-write --document DOC_ID --table-index 1 --row 2 --col 0 --text 'Updated'

TIPS:
  Replaces existing cell content. Row and column indices are 0-based.
  Use +table-read to inspect current table content first.",
                ),
        );
        cmd = cmd.subcommand(
            Command::new("+insert-table-row")
                .about("[Helper] Insert a new row into a table")
                .arg(
                    Arg::new("document")
                        .long("document")
                        .help("Document ID")
                        .required(true)
                        .value_name("ID"),
                )
                .arg(
                    Arg::new("table-index")
                        .long("table-index")
                        .help("0-based index of the table in the document (default: 0)")
                        .value_name("N")
                        .value_parser(clap::value_parser!(usize))
                        .default_value("0"),
                )
                .arg(
                    Arg::new("after-row")
                        .long("after-row")
                        .help("Insert after this 0-based row (default: after last row)")
                        .value_name("N")
                        .value_parser(clap::value_parser!(usize)),
                )
                .after_help(
                    "\
EXAMPLES:
  gws docs +insert-table-row --document DOC_ID
  gws docs +insert-table-row --document DOC_ID --after-row 0
  gws docs +insert-table-row --document DOC_ID --table-index 2 --after-row 3

TIPS:
  Without --after-row, inserts after the last row.
  The new row will have the same number of columns as the existing rows.",
                ),
        );
        cmd = cmd.subcommand(
            Command::new("+delete-table-row")
                .about("[Helper] Delete a row from a table")
                .arg(
                    Arg::new("document")
                        .long("document")
                        .help("Document ID")
                        .required(true)
                        .value_name("ID"),
                )
                .arg(
                    Arg::new("table-index")
                        .long("table-index")
                        .help("0-based index of the table in the document (default: 0)")
                        .value_name("N")
                        .value_parser(clap::value_parser!(usize))
                        .default_value("0"),
                )
                .arg(
                    Arg::new("row")
                        .long("row")
                        .help("0-based row index to delete")
                        .required(true)
                        .value_name("N")
                        .value_parser(clap::value_parser!(usize)),
                )
                .after_help(
                    "\
EXAMPLES:
  gws docs +delete-table-row --document DOC_ID --row 2
  gws docs +delete-table-row --document DOC_ID --table-index 1 --row 0

TIPS:
  Row index is 0-based. Be careful deleting row 0 if it contains headers.",
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
            if let Some(matches) = matches.subcommand_matches("+write") {
                let (params_str, body_str, scopes) = build_write_request(matches, doc)?;

                let scope_strs: Vec<&str> = scopes.iter().map(|s| s.as_str()).collect();
                let (token, auth_method) = match auth::get_token(&scope_strs).await {
                    Ok(t) => (Some(t), executor::AuthMethod::OAuth),
                    Err(_) if matches.get_flag("dry-run") => (None, executor::AuthMethod::None),
                    Err(e) => {
                        return Err(GwsError::Auth(format!(
                            "Docs auth failed: {}",
                            crate::output::sanitize_for_terminal(&e.to_string())
                        )))
                    }
                };

                // Method: documents.batchUpdate
                let documents_res = doc.resources.get("documents").ok_or_else(|| {
                    GwsError::Discovery("Resource 'documents' not found".to_string())
                })?;
                let batch_update_method =
                    documents_res.methods.get("batchUpdate").ok_or_else(|| {
                        GwsError::Discovery("Method 'documents.batchUpdate' not found".to_string())
                    })?;

                let pagination = executor::PaginationConfig {
                    page_all: false,
                    page_limit: 10,
                    page_delay_ms: 100,
                };

                executor::execute_method(
                    doc,
                    batch_update_method,
                    Some(&params_str),
                    Some(&body_str),
                    token.as_deref(),
                    auth_method,
                    None,
                    None,
                    matches.get_flag("dry-run"),
                    &pagination,
                    None,
                    &crate::helpers::modelarmor::SanitizeMode::Warn,
                    &crate::formatter::OutputFormat::default(),
                    false,
                )
                .await?;

                return Ok(true);
            }

            if let Some(matches) = matches.subcommand_matches("+revisions") {
                handle_revisions(matches).await?;
                return Ok(true);
            }

            // Add these inside handle() BEFORE the final `Ok(false)`, after the existing
            // "+revisions" dispatch block:

            if let Some(matches) = matches.subcommand_matches("+find-replace") {
                handle_find_replace(matches).await?;
                return Ok(true);
            }

            if let Some(matches) = matches.subcommand_matches("+insert-link") {
                handle_insert_link(matches).await?;
                return Ok(true);
            }

            if let Some(matches) = matches.subcommand_matches("+set-style") {
                handle_set_style(matches).await?;
                return Ok(true);
            }

            if let Some(matches) = matches.subcommand_matches("+table-read") {
                handle_table_read(matches).await?;
                return Ok(true);
            }

            if let Some(matches) = matches.subcommand_matches("+table-list") {
                handle_table_list(matches).await?;
                return Ok(true);
            }

            if let Some(matches) = matches.subcommand_matches("+table-add-column") {
                handle_table_add_column(matches).await?;
                return Ok(true);
            }

            // Add these inside DocsHelper::handle(), after the existing +revisions dispatch:

            if let Some(matches) = matches.subcommand_matches("+table-write") {
                handle_table_write(matches).await?;
                return Ok(true);
            }

            if let Some(matches) = matches.subcommand_matches("+insert-table-row") {
                handle_insert_table_row(matches).await?;
                return Ok(true);
            }

            if let Some(matches) = matches.subcommand_matches("+delete-table-row") {
                handle_delete_table_row(matches).await?;
                return Ok(true);
            }

            Ok(false)
        })
    }
}

async fn handle_revisions(matches: &ArgMatches) -> Result<(), GwsError> {
    let document_id = matches.get_one::<String>("document").unwrap();
    let limit = matches.get_one::<u32>("limit").copied().unwrap_or(20);

    let scope = "https://www.googleapis.com/auth/drive.readonly";
    let token = auth::get_token(&[scope]).await.map_err(|e| {
        GwsError::Auth(format!(
            "Docs auth failed: {}",
            crate::output::sanitize_for_terminal(&e.to_string())
        ))
    })?;

    let client = crate::client::build_client()?;
    let limit_str = limit.to_string();

    let resp = client
        .get(format!(
            "https://www.googleapis.com/drive/v3/files/{}/revisions",
            document_id
        ))
        .query(&[
            (
                "fields",
                "revisions(id,modifiedTime,lastModifyingUser/displayName,keepForever,size)",
            ),
            ("pageSize", limit_str.as_str()),
        ])
        .bearer_auth(&token)
        .send()
        .await
        .map_err(|e| GwsError::Other(anyhow::anyhow!("HTTP request failed: {e}")))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(GwsError::Api {
            code: status.as_u16(),
            message: body,
            reason: "revisions_request_failed".to_string(),
            enable_url: None,
        });
    }

    let value: Value = resp
        .json()
        .await
        .map_err(|e| GwsError::Other(anyhow::anyhow!("JSON parse failed: {e}")))?;

    let fmt = matches
        .get_one::<String>("format")
        .map(|s| crate::formatter::OutputFormat::from_str(s))
        .unwrap_or_default();
    println!("{}", crate::formatter::format_value(&value, &fmt));
    Ok(())
}

// Add these after the existing build_write_request function, before #[cfg(test)]:

/// Fetch a Google Doc by ID. Returns the parsed JSON body.
async fn fetch_document(document_id: &str) -> Result<Value, GwsError> {
    let scope = "https://www.googleapis.com/auth/documents";
    let token = auth::get_token(&[scope]).await.map_err(|e| {
        GwsError::Auth(format!(
            "Docs auth failed: {}",
            crate::output::sanitize_for_terminal(&e.to_string())
        ))
    })?;

    let client = crate::client::build_client()?;
    let url = format!("https://docs.googleapis.com/v1/documents/{}", document_id);

    let resp = client
        .get(&url)
        .bearer_auth(&token)
        .send()
        .await
        .map_err(|e| GwsError::Other(anyhow::anyhow!("HTTP request failed: {e}")))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(GwsError::Api {
            code: status.as_u16(),
            message: body,
            reason: "document_fetch_failed".to_string(),
            enable_url: None,
        });
    }

    resp.json()
        .await
        .map_err(|e| GwsError::Other(anyhow::anyhow!("JSON parse failed: {e}")))
}

/// Send a batchUpdate request to a Google Doc. Returns the parsed JSON response.
async fn batch_update(document_id: &str, requests: Value) -> Result<Value, GwsError> {
    let scope = "https://www.googleapis.com/auth/documents";
    let token = auth::get_token(&[scope]).await.map_err(|e| {
        GwsError::Auth(format!(
            "Docs auth failed: {}",
            crate::output::sanitize_for_terminal(&e.to_string())
        ))
    })?;

    let client = crate::client::build_client()?;
    let url = format!(
        "https://docs.googleapis.com/v1/documents/{}:batchUpdate",
        document_id
    );

    let body = json!({ "requests": requests });

    let resp = client
        .post(&url)
        .bearer_auth(&token)
        .json(&body)
        .send()
        .await
        .map_err(|e| GwsError::Other(anyhow::anyhow!("HTTP request failed: {e}")))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body_text = resp.text().await.unwrap_or_default();
        return Err(GwsError::Api {
            code: status.as_u16(),
            message: body_text,
            reason: "batch_update_failed".to_string(),
            enable_url: None,
        });
    }

    resp.json()
        .await
        .map_err(|e| GwsError::Other(anyhow::anyhow!("JSON parse failed: {e}")))
}

/// Extract all text runs from a document body. Returns Vec<(content, startIndex, endIndex)>.
fn extract_text_runs(doc: &Value) -> Vec<(String, i64, i64)> {
    let mut runs = Vec::new();
    if let Some(content) = doc["body"]["content"].as_array() {
        for element in content {
            if let Some(paragraph) = element.get("paragraph") {
                if let Some(elements) = paragraph["elements"].as_array() {
                    for el in elements {
                        if let Some(text_run) = el.get("textRun") {
                            let text = text_run["content"].as_str().unwrap_or_default().to_string();
                            let start = el["startIndex"].as_i64().unwrap_or(0);
                            let end = el["endIndex"].as_i64().unwrap_or(0);
                            runs.push((text, start, end));
                        }
                    }
                }
            }
            // Also walk table cells
            if let Some(table) = element.get("table") {
                if let Some(rows) = table["tableRows"].as_array() {
                    for row in rows {
                        if let Some(cells) = row["tableCells"].as_array() {
                            for cell in cells {
                                if let Some(cell_content) = cell["content"].as_array() {
                                    for para in cell_content {
                                        if let Some(paragraph) = para.get("paragraph") {
                                            if let Some(elements) = paragraph["elements"].as_array()
                                            {
                                                for el in elements {
                                                    if let Some(text_run) = el.get("textRun") {
                                                        let text = text_run["content"]
                                                            .as_str()
                                                            .unwrap_or_default()
                                                            .to_string();
                                                        let start =
                                                            el["startIndex"].as_i64().unwrap_or(0);
                                                        let end =
                                                            el["endIndex"].as_i64().unwrap_or(0);
                                                        runs.push((text, start, end));
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    runs
}

/// Extract paragraph ranges from a document body.
/// Returns Vec<(paragraph_text, startIndex, endIndex)> for paragraphs containing search text.
fn find_paragraphs_containing(doc: &Value, search: &str) -> Vec<(String, i64, i64)> {
    let mut results = Vec::new();
    if let Some(content) = doc["body"]["content"].as_array() {
        for element in content {
            if let Some(paragraph) = element.get("paragraph") {
                let start = element["startIndex"].as_i64().unwrap_or(0);
                let end = element["endIndex"].as_i64().unwrap_or(0);
                // Collect full paragraph text from its elements
                let mut para_text = String::new();
                if let Some(elements) = paragraph["elements"].as_array() {
                    for el in elements {
                        if let Some(text_run) = el.get("textRun") {
                            para_text.push_str(text_run["content"].as_str().unwrap_or_default());
                        }
                    }
                }
                if para_text.contains(search) {
                    results.push((para_text, start, end));
                }
            }
        }
    }
    results
}

/// Find tables in document body content.
/// Returns Vec<(table_element_start_index, &Value)> where Value is the table object.
fn find_tables(doc: &Value) -> Vec<(i64, &Value)> {
    let mut tables = Vec::new();
    if let Some(content) = doc["body"]["content"].as_array() {
        for element in content {
            if element.get("table").is_some() {
                let start = element["startIndex"].as_i64().unwrap_or(0);
                tables.push((start, &element["table"]));
            }
        }
    }
    tables
}

/// Extract cell text from a table cell Value.
fn extract_cell_text(cell: &Value) -> String {
    let mut text = String::new();
    if let Some(content) = cell["content"].as_array() {
        for para in content {
            if let Some(paragraph) = para.get("paragraph") {
                if let Some(elements) = paragraph["elements"].as_array() {
                    for el in elements {
                        if let Some(text_run) = el.get("textRun") {
                            text.push_str(text_run["content"].as_str().unwrap_or_default());
                        }
                    }
                }
            }
        }
    }
    // Trim trailing newline that Docs always adds
    text.trim_end_matches('\n').to_string()
}

/// Read a table into a 2D vec of strings.
fn read_table(table: &Value) -> (Vec<Vec<String>>, usize, usize) {
    let mut rows_data: Vec<Vec<String>> = Vec::new();
    let mut max_cols: usize = 0;
    if let Some(rows) = table["tableRows"].as_array() {
        for row in rows {
            let mut row_data: Vec<String> = Vec::new();
            if let Some(cells) = row["tableCells"].as_array() {
                for cell in cells {
                    row_data.push(extract_cell_text(cell));
                }
                if row_data.len() > max_cols {
                    max_cols = row_data.len();
                }
            }
            rows_data.push(row_data);
        }
    }
    let row_count = rows_data.len();
    (rows_data, row_count, max_cols)
}

// ---------------------------------------------------------------------------
// Handler: +find-replace
// ---------------------------------------------------------------------------
async fn handle_find_replace(matches: &ArgMatches) -> Result<(), GwsError> {
    let document_id = matches.get_one::<String>("document").unwrap();
    let find_text = matches.get_one::<String>("find").unwrap();
    let replace_text = matches.get_one::<String>("replace").unwrap();
    let match_case = matches.get_flag("match-case");

    let requests = json!([{
        "replaceAllText": {
            "containsText": {
                "text": find_text,
                "matchCase": match_case
            },
            "replaceText": replace_text
        }
    }]);

    let result = batch_update(document_id, requests).await?;

    // Extract occurrencesChanged from the reply
    let occurrences = result["replies"]
        .as_array()
        .and_then(|replies| replies.first())
        .and_then(|r| r["replaceAllText"]["occurrencesChanged"].as_i64())
        .unwrap_or(0);

    let output = json!({
        "occurrencesChanged": occurrences,
        "find": find_text,
        "replace": replace_text,
        "matchCase": match_case
    });

    println!(
        "{}",
        serde_json::to_string_pretty(&output)
            .map_err(|e| { GwsError::Other(anyhow::anyhow!("JSON serialization failed: {e}")) })?
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Handler: +insert-link
// ---------------------------------------------------------------------------
async fn handle_insert_link(matches: &ArgMatches) -> Result<(), GwsError> {
    let document_id = matches.get_one::<String>("document").unwrap();
    let search_text = matches.get_one::<String>("text").unwrap();
    let url = matches.get_one::<String>("url").unwrap();

    // Step 1: Fetch document to find text position
    let doc = fetch_document(document_id).await?;
    let runs = extract_text_runs(&doc);

    // Find the first run containing the search text
    let mut found_start: Option<i64> = None;
    let mut found_end: Option<i64> = None;

    for (content, run_start, _run_end) in &runs {
        if let Some(offset) = content.find(search_text.as_str()) {
            let byte_offset = content[..offset].chars().count() as i64;
            found_start = Some(run_start + byte_offset);
            found_end = Some(run_start + byte_offset + search_text.chars().count() as i64);
            break;
        }
    }

    let start_index = found_start.ok_or_else(|| {
        GwsError::Other(anyhow::anyhow!(
            "Text not found in document: {}",
            search_text
        ))
    })?;
    let end_index = found_end.unwrap();

    // Step 2: Apply link via batchUpdate
    let requests = json!([{
        "updateTextStyle": {
            "range": {
                "startIndex": start_index,
                "endIndex": end_index
            },
            "textStyle": {
                "link": {
                    "url": url
                }
            },
            "fields": "link"
        }
    }]);

    batch_update(document_id, requests).await?;

    let output = json!({
        "linked": search_text,
        "url": url,
        "startIndex": start_index,
        "endIndex": end_index
    });

    println!(
        "{}",
        serde_json::to_string_pretty(&output)
            .map_err(|e| { GwsError::Other(anyhow::anyhow!("JSON serialization failed: {e}")) })?
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Handler: +set-style
// ---------------------------------------------------------------------------
async fn handle_set_style(matches: &ArgMatches) -> Result<(), GwsError> {
    let document_id = matches.get_one::<String>("document").unwrap();
    let find_text = matches.get_one::<String>("find").unwrap();
    let style = matches.get_one::<String>("style").unwrap();

    // Step 1: Fetch document to find paragraphs
    let doc = fetch_document(document_id).await?;
    let paragraphs = find_paragraphs_containing(&doc, find_text);

    if paragraphs.is_empty() {
        return Err(GwsError::Other(anyhow::anyhow!(
            "No paragraphs found containing text: {}",
            find_text
        )));
    }

    // Step 2: Build updateParagraphStyle requests for each matching paragraph
    let requests: Vec<Value> = paragraphs
        .iter()
        .map(|(_text, start, end)| {
            json!({
                "updateParagraphStyle": {
                    "range": {
                        "startIndex": start,
                        "endIndex": end
                    },
                    "paragraphStyle": {
                        "namedStyleType": style
                    },
                    "fields": "namedStyleType"
                }
            })
        })
        .collect();

    let count = requests.len();
    batch_update(document_id, Value::Array(requests)).await?;

    let output = json!({ "updated": count });
    println!(
        "{}",
        serde_json::to_string_pretty(&output)
            .map_err(|e| { GwsError::Other(anyhow::anyhow!("JSON serialization failed: {e}")) })?
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Handler: +table-read
// ---------------------------------------------------------------------------
async fn handle_table_read(matches: &ArgMatches) -> Result<(), GwsError> {
    let document_id = matches.get_one::<String>("document").unwrap();
    let table_index = matches
        .get_one::<usize>("table-index")
        .copied()
        .unwrap_or(0);

    let doc = fetch_document(document_id).await?;
    let tables = find_tables(&doc);

    if table_index >= tables.len() {
        return Err(GwsError::Other(anyhow::anyhow!(
            "Table index {} out of range (document has {} table(s))",
            table_index,
            tables.len()
        )));
    }

    let (_start_idx, table) = tables[table_index];
    let (rows_data, row_count, col_count) = read_table(table);

    let output = json!({
        "rows": rows_data,
        "rowCount": row_count,
        "columnCount": col_count
    });

    println!(
        "{}",
        serde_json::to_string_pretty(&output)
            .map_err(|e| { GwsError::Other(anyhow::anyhow!("JSON serialization failed: {e}")) })?
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Handler: +table-list
// ---------------------------------------------------------------------------
async fn handle_table_list(matches: &ArgMatches) -> Result<(), GwsError> {
    let document_id = matches.get_one::<String>("document").unwrap();

    let doc = fetch_document(document_id).await?;
    let tables = find_tables(&doc);

    let mut output: Vec<Value> = Vec::new();
    for (idx, (_start, table)) in tables.iter().enumerate() {
        let (rows_data, row_count, col_count) = read_table(table);
        let headers: Vec<String> = if !rows_data.is_empty() {
            rows_data[0].clone()
        } else {
            Vec::new()
        };
        output.push(json!({
            "index": idx,
            "rows": row_count,
            "cols": col_count,
            "headers": headers
        }));
    }

    println!(
        "{}",
        serde_json::to_string_pretty(&Value::Array(output))
            .map_err(|e| { GwsError::Other(anyhow::anyhow!("JSON serialization failed: {e}")) })?
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Handler: +table-add-column
// ---------------------------------------------------------------------------
async fn handle_table_add_column(matches: &ArgMatches) -> Result<(), GwsError> {
    let document_id = matches.get_one::<String>("document").unwrap();
    let table_index = matches
        .get_one::<usize>("table-index")
        .copied()
        .unwrap_or(0);

    // Step 1: Fetch document to find table metadata
    let doc = fetch_document(document_id).await?;
    let tables = find_tables(&doc);

    if table_index >= tables.len() {
        return Err(GwsError::Other(anyhow::anyhow!(
            "Table index {} out of range (document has {} table(s))",
            table_index,
            tables.len()
        )));
    }

    let (table_start_index, table) = tables[table_index];
    let (_rows_data, _row_count, col_count) = read_table(table);

    // Default: insert after last column
    let after_col = matches
        .get_one::<usize>("after-col")
        .copied()
        .unwrap_or_else(|| if col_count > 0 { col_count - 1 } else { 0 });

    // insertRight=true means insert a column to the right of the specified column
    let requests = json!([{
        "insertTableColumn": {
            "tableCellLocation": {
                "tableStartLocation": {
                    "index": table_start_index
                },
                "columnIndex": after_col,
                "rowIndex": 0
            },
            "insertRight": true
        }
    }]);

    batch_update(document_id, requests).await?;

    let output = json!({ "columnsAfter": col_count + 1 });
    println!(
        "{}",
        serde_json::to_string_pretty(&output)
            .map_err(|e| { GwsError::Other(anyhow::anyhow!("JSON serialization failed: {e}")) })?
    );
    Ok(())
}

/// Send a batchUpdate request to a Google Doc.
async fn send_batch_update(doc_id: &str, requests: Vec<Value>) -> Result<Value, GwsError> {
    let token = auth::get_token(&["https://www.googleapis.com/auth/documents"])
        .await
        .map_err(|e| {
            GwsError::Auth(format!(
                "Docs auth failed: {}",
                crate::output::sanitize_for_terminal(&e.to_string()),
            ))
        })?;
    let client = crate::client::build_client()?;
    let resp = client
        .post(format!(
            "https://docs.googleapis.com/v1/documents/{}:batchUpdate",
            doc_id
        ))
        .bearer_auth(&token)
        .json(&json!({ "requests": requests }))
        .send()
        .await
        .map_err(|e| GwsError::Other(anyhow::anyhow!("HTTP request failed: {e}")))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(GwsError::Api {
            code: status.as_u16(),
            message: body,
            reason: "batchUpdate_failed".to_string(),
            enable_url: None,
        });
    }

    let result: Value = resp
        .json()
        .await
        .map_err(|e| GwsError::Other(anyhow::anyhow!("JSON parse failed: {e}")))?;
    Ok(result)
}

/// Find the startIndex of the Nth table (0-based) in a document.
fn find_table_start_index(doc_val: &Value, table_index: usize) -> Result<i64, GwsError> {
    let content = doc_val["body"]["content"]
        .as_array()
        .ok_or_else(|| GwsError::Other(anyhow::anyhow!("No body content")))?;
    let tables: Vec<&Value> = content
        .iter()
        .filter(|el| el.get("table").is_some())
        .collect();
    let table = tables.get(table_index).ok_or_else(|| {
        GwsError::Other(anyhow::anyhow!(
            "Table index {} not found (document has {} tables)",
            table_index,
            tables.len()
        ))
    })?;
    let start_index = table["startIndex"]
        .as_i64()
        .ok_or_else(|| GwsError::Other(anyhow::anyhow!("Table has no startIndex")))?;
    Ok(start_index)
}

/// Get the row count for the Nth table in a document.
fn get_table_row_count(doc_val: &Value, table_index: usize) -> Result<usize, GwsError> {
    let content = doc_val["body"]["content"]
        .as_array()
        .ok_or_else(|| GwsError::Other(anyhow::anyhow!("No body content")))?;
    let tables: Vec<&Value> = content
        .iter()
        .filter(|el| el.get("table").is_some())
        .collect();
    let table = tables.get(table_index).ok_or_else(|| {
        GwsError::Other(anyhow::anyhow!(
            "Table index {} not found (document has {} tables)",
            table_index,
            tables.len()
        ))
    })?;
    let rows = table["table"]["tableRows"]
        .as_array()
        .ok_or_else(|| GwsError::Other(anyhow::anyhow!("No tableRows in table")))?;
    Ok(rows.len())
}

/// Find the content range (startIndex, endIndex - 1) for a specific cell in a table.
///
/// In a Google Docs table, cell.content is an array of structural elements (paragraphs).
/// Each paragraph has a startIndex and endIndex.
/// To replace cell text: deleteContentRange(start..end-1) then insertText at start.
fn find_cell_range(
    doc_val: &Value,
    table_index: usize,
    row: usize,
    col: usize,
) -> Result<(i64, i64), GwsError> {
    let content = doc_val["body"]["content"]
        .as_array()
        .ok_or_else(|| GwsError::Other(anyhow::anyhow!("No body content")))?;
    let tables: Vec<&Value> = content
        .iter()
        .filter(|el| el.get("table").is_some())
        .collect();
    let table = tables
        .get(table_index)
        .ok_or_else(|| GwsError::Other(anyhow::anyhow!("Table {} not found", table_index)))?;
    let table_rows = table["table"]["tableRows"]
        .as_array()
        .ok_or_else(|| GwsError::Other(anyhow::anyhow!("No tableRows")))?;
    let table_row = table_rows
        .get(row)
        .ok_or_else(|| GwsError::Other(anyhow::anyhow!("Row {} not found", row)))?;
    let cells = table_row["tableCells"]
        .as_array()
        .ok_or_else(|| GwsError::Other(anyhow::anyhow!("No tableCells")))?;
    let cell = cells
        .get(col)
        .ok_or_else(|| GwsError::Other(anyhow::anyhow!("Col {} not found", col)))?;
    let cell_content = cell["content"]
        .as_array()
        .ok_or_else(|| GwsError::Other(anyhow::anyhow!("No cell content")))?;
    // Cell content contains paragraphs. We want the range of all content within the cell.
    let first_para = cell_content
        .first()
        .ok_or_else(|| GwsError::Other(anyhow::anyhow!("Empty cell content")))?;
    let last_para = cell_content.last().unwrap();
    let start = first_para["startIndex"].as_i64().unwrap_or(0);
    let end = last_para["endIndex"].as_i64().unwrap_or(start + 1);
    // The actual text content is start..end-1 (end-1 is the paragraph end marker)
    Ok((start, end - 1))
}

async fn handle_table_write(matches: &ArgMatches) -> Result<(), GwsError> {
    let doc_id = matches.get_one::<String>("document").unwrap();
    let table_index = matches
        .get_one::<usize>("table-index")
        .copied()
        .unwrap_or(0);
    let row = matches.get_one::<usize>("row").copied().unwrap();
    let col = matches.get_one::<usize>("col").copied().unwrap();
    let text = matches.get_one::<String>("text").unwrap();

    // 1. Fetch document
    let doc_val = fetch_document(doc_id).await?;

    // 2. Find cell range
    let (start, end) = find_cell_range(&doc_val, table_index, row, col)?;

    // 3. Build requests: delete existing content first (if any), then insert new text
    let mut requests: Vec<Value> = Vec::new();

    if end > start {
        // Delete existing cell content before inserting new text.
        // The delete must come before insert in the requests array so
        // that positional indices remain valid for the insert.
        requests.push(json!({
            "deleteContentRange": {
                "range": {
                    "startIndex": start,
                    "endIndex": end,
                    "segmentId": ""
                }
            }
        }));
    }

    // Insert new text at the cell start position
    requests.push(json!({
        "insertText": {
            "text": text,
            "location": {
                "index": start,
                "segmentId": ""
            }
        }
    }));

    // 4. Requests are ordered correctly: delete (higher range) before insert.
    // Google Docs batchUpdate processes requests sequentially, so delete
    // clears the range, then insert writes at the now-empty start position.

    // 5. POST batchUpdate
    send_batch_update(doc_id, requests).await?;

    // Output result
    let result = json!({
        "written": {
            "table": table_index,
            "row": row,
            "col": col,
            "text": text
        }
    });
    println!("{}", serde_json::to_string_pretty(&result).unwrap());
    Ok(())
}

async fn handle_insert_table_row(matches: &ArgMatches) -> Result<(), GwsError> {
    let doc_id = matches.get_one::<String>("document").unwrap();
    let table_index = matches
        .get_one::<usize>("table-index")
        .copied()
        .unwrap_or(0);

    // 1. Fetch document to find table start index and row count
    let doc_val = fetch_document(doc_id).await?;
    let start_index = find_table_start_index(&doc_val, table_index)?;
    let row_count = get_table_row_count(&doc_val, table_index)?;

    // 2. Determine insert row: if --after-row not specified, use last row index
    let after_row = matches
        .get_one::<usize>("after-row")
        .copied()
        .unwrap_or_else(|| if row_count > 0 { row_count - 1 } else { 0 });

    // Validate row index
    if after_row >= row_count {
        return Err(GwsError::Other(anyhow::anyhow!(
            "Row index {} out of range (table has {} rows, max index is {})",
            after_row,
            row_count,
            row_count.saturating_sub(1)
        )));
    }

    // 3. POST batchUpdate with insertTableRow request
    let requests = vec![json!({
        "insertTableRow": {
            "tableCellLocation": {
                "tableStartLocation": {
                    "index": start_index
                },
                "rowIndex": after_row
            },
            "insertBelow": true
        }
    })];

    send_batch_update(doc_id, requests).await?;

    // Output result
    let result = json!({
        "insertedAfterRow": after_row,
        "rowsBefore": row_count
    });
    println!("{}", serde_json::to_string_pretty(&result).unwrap());
    Ok(())
}

async fn handle_delete_table_row(matches: &ArgMatches) -> Result<(), GwsError> {
    let doc_id = matches.get_one::<String>("document").unwrap();
    let table_index = matches
        .get_one::<usize>("table-index")
        .copied()
        .unwrap_or(0);
    let row = matches.get_one::<usize>("row").copied().unwrap();

    // 1. Fetch document to find table start index and validate row exists
    let doc_val = fetch_document(doc_id).await?;
    let start_index = find_table_start_index(&doc_val, table_index)?;
    let row_count = get_table_row_count(&doc_val, table_index)?;

    // Validate row index
    if row >= row_count {
        return Err(GwsError::Other(anyhow::anyhow!(
            "Row index {} out of range (table has {} rows, max index is {})",
            row,
            row_count,
            row_count.saturating_sub(1)
        )));
    }

    // Cannot delete the only row in a table
    if row_count <= 1 {
        return Err(GwsError::Other(anyhow::anyhow!(
            "Cannot delete the only row in a table"
        )));
    }

    // 2. POST batchUpdate with deleteTableRow request
    let requests = vec![json!({
        "deleteTableRow": {
            "tableCellLocation": {
                "tableStartLocation": {
                    "index": start_index
                },
                "rowIndex": row
            }
        }
    })];

    send_batch_update(doc_id, requests).await?;

    // Output result
    let result = json!({
        "deletedRow": row,
        "rowsAfter": row_count - 1
    });
    println!("{}", serde_json::to_string_pretty(&result).unwrap());
    Ok(())
}

fn build_write_request(
    matches: &ArgMatches,
    doc: &crate::discovery::RestDescription,
) -> Result<(String, String, Vec<String>), GwsError> {
    let document_id = matches.get_one::<String>("document").unwrap();
    let text = matches.get_one::<String>("text").unwrap();

    let documents_res = doc
        .resources
        .get("documents")
        .ok_or_else(|| GwsError::Discovery("Resource 'documents' not found".to_string()))?;
    let batch_update_method = documents_res.methods.get("batchUpdate").ok_or_else(|| {
        GwsError::Discovery("Method 'documents.batchUpdate' not found".to_string())
    })?;

    let params = json!({
        "documentId": document_id
    });

    let body = json!({
        "requests": [
            {
                "insertText": {
                    "text": text,
                    "endOfSegmentLocation": {
                        "segmentId": "" // Empty means body
                    }
                }
            }
        ]
    });

    let scopes: Vec<String> = batch_update_method
        .scopes
        .iter()
        .map(|s| s.to_string())
        .collect();

    Ok((params.to_string(), body.to_string(), scopes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::discovery::{RestDescription, RestMethod, RestResource};
    use std::collections::HashMap;

    fn make_mock_doc() -> RestDescription {
        let mut methods = HashMap::new();
        methods.insert(
            "batchUpdate".to_string(),
            RestMethod {
                scopes: vec!["https://scope".to_string()],
                ..Default::default()
            },
        );

        let mut documents_res = RestResource::default();
        documents_res.methods = methods;

        let mut resources = HashMap::new();
        resources.insert("documents".to_string(), documents_res);

        RestDescription {
            resources,
            ..Default::default()
        }
    }

    fn make_matches_write(args: &[&str]) -> ArgMatches {
        let cmd = Command::new("test")
            .arg(Arg::new("document").long("document"))
            .arg(Arg::new("text").long("text"));
        cmd.try_get_matches_from(args).unwrap()
    }

    #[test]
    fn test_build_write_request() {
        let doc = make_mock_doc();
        let matches = make_matches_write(&["test", "--document", "123", "--text", "hello world"]);
        let (params, body, scopes) = build_write_request(&matches, &doc).unwrap();

        assert!(params.contains("123"));
        assert!(body.contains("hello world"));
        assert!(body.contains("endOfSegmentLocation"));
        assert_eq!(scopes[0], "https://scope");
    }

    #[test]
    fn test_revisions_command_registered() {
        let helper = DocsHelper;
        let base = Command::new("docs");
        let doc = RestDescription::default();
        let cmd = helper.inject_commands(base, &doc);
        let subcommands: Vec<&str> = cmd.get_subcommands().map(|c| c.get_name()).collect();
        assert!(subcommands.contains(&"+revisions"));
        assert!(subcommands.contains(&"+write"));
    }

    #[test]
    fn test_revisions_requires_document() {
        let helper = DocsHelper;
        let base = Command::new("docs");
        let doc = RestDescription::default();
        let cmd = helper.inject_commands(base, &doc);
        let revisions_cmd = cmd
            .get_subcommands()
            .find(|c| c.get_name() == "+revisions")
            .unwrap();
        let doc_arg = revisions_cmd
            .get_arguments()
            .find(|a| a.get_id() == "document")
            .unwrap();
        assert!(doc_arg.is_required_set());
    }

    // --- soul-codes helper tests ---
    // Add these inside the existing #[cfg(test)] mod tests { block:

    #[test]
    fn test_find_replace_command_registered() {
        let helper = DocsHelper;
        let base = Command::new("docs");
        let doc = RestDescription::default();
        let cmd = helper.inject_commands(base, &doc);
        let subcommands: Vec<&str> = cmd.get_subcommands().map(|c| c.get_name()).collect();
        assert!(subcommands.contains(&"+find-replace"));
    }

    #[test]
    fn test_find_replace_requires_document_find_replace() {
        let helper = DocsHelper;
        let base = Command::new("docs");
        let doc = RestDescription::default();
        let cmd = helper.inject_commands(base, &doc);
        let sub = cmd
            .get_subcommands()
            .find(|c| c.get_name() == "+find-replace")
            .unwrap();
        for arg_name in &["document", "find", "replace"] {
            let arg = sub
                .get_arguments()
                .find(|a| a.get_id().as_str() == *arg_name)
                .unwrap_or_else(|| panic!("Missing arg: {}", arg_name));
            assert!(
                arg.is_required_set(),
                "Arg '{}' should be required",
                arg_name
            );
        }
    }

    #[test]
    fn test_insert_link_command_registered() {
        let helper = DocsHelper;
        let base = Command::new("docs");
        let doc = RestDescription::default();
        let cmd = helper.inject_commands(base, &doc);
        let subcommands: Vec<&str> = cmd.get_subcommands().map(|c| c.get_name()).collect();
        assert!(subcommands.contains(&"+insert-link"));
    }

    #[test]
    fn test_insert_link_requires_document_text_url() {
        let helper = DocsHelper;
        let base = Command::new("docs");
        let doc = RestDescription::default();
        let cmd = helper.inject_commands(base, &doc);
        let sub = cmd
            .get_subcommands()
            .find(|c| c.get_name() == "+insert-link")
            .unwrap();
        for arg_name in &["document", "text", "url"] {
            let arg = sub
                .get_arguments()
                .find(|a| a.get_id().as_str() == *arg_name)
                .unwrap_or_else(|| panic!("Missing arg: {}", arg_name));
            assert!(
                arg.is_required_set(),
                "Arg '{}' should be required",
                arg_name
            );
        }
    }

    #[test]
    fn test_set_style_command_registered() {
        let helper = DocsHelper;
        let base = Command::new("docs");
        let doc = RestDescription::default();
        let cmd = helper.inject_commands(base, &doc);
        let subcommands: Vec<&str> = cmd.get_subcommands().map(|c| c.get_name()).collect();
        assert!(subcommands.contains(&"+set-style"));
    }

    #[test]
    fn test_set_style_requires_document_find_style() {
        let helper = DocsHelper;
        let base = Command::new("docs");
        let doc = RestDescription::default();
        let cmd = helper.inject_commands(base, &doc);
        let sub = cmd
            .get_subcommands()
            .find(|c| c.get_name() == "+set-style")
            .unwrap();
        for arg_name in &["document", "find", "style"] {
            let arg = sub
                .get_arguments()
                .find(|a| a.get_id().as_str() == *arg_name)
                .unwrap_or_else(|| panic!("Missing arg: {}", arg_name));
            assert!(
                arg.is_required_set(),
                "Arg '{}' should be required",
                arg_name
            );
        }
    }

    #[test]
    fn test_set_style_validates_style_values() {
        let helper = DocsHelper;
        let base = Command::new("docs");
        let doc = RestDescription::default();
        let cmd = helper.inject_commands(base, &doc);
        let sub = cmd
            .get_subcommands()
            .find(|c| c.get_name() == "+set-style")
            .unwrap()
            .clone();
        // Valid style should parse
        let result = sub.clone().try_get_matches_from([
            "+set-style",
            "--document",
            "doc1",
            "--find",
            "text",
            "--style",
            "HEADING_1",
        ]);
        assert!(result.is_ok());
        // Invalid style should fail
        let result = sub.try_get_matches_from([
            "+set-style",
            "--document",
            "doc1",
            "--find",
            "text",
            "--style",
            "INVALID_STYLE",
        ]);
        assert!(result.is_err());
    }

    #[test]
    fn test_table_read_command_registered() {
        let helper = DocsHelper;
        let base = Command::new("docs");
        let doc = RestDescription::default();
        let cmd = helper.inject_commands(base, &doc);
        let subcommands: Vec<&str> = cmd.get_subcommands().map(|c| c.get_name()).collect();
        assert!(subcommands.contains(&"+table-read"));
    }

    #[test]
    fn test_table_read_requires_document() {
        let helper = DocsHelper;
        let base = Command::new("docs");
        let doc = RestDescription::default();
        let cmd = helper.inject_commands(base, &doc);
        let sub = cmd
            .get_subcommands()
            .find(|c| c.get_name() == "+table-read")
            .unwrap();
        let doc_arg = sub
            .get_arguments()
            .find(|a| a.get_id() == "document")
            .unwrap();
        assert!(doc_arg.is_required_set());
        // table-index should NOT be required
        let idx_arg = sub
            .get_arguments()
            .find(|a| a.get_id() == "table-index")
            .unwrap();
        assert!(!idx_arg.is_required_set());
    }

    #[test]
    fn test_table_list_command_registered() {
        let helper = DocsHelper;
        let base = Command::new("docs");
        let doc = RestDescription::default();
        let cmd = helper.inject_commands(base, &doc);
        let subcommands: Vec<&str> = cmd.get_subcommands().map(|c| c.get_name()).collect();
        assert!(subcommands.contains(&"+table-list"));
    }

    #[test]
    fn test_table_list_requires_document() {
        let helper = DocsHelper;
        let base = Command::new("docs");
        let doc = RestDescription::default();
        let cmd = helper.inject_commands(base, &doc);
        let sub = cmd
            .get_subcommands()
            .find(|c| c.get_name() == "+table-list")
            .unwrap();
        let doc_arg = sub
            .get_arguments()
            .find(|a| a.get_id() == "document")
            .unwrap();
        assert!(doc_arg.is_required_set());
    }

    #[test]
    fn test_table_add_column_command_registered() {
        let helper = DocsHelper;
        let base = Command::new("docs");
        let doc = RestDescription::default();
        let cmd = helper.inject_commands(base, &doc);
        let subcommands: Vec<&str> = cmd.get_subcommands().map(|c| c.get_name()).collect();
        assert!(subcommands.contains(&"+table-add-column"));
    }

    #[test]
    fn test_table_add_column_requires_document_only() {
        let helper = DocsHelper;
        let base = Command::new("docs");
        let doc = RestDescription::default();
        let cmd = helper.inject_commands(base, &doc);
        let sub = cmd
            .get_subcommands()
            .find(|c| c.get_name() == "+table-add-column")
            .unwrap();
        let doc_arg = sub
            .get_arguments()
            .find(|a| a.get_id() == "document")
            .unwrap();
        assert!(doc_arg.is_required_set());
        // table-index and after-col should NOT be required
        let idx_arg = sub
            .get_arguments()
            .find(|a| a.get_id() == "table-index")
            .unwrap();
        assert!(!idx_arg.is_required_set());
        let col_arg = sub
            .get_arguments()
            .find(|a| a.get_id() == "after-col")
            .unwrap();
        assert!(!col_arg.is_required_set());
    }

    #[test]
    fn test_all_new_commands_registered() {
        let helper = DocsHelper;
        let base = Command::new("docs");
        let doc = RestDescription::default();
        let cmd = helper.inject_commands(base, &doc);
        let subcommands: Vec<&str> = cmd.get_subcommands().map(|c| c.get_name()).collect();
        let expected = vec![
            "+write",
            "+revisions",
            "+find-replace",
            "+insert-link",
            "+set-style",
            "+table-read",
            "+table-list",
            "+table-add-column",
        ];
        for name in expected {
            assert!(subcommands.contains(&name), "Missing subcommand: {}", name);
        }
    }

    #[test]
    fn test_extract_text_runs_from_doc() {
        let doc = json!({
            "body": {
                "content": [
                    {
                        "paragraph": {
                            "elements": [
                                {
                                    "textRun": {
                                        "content": "Hello world"
                                    },
                                    "startIndex": 1,
                                    "endIndex": 12
                                }
                            ]
                        }
                    }
                ]
            }
        });
        let runs = extract_text_runs(&doc);
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].0, "Hello world");
        assert_eq!(runs[0].1, 1);
        assert_eq!(runs[0].2, 12);
    }

    #[test]
    fn test_find_paragraphs_containing() {
        let doc = json!({
            "body": {
                "content": [
                    {
                        "startIndex": 0,
                        "endIndex": 12,
                        "paragraph": {
                            "elements": [
                                {
                                    "textRun": { "content": "Hello world\n" },
                                    "startIndex": 0,
                                    "endIndex": 12
                                }
                            ]
                        }
                    },
                    {
                        "startIndex": 12,
                        "endIndex": 22,
                        "paragraph": {
                            "elements": [
                                {
                                    "textRun": { "content": "Goodbye\n" },
                                    "startIndex": 12,
                                    "endIndex": 22
                                }
                            ]
                        }
                    }
                ]
            }
        });
        let results = find_paragraphs_containing(&doc, "Hello");
        assert_eq!(results.len(), 1);
        assert!(results[0].0.contains("Hello"));
        assert_eq!(results[0].1, 0);

        let results = find_paragraphs_containing(&doc, "Goodbye");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].1, 12);

        let results = find_paragraphs_containing(&doc, "not here");
        assert!(results.is_empty());
    }

    #[test]
    fn test_read_table() {
        let table = json!({
            "tableRows": [
                {
                    "tableCells": [
                        {
                            "content": [{
                                "paragraph": {
                                    "elements": [{
                                        "textRun": { "content": "Name\n" }
                                    }]
                                }
                            }]
                        },
                        {
                            "content": [{
                                "paragraph": {
                                    "elements": [{
                                        "textRun": { "content": "Age\n" }
                                    }]
                                }
                            }]
                        }
                    ]
                },
                {
                    "tableCells": [
                        {
                            "content": [{
                                "paragraph": {
                                    "elements": [{
                                        "textRun": { "content": "Alice\n" }
                                    }]
                                }
                            }]
                        },
                        {
                            "content": [{
                                "paragraph": {
                                    "elements": [{
                                        "textRun": { "content": "30\n" }
                                    }]
                                }
                            }]
                        }
                    ]
                }
            ]
        });
        let (rows, row_count, col_count) = read_table(&table);
        assert_eq!(row_count, 2);
        assert_eq!(col_count, 2);
        assert_eq!(rows[0], vec!["Name", "Age"]);
        assert_eq!(rows[1], vec!["Alice", "30"]);
    }

    #[test]
    fn test_find_tables() {
        let doc = json!({
            "body": {
                "content": [
                    {
                        "startIndex": 0,
                        "endIndex": 5,
                        "paragraph": { "elements": [] }
                    },
                    {
                        "startIndex": 5,
                        "endIndex": 50,
                        "table": {
                            "tableRows": [{
                                "tableCells": [{
                                    "content": [{
                                        "paragraph": {
                                            "elements": [{
                                                "textRun": { "content": "A\n" }
                                            }]
                                        }
                                    }]
                                }]
                            }]
                        }
                    },
                    {
                        "startIndex": 50,
                        "endIndex": 100,
                        "table": {
                            "tableRows": [{
                                "tableCells": [{
                                    "content": [{
                                        "paragraph": {
                                            "elements": [{
                                                "textRun": { "content": "B\n" }
                                            }]
                                        }
                                    }]
                                }]
                            }]
                        }
                    }
                ]
            }
        });
        let tables = find_tables(&doc);
        assert_eq!(tables.len(), 2);
        assert_eq!(tables[0].0, 5);
        assert_eq!(tables[1].0, 50);
    }

    #[test]
    fn test_extract_cell_text_trims_newline() {
        let cell = json!({
            "content": [{
                "paragraph": {
                    "elements": [{
                        "textRun": { "content": "Hello\n" }
                    }]
                }
            }]
        });
        assert_eq!(extract_cell_text(&cell), "Hello");
    }

    #[test]
    fn test_extract_cell_text_multi_paragraph() {
        let cell = json!({
            "content": [
                {
                    "paragraph": {
                        "elements": [{
                            "textRun": { "content": "Line 1\n" }
                        }]
                    }
                },
                {
                    "paragraph": {
                        "elements": [{
                            "textRun": { "content": "Line 2\n" }
                        }]
                    }
                }
            ]
        });
        assert_eq!(extract_cell_text(&cell), "Line 1\nLine 2");
    }

    // Add these inside the existing #[cfg(test)] mod tests { ... }

    #[test]
    fn test_table_write_command_registered() {
        let helper = DocsHelper;
        let base = Command::new("docs");
        let doc = RestDescription::default();
        let cmd = helper.inject_commands(base, &doc);
        let subcommands: Vec<&str> = cmd.get_subcommands().map(|c| c.get_name()).collect();
        assert!(subcommands.contains(&"+table-write"));
    }

    #[test]
    fn test_insert_table_row_command_registered() {
        let helper = DocsHelper;
        let base = Command::new("docs");
        let doc = RestDescription::default();
        let cmd = helper.inject_commands(base, &doc);
        let subcommands: Vec<&str> = cmd.get_subcommands().map(|c| c.get_name()).collect();
        assert!(subcommands.contains(&"+insert-table-row"));
    }

    #[test]
    fn test_delete_table_row_command_registered() {
        let helper = DocsHelper;
        let base = Command::new("docs");
        let doc = RestDescription::default();
        let cmd = helper.inject_commands(base, &doc);
        let subcommands: Vec<&str> = cmd.get_subcommands().map(|c| c.get_name()).collect();
        assert!(subcommands.contains(&"+delete-table-row"));
    }

    #[test]
    fn test_table_write_requires_document_row_col_text() {
        let helper = DocsHelper;
        let base = Command::new("docs");
        let doc = RestDescription::default();
        let cmd = helper.inject_commands(base, &doc);
        let tw_cmd = cmd
            .get_subcommands()
            .find(|c| c.get_name() == "+table-write")
            .unwrap();
        for arg_name in &["document", "row", "col", "text"] {
            let arg = tw_cmd
                .get_arguments()
                .find(|a| a.get_id().as_str() == *arg_name)
                .unwrap_or_else(|| panic!("Arg {} not found", arg_name));
            assert!(arg.is_required_set(), "Arg {} should be required", arg_name);
        }
    }

    #[test]
    fn test_table_write_table_index_optional_defaults_zero() {
        let helper = DocsHelper;
        let base = Command::new("docs");
        let doc = RestDescription::default();
        let cmd = helper.inject_commands(base, &doc);
        let tw_cmd = cmd
            .get_subcommands()
            .find(|c| c.get_name() == "+table-write")
            .unwrap();
        let ti_arg = tw_cmd
            .get_arguments()
            .find(|a| a.get_id().as_str() == "table-index")
            .unwrap();
        assert!(!ti_arg.is_required_set());
        // Default value should be "0"
        let defaults: Vec<&str> = ti_arg
            .get_default_values()
            .iter()
            .map(|v| v.to_str().unwrap())
            .collect();
        assert_eq!(defaults, vec!["0"]);
    }

    #[test]
    fn test_insert_table_row_after_row_optional() {
        let helper = DocsHelper;
        let base = Command::new("docs");
        let doc = RestDescription::default();
        let cmd = helper.inject_commands(base, &doc);
        let itr_cmd = cmd
            .get_subcommands()
            .find(|c| c.get_name() == "+insert-table-row")
            .unwrap();
        let ar_arg = itr_cmd
            .get_arguments()
            .find(|a| a.get_id().as_str() == "after-row")
            .unwrap();
        assert!(!ar_arg.is_required_set());
    }

    #[test]
    fn test_delete_table_row_requires_document_and_row() {
        let helper = DocsHelper;
        let base = Command::new("docs");
        let doc = RestDescription::default();
        let cmd = helper.inject_commands(base, &doc);
        let dtr_cmd = cmd
            .get_subcommands()
            .find(|c| c.get_name() == "+delete-table-row")
            .unwrap();
        for arg_name in &["document", "row"] {
            let arg = dtr_cmd
                .get_arguments()
                .find(|a| a.get_id().as_str() == *arg_name)
                .unwrap_or_else(|| panic!("Arg {} not found", arg_name));
            assert!(arg.is_required_set(), "Arg {} should be required", arg_name);
        }
    }

    #[test]
    fn test_find_table_start_index_basic() {
        let doc = json!({
            "body": {
                "content": [
                    {"paragraph": {}, "startIndex": 1, "endIndex": 10},
                    {"table": {"tableRows": []}, "startIndex": 11, "endIndex": 50},
                    {"paragraph": {}, "startIndex": 51, "endIndex": 60},
                    {"table": {"tableRows": []}, "startIndex": 61, "endIndex": 100}
                ]
            }
        });
        assert_eq!(find_table_start_index(&doc, 0).unwrap(), 11);
        assert_eq!(find_table_start_index(&doc, 1).unwrap(), 61);
    }

    #[test]
    fn test_find_table_start_index_not_found() {
        let doc = json!({
            "body": {
                "content": [
                    {"paragraph": {}, "startIndex": 1, "endIndex": 10}
                ]
            }
        });
        let err = find_table_start_index(&doc, 0).unwrap_err();
        assert!(err.to_string().contains("Table index 0 not found"));
    }

    #[test]
    fn test_find_cell_range_basic() {
        let doc = json!({
            "body": {
                "content": [
                    {
                        "table": {
                            "tableRows": [
                                {
                                    "tableCells": [
                                        {
                                            "content": [
                                                {"startIndex": 5, "endIndex": 15}
                                            ]
                                        },
                                        {
                                            "content": [
                                                {"startIndex": 16, "endIndex": 25}
                                            ]
                                        }
                                    ]
                                },
                                {
                                    "tableCells": [
                                        {
                                            "content": [
                                                {"startIndex": 30, "endIndex": 40},
                                                {"startIndex": 40, "endIndex": 50}
                                            ]
                                        }
                                    ]
                                }
                            ]
                        },
                        "startIndex": 1,
                        "endIndex": 100
                    }
                ]
            }
        });

        // Table 0, row 0, col 0: start=5, end=15-1=14
        let (s, e) = find_cell_range(&doc, 0, 0, 0).unwrap();
        assert_eq!(s, 5);
        assert_eq!(e, 14);

        // Table 0, row 0, col 1: start=16, end=25-1=24
        let (s, e) = find_cell_range(&doc, 0, 0, 1).unwrap();
        assert_eq!(s, 16);
        assert_eq!(e, 24);

        // Table 0, row 1, col 0: multiple paragraphs, start=30, end=50-1=49
        let (s, e) = find_cell_range(&doc, 0, 1, 0).unwrap();
        assert_eq!(s, 30);
        assert_eq!(e, 49);
    }

    #[test]
    fn test_find_cell_range_row_out_of_bounds() {
        let doc = json!({
            "body": {
                "content": [
                    {
                        "table": {
                            "tableRows": [
                                {
                                    "tableCells": [
                                        {"content": [{"startIndex": 5, "endIndex": 15}]}
                                    ]
                                }
                            ]
                        },
                        "startIndex": 1,
                        "endIndex": 50
                    }
                ]
            }
        });
        let err = find_cell_range(&doc, 0, 5, 0).unwrap_err();
        assert!(err.to_string().contains("Row 5 not found"));
    }

    #[test]
    fn test_find_cell_range_col_out_of_bounds() {
        let doc = json!({
            "body": {
                "content": [
                    {
                        "table": {
                            "tableRows": [
                                {
                                    "tableCells": [
                                        {"content": [{"startIndex": 5, "endIndex": 15}]}
                                    ]
                                }
                            ]
                        },
                        "startIndex": 1,
                        "endIndex": 50
                    }
                ]
            }
        });
        let err = find_cell_range(&doc, 0, 0, 3).unwrap_err();
        assert!(err.to_string().contains("Col 3 not found"));
    }

    #[test]
    fn test_get_table_row_count() {
        let doc = json!({
            "body": {
                "content": [
                    {
                        "table": {
                            "tableRows": [
                                {"tableCells": []},
                                {"tableCells": []},
                                {"tableCells": []}
                            ]
                        },
                        "startIndex": 1,
                        "endIndex": 100
                    }
                ]
            }
        });
        assert_eq!(get_table_row_count(&doc, 0).unwrap(), 3);
    }

    #[test]
    fn test_get_table_row_count_table_not_found() {
        let doc = json!({
            "body": {
                "content": [
                    {"paragraph": {}, "startIndex": 1, "endIndex": 10}
                ]
            }
        });
        let err = get_table_row_count(&doc, 0).unwrap_err();
        assert!(err.to_string().contains("Table index 0 not found"));
    }
}
