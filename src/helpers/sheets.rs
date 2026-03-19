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
use serde_json::json;
use std::future::Future;
use std::pin::Pin;

pub struct SheetsHelper;

impl Helper for SheetsHelper {
    fn inject_commands(
        &self,
        mut cmd: Command,
        _doc: &crate::discovery::RestDescription,
    ) -> Command {
        cmd = cmd.subcommand(
            Command::new("+append")
                .about("[Helper] Append a row to a spreadsheet")
                .arg(
                    Arg::new("spreadsheet")
                        .long("spreadsheet")
                        .help("Spreadsheet ID")
                        .required(true)
                        .value_name("ID"),
                )
                .arg(
                    Arg::new("values")
                        .long("values")
                        .help("Comma-separated values (simple strings)")
                        .value_name("VALUES"),
                )
                .arg(
                    Arg::new("json-values")
                        .long("json-values")
                        .help("JSON array of rows, e.g. '[[\"a\",\"b\"],[\"c\",\"d\"]]'")
                        .value_name("JSON"),
                )
                .after_help(
                    r#"EXAMPLES:
  gws sheets +append --spreadsheet ID --values 'Alice,100,true'
  gws sheets +append --spreadsheet ID --json-values '[["a","b"],["c","d"]]'

TIPS:
  Use --values for simple single-row appends.
  Use --json-values for bulk multi-row inserts."#,
                ),
        );

        cmd = cmd.subcommand(
            Command::new("+read")
                .about("[Helper] Read values from a spreadsheet")
                .arg(
                    Arg::new("spreadsheet")
                        .long("spreadsheet")
                        .help("Spreadsheet ID")
                        .required(true)
                        .value_name("ID"),
                )
                .arg(
                    Arg::new("range")
                        .long("range")
                        .help("Range to read (e.g. 'Sheet1!A1:B2')")
                        .required(true)
                        .value_name("RANGE"),
                )
                .after_help(
                    "\
EXAMPLES:
  gws sheets +read --spreadsheet ID --range \"Sheet1!A1:D10\"
  gws sheets +read --spreadsheet ID --range Sheet1

TIPS:
  Read-only — never modifies the spreadsheet.
  For advanced options, use the raw values.get API.",
                ),
        );

        // --- soul-codes helpers ---
        // Insert these `cmd = cmd.subcommand(...)` blocks inside `inject_commands()`,
        // after the existing `+read` subcommand block and before the final `cmd` return.

        cmd = cmd.subcommand(
            Command::new("+add-tab")
                .about("[Helper] Add a new tab/sheet to a spreadsheet")
                .arg(
                    Arg::new("spreadsheet")
                        .long("spreadsheet")
                        .help("Spreadsheet ID")
                        .required(true)
                        .value_name("ID"),
                )
                .arg(
                    Arg::new("tab-name")
                        .long("tab-name")
                        .help("Name of the new tab")
                        .required(true)
                        .value_name("NAME"),
                )
                .after_help(
                    "EXAMPLES:\n  gws sheets +add-tab --spreadsheet ID --tab-name 'Q1 Data'",
                ),
        );

        cmd = cmd.subcommand(
            Command::new("+freeze")
                .about("[Helper] Freeze rows and/or columns in a tab")
                .arg(
                    Arg::new("spreadsheet")
                        .long("spreadsheet")
                        .help("Spreadsheet ID")
                        .required(true)
                        .value_name("ID"),
                )
                .arg(
                    Arg::new("tab")
                        .long("tab")
                        .help("Tab/sheet name")
                        .required(true)
                        .value_name("NAME"),
                )
                .arg(
                    Arg::new("rows")
                        .long("rows")
                        .help("Number of rows to freeze")
                        .value_name("N")
                        .value_parser(clap::value_parser!(usize)),
                )
                .arg(
                    Arg::new("cols")
                        .long("cols")
                        .help("Number of columns to freeze")
                        .value_name("N")
                        .value_parser(clap::value_parser!(usize)),
                )
                .after_help(
                    "EXAMPLES:\n  gws sheets +freeze --spreadsheet ID --tab Sheet1 --rows 1\n  gws sheets +freeze --spreadsheet ID --tab Sheet1 --rows 2 --cols 1",
                ),
        );

        cmd = cmd.subcommand(
            Command::new("+column-width")
                .about("[Helper] Set the pixel width of a column")
                .arg(
                    Arg::new("spreadsheet")
                        .long("spreadsheet")
                        .help("Spreadsheet ID")
                        .required(true)
                        .value_name("ID"),
                )
                .arg(
                    Arg::new("tab")
                        .long("tab")
                        .help("Tab/sheet name")
                        .required(true)
                        .value_name("NAME"),
                )
                .arg(
                    Arg::new("col")
                        .long("col")
                        .help("Column index (0-based)")
                        .required(true)
                        .value_name("N")
                        .value_parser(clap::value_parser!(usize)),
                )
                .arg(
                    Arg::new("pixels")
                        .long("pixels")
                        .help("Width in pixels")
                        .required(true)
                        .value_name("N")
                        .value_parser(clap::value_parser!(usize)),
                )
                .after_help(
                    "EXAMPLES:\n  gws sheets +column-width --spreadsheet ID --tab Sheet1 --col 0 --pixels 200",
                ),
        );

        cmd = cmd.subcommand(
            Command::new("+format-header")
                .about("[Helper] Format a header row with colors, bold, centered text")
                .arg(
                    Arg::new("spreadsheet")
                        .long("spreadsheet")
                        .help("Spreadsheet ID")
                        .required(true)
                        .value_name("ID"),
                )
                .arg(
                    Arg::new("tab")
                        .long("tab")
                        .help("Tab/sheet name")
                        .required(true)
                        .value_name("NAME"),
                )
                .arg(
                    Arg::new("range")
                        .long("range")
                        .help("A1 range, e.g. 'A1:F1'")
                        .required(true)
                        .value_name("RANGE"),
                )
                .arg(
                    Arg::new("bg-hex")
                        .long("bg-hex")
                        .help("Background color hex (default #445269)")
                        .value_name("COLOR")
                        .default_value("#445269"),
                )
                .arg(
                    Arg::new("text-hex")
                        .long("text-hex")
                        .help("Text color hex (default #FFFFFF)")
                        .value_name("COLOR")
                        .default_value("#FFFFFF"),
                )
                .after_help(
                    "EXAMPLES:\n  gws sheets +format-header --spreadsheet ID --tab Sheet1 --range A1:F1\n  gws sheets +format-header --spreadsheet ID --tab Sheet1 --range A1:F1 --bg-hex '#1a2b3c' --text-hex '#FFFF00'",
                ),
        );

        cmd = cmd.subcommand(
            Command::new("+conditional-format")
                .about("[Helper] Add a conditional formatting rule (text equals)")
                .arg(
                    Arg::new("spreadsheet")
                        .long("spreadsheet")
                        .help("Spreadsheet ID")
                        .required(true)
                        .value_name("ID"),
                )
                .arg(
                    Arg::new("tab")
                        .long("tab")
                        .help("Tab/sheet name")
                        .required(true)
                        .value_name("NAME"),
                )
                .arg(
                    Arg::new("range")
                        .long("range")
                        .help("A1 range, e.g. 'B2:B100'")
                        .required(true)
                        .value_name("RANGE"),
                )
                .arg(
                    Arg::new("match-text")
                        .long("match-text")
                        .help("Text value to match")
                        .required(true)
                        .value_name("TEXT"),
                )
                .arg(
                    Arg::new("bg-hex")
                        .long("bg-hex")
                        .help("Background color hex for matching cells")
                        .required(true)
                        .value_name("COLOR"),
                )
                .after_help(
                    "EXAMPLES:\n  gws sheets +conditional-format --spreadsheet ID --tab Sheet1 --range B2:B100 --match-text Done --bg-hex '#00FF00'",
                ),
        );

        cmd = cmd.subcommand(
            Command::new("+read-notes")
                .about("[Helper] Read cell notes from a range")
                .arg(
                    Arg::new("spreadsheet")
                        .long("spreadsheet")
                        .help("Spreadsheet ID")
                        .required(true)
                        .value_name("ID"),
                )
                .arg(
                    Arg::new("range")
                        .long("range")
                        .help("Range with sheet prefix, e.g. 'Sheet1!A1:C3'")
                        .required(true)
                        .value_name("RANGE"),
                )
                .after_help(
                    "EXAMPLES:\n  gws sheets +read-notes --spreadsheet ID --range 'Sheet1!A1:C10'",
                ),
        );

        cmd = cmd.subcommand(
            Command::new("+write-note")
                .about("[Helper] Write a note to a single cell")
                .arg(
                    Arg::new("spreadsheet")
                        .long("spreadsheet")
                        .help("Spreadsheet ID")
                        .required(true)
                        .value_name("ID"),
                )
                .arg(
                    Arg::new("tab")
                        .long("tab")
                        .help("Tab/sheet name")
                        .required(true)
                        .value_name("NAME"),
                )
                .arg(
                    Arg::new("cell")
                        .long("cell")
                        .help("Cell reference, e.g. 'B5'")
                        .required(true)
                        .value_name("CELL"),
                )
                .arg(
                    Arg::new("note")
                        .long("note")
                        .help("Note text to write")
                        .required(true)
                        .value_name("TEXT"),
                )
                .after_help(
                    "EXAMPLES:\n  gws sheets +write-note --spreadsheet ID --tab Sheet1 --cell B5 --note 'Review needed'",
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
            if let Some(matches) = matches.subcommand_matches("+append") {
                let config = parse_append_args(matches);
                let (params_str, body_str, scopes) = build_append_request(&config, doc)?;

                let scope_strs: Vec<&str> = scopes.iter().map(|s| s.as_str()).collect();
                let (token, auth_method) = match auth::get_token(&scope_strs).await {
                    Ok(t) => (Some(t), executor::AuthMethod::OAuth),
                    Err(_) if matches.get_flag("dry-run") => (None, executor::AuthMethod::None),
                    Err(e) => return Err(GwsError::Auth(format!("Sheets auth failed: {e}"))),
                };

                let spreadsheets_res = doc.resources.get("spreadsheets").ok_or_else(|| {
                    GwsError::Discovery("Resource 'spreadsheets' not found".to_string())
                })?;
                let values_res = spreadsheets_res.resources.get("values").ok_or_else(|| {
                    GwsError::Discovery("Resource 'spreadsheets.values' not found".to_string())
                })?;
                let append_method = values_res.methods.get("append").ok_or_else(|| {
                    GwsError::Discovery("Method 'spreadsheets.values.append' not found".to_string())
                })?;

                let pagination = executor::PaginationConfig {
                    page_all: false,
                    page_limit: 10,
                    page_delay_ms: 100,
                };

                executor::execute_method(
                    doc,
                    append_method,
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

            if let Some(matches) = matches.subcommand_matches("+read") {
                let config = parse_read_args(matches);
                let (params_str, scopes) = build_read_request(&config, doc)?;

                // Re-find method
                let spreadsheets_res = doc.resources.get("spreadsheets").ok_or_else(|| {
                    GwsError::Discovery("Resource 'spreadsheets' not found".to_string())
                })?;
                let values_res = spreadsheets_res.resources.get("values").ok_or_else(|| {
                    GwsError::Discovery("Resource 'spreadsheets.values' not found".to_string())
                })?;
                let get_method = values_res.methods.get("get").ok_or_else(|| {
                    GwsError::Discovery("Method 'spreadsheets.values.get' not found".to_string())
                })?;

                let scope_strs: Vec<&str> = scopes.iter().map(|s| s.as_str()).collect();
                let (token, auth_method) = match auth::get_token(&scope_strs).await {
                    Ok(t) => (Some(t), executor::AuthMethod::OAuth),
                    Err(_) if matches.get_flag("dry-run") => (None, executor::AuthMethod::None),
                    Err(e) => return Err(GwsError::Auth(format!("Sheets auth failed: {e}"))),
                };

                executor::execute_method(
                    doc,
                    get_method,
                    Some(&params_str),
                    None,
                    token.as_deref(),
                    auth_method,
                    None,
                    None,
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

            // Insert these blocks inside the `handle()` async block, after the existing
            // `+read` dispatch and before the final `Ok(false)`.

            if let Some(sub) = matches.subcommand_matches("+add-tab") {
                handle_add_tab(sub).await?;
                return Ok(true);
            }

            if let Some(sub) = matches.subcommand_matches("+freeze") {
                handle_freeze(sub).await?;
                return Ok(true);
            }

            if let Some(sub) = matches.subcommand_matches("+column-width") {
                handle_column_width(sub).await?;
                return Ok(true);
            }

            if let Some(sub) = matches.subcommand_matches("+format-header") {
                handle_format_header(sub).await?;
                return Ok(true);
            }

            if let Some(sub) = matches.subcommand_matches("+conditional-format") {
                handle_conditional_format(sub).await?;
                return Ok(true);
            }

            if let Some(sub) = matches.subcommand_matches("+read-notes") {
                handle_read_notes(sub).await?;
                return Ok(true);
            }

            if let Some(sub) = matches.subcommand_matches("+write-note") {
                handle_write_note(sub).await?;
                return Ok(true);
            }

            Ok(false)
        })
    }
}

fn build_append_request(
    config: &AppendConfig,
    doc: &crate::discovery::RestDescription,
) -> Result<(String, String, Vec<String>), GwsError> {
    let spreadsheets_res = doc
        .resources
        .get("spreadsheets")
        .ok_or_else(|| GwsError::Discovery("Resource 'spreadsheets' not found".to_string()))?;
    let values_res = spreadsheets_res.resources.get("values").ok_or_else(|| {
        GwsError::Discovery("Resource 'spreadsheets.values' not found".to_string())
    })?;
    let append_method = values_res.methods.get("append").ok_or_else(|| {
        GwsError::Discovery("Method 'spreadsheets.values.append' not found".to_string())
    })?;

    let range = "A1";

    let params = json!({
        "spreadsheetId": config.spreadsheet_id,
        "range": range,
        "valueInputOption": "USER_ENTERED"
    });

    let body = json!({
        "values": config.values
    });

    // Map `&String` scope URLs to owned `String`s for the return value
    let scopes: Vec<String> = append_method.scopes.iter().map(|s| s.to_string()).collect();

    Ok((params.to_string(), body.to_string(), scopes))
}

fn build_read_request(
    config: &ReadConfig,
    doc: &crate::discovery::RestDescription,
) -> Result<(String, Vec<String>), GwsError> {
    // ... resource lookup omitted for brevity ...
    let spreadsheets_res = doc
        .resources
        .get("spreadsheets")
        .ok_or_else(|| GwsError::Discovery("Resource 'spreadsheets' not found".to_string()))?;
    let values_res = spreadsheets_res.resources.get("values").ok_or_else(|| {
        GwsError::Discovery("Resource 'spreadsheets.values' not found".to_string())
    })?;
    let get_method = values_res.methods.get("get").ok_or_else(|| {
        GwsError::Discovery("Method 'spreadsheets.values.get' not found".to_string())
    })?;

    let params = json!({
        "spreadsheetId": config.spreadsheet_id,
        "range": config.range
    });

    let scopes: Vec<String> = get_method.scopes.iter().map(|s| s.to_string()).collect();

    Ok((params.to_string(), scopes))
}

/// Configuration for appending values to a spreadsheet.
///
/// Holds the parsed arguments for the `+append` subcommand.
pub struct AppendConfig {
    /// The ID of the spreadsheet to append to.
    pub spreadsheet_id: String,
    /// The rows to append, where each inner Vec represents one row.
    pub values: Vec<Vec<String>>,
}

/// Parses arguments for the `+append` command.
///
/// Supports both `--values` (single row) and `--json-values` (single or multi-row).
pub fn parse_append_args(matches: &ArgMatches) -> AppendConfig {
    let values = if let Some(json_str) = matches.get_one::<String>("json-values") {
        // Try parsing as array-of-arrays (multi-row) first
        if let Ok(parsed) = serde_json::from_str::<Vec<Vec<String>>>(json_str) {
            parsed
        } else if let Ok(parsed) = serde_json::from_str::<Vec<String>>(json_str) {
            // Single flat array — treat as one row
            vec![parsed]
        } else {
            eprintln!(
                "Warning: --json-values is not valid JSON; expected an array or array-of-arrays"
            );
            Vec::new()
        }
    } else if let Some(values_str) = matches.get_one::<String>("values") {
        vec![values_str.split(',').map(|s| s.to_string()).collect()]
    } else {
        Vec::new()
    };

    AppendConfig {
        spreadsheet_id: matches.get_one::<String>("spreadsheet").unwrap().clone(),
        values,
    }
}

/// Configuration for reading values from a spreadsheet.
pub struct ReadConfig {
    pub spreadsheet_id: String,
    /// A1 notation range (e.g. "Sheet1!A1:B2").
    pub range: String,
}

pub fn parse_read_args(matches: &ArgMatches) -> ReadConfig {
    ReadConfig {
        spreadsheet_id: matches.get_one::<String>("spreadsheet").unwrap().clone(),
        range: matches.get_one::<String>("range").unwrap().clone(),
    }
}

// Place these after the existing `parse_read_args` function and before `#[cfg(test)]`.

// ---------------------------------------------------------------------------
// A1 notation helpers
// ---------------------------------------------------------------------------

/// Convert column letters to a 0-based index: A→0, B→1, Z→25, AA→26
fn col_letters_to_index(letters: &str) -> usize {
    letters.chars().fold(0usize, |acc, c| {
        acc * 26 + (c.to_ascii_uppercase() as usize - 'A' as usize + 1)
    }) - 1
}

/// Convert a 0-based column index back to letters: 0→A, 1→B, 25→Z, 26→AA
fn col_index_to_letters(mut idx: usize) -> String {
    let mut letters = Vec::new();
    loop {
        letters.push((b'A' + (idx % 26) as u8) as char);
        if idx < 26 {
            break;
        }
        idx = idx / 26 - 1;
    }
    letters.reverse();
    letters.into_iter().collect()
}

/// Parse a cell reference like "B5" → (col=1, row=4) both 0-based.
fn parse_cell_ref(cell: &str) -> Result<(usize, usize), GwsError> {
    let split_pos = cell
        .chars()
        .position(|c| c.is_ascii_digit())
        .ok_or_else(|| GwsError::Other(anyhow::anyhow!("Invalid cell reference: {}", cell)))?;
    let col_str = &cell[..split_pos];
    let row_str = &cell[split_pos..];
    if col_str.is_empty() {
        return Err(GwsError::Other(anyhow::anyhow!(
            "Invalid cell reference (no column): {}",
            cell
        )));
    }
    let col = col_letters_to_index(col_str);
    let row: usize = row_str
        .parse::<usize>()
        .map_err(|_| GwsError::Other(anyhow::anyhow!("Invalid row in cell: {}", cell)))?
        - 1;
    Ok((col, row))
}

/// Parse an A1:C3 range into (start_col, start_row, end_col, end_row) with
/// end indices exclusive (suitable for Google Sheets GridRange).
/// Handles optional "Sheet1!" prefix by stripping it.
fn parse_a1_range(range: &str) -> Result<(usize, usize, usize, usize), GwsError> {
    let range = range.split('!').next_back().unwrap_or(range);
    let parts: Vec<&str> = range.split(':').collect();
    let (sc, sr) = parse_cell_ref(parts[0])?;
    if parts.len() == 1 {
        return Ok((sc, sr, sc + 1, sr + 1));
    }
    let (ec, er) = parse_cell_ref(parts[1])?;
    Ok((sc, sr, ec + 1, er + 1)) // end indices are exclusive
}

/// Parse a hex color string like "#445269" into a Sheets-API-compatible
/// JSON color object with red/green/blue as floats in 0.0..1.0.
fn parse_hex_color(hex: &str) -> Result<serde_json::Value, GwsError> {
    let hex = hex.trim_start_matches('#');
    if hex.len() != 6 {
        return Err(GwsError::Other(anyhow::anyhow!(
            "Invalid hex color (expected 6 hex digits): #{}",
            hex
        )));
    }
    let r = u8::from_str_radix(&hex[0..2], 16)
        .map_err(|_| GwsError::Other(anyhow::anyhow!("Invalid hex red channel")))?;
    let g = u8::from_str_radix(&hex[2..4], 16)
        .map_err(|_| GwsError::Other(anyhow::anyhow!("Invalid hex green channel")))?;
    let b = u8::from_str_radix(&hex[4..6], 16)
        .map_err(|_| GwsError::Other(anyhow::anyhow!("Invalid hex blue channel")))?;
    Ok(json!({
        "red":   (r as f64) / 255.0,
        "green": (g as f64) / 255.0,
        "blue":  (b as f64) / 255.0,
    }))
}

// ---------------------------------------------------------------------------
// Shared: get sheet ID by tab name
// ---------------------------------------------------------------------------

async fn get_sheet_id(
    client: &reqwest::Client,
    token: &str,
    spreadsheet_id: &str,
    tab_name: &str,
) -> Result<i64, GwsError> {
    let resp = client
        .get(format!(
            "https://sheets.googleapis.com/v4/spreadsheets/{}",
            spreadsheet_id
        ))
        .query(&[("fields", "sheets.properties")])
        .bearer_auth(token)
        .send()
        .await
        .map_err(|e| GwsError::Other(anyhow::anyhow!("HTTP error: {e}")))?;
    let val: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| GwsError::Other(anyhow::anyhow!("JSON parse: {e}")))?;
    let sheets = val["sheets"]
        .as_array()
        .ok_or_else(|| GwsError::Other(anyhow::anyhow!("No sheets found")))?;
    for sheet in sheets {
        if sheet["properties"]["title"].as_str() == Some(tab_name) {
            return Ok(sheet["properties"]["sheetId"].as_i64().unwrap_or(0));
        }
    }
    Err(GwsError::Other(anyhow::anyhow!(
        "Tab '{}' not found",
        tab_name
    )))
}

// ---------------------------------------------------------------------------
// Shared: auth + client setup
// ---------------------------------------------------------------------------

async fn sheets_auth_and_client() -> Result<(reqwest::Client, String), GwsError> {
    let token = auth::get_token(&["https://www.googleapis.com/auth/spreadsheets"])
        .await
        .map_err(|e| GwsError::Auth(format!("Sheets auth failed: {e}")))?;
    let client = crate::client::build_client()?;
    Ok((client, token))
}

// ---------------------------------------------------------------------------
// Shared: POST batchUpdate with error handling
// ---------------------------------------------------------------------------

async fn batch_update(
    client: &reqwest::Client,
    token: &str,
    spreadsheet_id: &str,
    requests: serde_json::Value,
) -> Result<serde_json::Value, GwsError> {
    let resp = client
        .post(format!(
            "https://sheets.googleapis.com/v4/spreadsheets/{}:batchUpdate",
            spreadsheet_id
        ))
        .bearer_auth(token)
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
    let val: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| GwsError::Other(anyhow::anyhow!("JSON parse: {e}")))?;
    Ok(val)
}

// ---------------------------------------------------------------------------
// 1. +add-tab
// ---------------------------------------------------------------------------

async fn handle_add_tab(matches: &ArgMatches) -> Result<(), GwsError> {
    let spreadsheet_id = matches.get_one::<String>("spreadsheet").unwrap();
    let tab_name = matches.get_one::<String>("tab-name").unwrap();

    let (client, token) = sheets_auth_and_client().await?;

    let requests = json!([{
        "addSheet": {
            "properties": {
                "title": tab_name
            }
        }
    }]);

    let resp_val = batch_update(&client, &token, spreadsheet_id, requests).await?;

    // Extract sheetId from response
    let sheet_id = resp_val["replies"][0]["addSheet"]["properties"]["sheetId"]
        .as_i64()
        .unwrap_or(0);

    println!("{}", json!({ "tabName": tab_name, "sheetId": sheet_id }));
    Ok(())
}

// ---------------------------------------------------------------------------
// 2. +freeze
// ---------------------------------------------------------------------------

async fn handle_freeze(matches: &ArgMatches) -> Result<(), GwsError> {
    let spreadsheet_id = matches.get_one::<String>("spreadsheet").unwrap();
    let tab_name = matches.get_one::<String>("tab").unwrap();
    let frozen_rows = matches.get_one::<usize>("rows").copied().unwrap_or(0);
    let frozen_cols = matches.get_one::<usize>("cols").copied().unwrap_or(0);

    if frozen_rows == 0 && frozen_cols == 0 {
        return Err(GwsError::Validation(
            "At least one of --rows or --cols must be specified".to_string(),
        ));
    }

    let (client, token) = sheets_auth_and_client().await?;
    let sheet_id = get_sheet_id(&client, &token, spreadsheet_id, tab_name).await?;

    let requests = json!([{
        "updateSheetProperties": {
            "properties": {
                "sheetId": sheet_id,
                "gridProperties": {
                    "frozenRowCount": frozen_rows,
                    "frozenColumnCount": frozen_cols
                }
            },
            "fields": "gridProperties.frozenRowCount,gridProperties.frozenColumnCount"
        }
    }]);

    batch_update(&client, &token, spreadsheet_id, requests).await?;

    println!(
        "{}",
        json!({ "frozen": { "rows": frozen_rows, "cols": frozen_cols } })
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// 3. +column-width
// ---------------------------------------------------------------------------

async fn handle_column_width(matches: &ArgMatches) -> Result<(), GwsError> {
    let spreadsheet_id = matches.get_one::<String>("spreadsheet").unwrap();
    let tab_name = matches.get_one::<String>("tab").unwrap();
    let col = *matches.get_one::<usize>("col").unwrap();
    let pixels = *matches.get_one::<usize>("pixels").unwrap();

    let (client, token) = sheets_auth_and_client().await?;
    let sheet_id = get_sheet_id(&client, &token, spreadsheet_id, tab_name).await?;

    let requests = json!([{
        "updateDimensionProperties": {
            "range": {
                "sheetId": sheet_id,
                "dimension": "COLUMNS",
                "startIndex": col,
                "endIndex": col + 1
            },
            "properties": {
                "pixelSize": pixels
            },
            "fields": "pixelSize"
        }
    }]);

    batch_update(&client, &token, spreadsheet_id, requests).await?;

    println!("{}", json!({ "col": col, "pixels": pixels }));
    Ok(())
}

// ---------------------------------------------------------------------------
// 4. +format-header
// ---------------------------------------------------------------------------

async fn handle_format_header(matches: &ArgMatches) -> Result<(), GwsError> {
    let spreadsheet_id = matches.get_one::<String>("spreadsheet").unwrap();
    let tab_name = matches.get_one::<String>("tab").unwrap();
    let range_str = matches.get_one::<String>("range").unwrap();
    let bg_hex = matches.get_one::<String>("bg-hex").unwrap();
    let text_hex = matches.get_one::<String>("text-hex").unwrap();

    let (sc, sr, ec, er) = parse_a1_range(range_str)?;
    let bg_color = parse_hex_color(bg_hex)?;
    let text_color = parse_hex_color(text_hex)?;

    let (client, token) = sheets_auth_and_client().await?;
    let sheet_id = get_sheet_id(&client, &token, spreadsheet_id, tab_name).await?;

    let requests = json!([{
        "repeatCell": {
            "range": {
                "sheetId": sheet_id,
                "startRowIndex": sr,
                "endRowIndex": er,
                "startColumnIndex": sc,
                "endColumnIndex": ec
            },
            "cell": {
                "userEnteredFormat": {
                    "backgroundColor": bg_color,
                    "textFormat": {
                        "bold": true,
                        "foregroundColor": text_color
                    },
                    "horizontalAlignment": "CENTER",
                    "wrapStrategy": "WRAP"
                }
            },
            "fields": "userEnteredFormat(backgroundColor,textFormat,horizontalAlignment,wrapStrategy)"
        }
    }]);

    batch_update(&client, &token, spreadsheet_id, requests).await?;

    println!("{}", json!({ "formatted": range_str }));
    Ok(())
}

// ---------------------------------------------------------------------------
// 5. +conditional-format
// ---------------------------------------------------------------------------

async fn handle_conditional_format(matches: &ArgMatches) -> Result<(), GwsError> {
    let spreadsheet_id = matches.get_one::<String>("spreadsheet").unwrap();
    let tab_name = matches.get_one::<String>("tab").unwrap();
    let range_str = matches.get_one::<String>("range").unwrap();
    let match_text = matches.get_one::<String>("match-text").unwrap();
    let bg_hex = matches.get_one::<String>("bg-hex").unwrap();

    let (sc, sr, ec, er) = parse_a1_range(range_str)?;
    let bg_color = parse_hex_color(bg_hex)?;

    let (client, token) = sheets_auth_and_client().await?;
    let sheet_id = get_sheet_id(&client, &token, spreadsheet_id, tab_name).await?;

    let requests = json!([{
        "addConditionalFormatRule": {
            "rule": {
                "ranges": [{
                    "sheetId": sheet_id,
                    "startRowIndex": sr,
                    "endRowIndex": er,
                    "startColumnIndex": sc,
                    "endColumnIndex": ec
                }],
                "booleanRule": {
                    "condition": {
                        "type": "TEXT_EQ",
                        "values": [{
                            "userEnteredValue": match_text
                        }]
                    },
                    "format": {
                        "backgroundColor": bg_color,
                        "textFormat": {
                            "bold": true
                        }
                    }
                }
            },
            "index": 0
        }
    }]);

    batch_update(&client, &token, spreadsheet_id, requests).await?;

    println!(
        "{}",
        json!({ "rule": "added", "range": range_str, "matchText": match_text })
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// 6. +read-notes
// ---------------------------------------------------------------------------

async fn handle_read_notes(matches: &ArgMatches) -> Result<(), GwsError> {
    let spreadsheet_id = matches.get_one::<String>("spreadsheet").unwrap();
    let range_str = matches.get_one::<String>("range").unwrap();

    let (client, token) = sheets_auth_and_client().await?;

    let url = format!(
        "https://sheets.googleapis.com/v4/spreadsheets/{}",
        spreadsheet_id
    );

    let resp = client
        .get(&url)
        .query(&[
            ("includeGridData", "true"),
            ("ranges", range_str.as_str()),
            ("fields", "sheets.data.rowData.values.note"),
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
            reason: "read_notes_failed".to_string(),
            enable_url: None,
        });
    }

    let val: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| GwsError::Other(anyhow::anyhow!("JSON parse: {e}")))?;

    // Determine the start position from the range to compute cell addresses
    let range_part = range_str
        .split('!')
        .next_back()
        .unwrap_or(range_str.as_str());
    let start_col_letters: String = range_part
        .chars()
        .take_while(|c| c.is_ascii_alphabetic())
        .collect();
    let start_row_str: String = range_part
        .chars()
        .skip_while(|c| c.is_ascii_alphabetic())
        .take_while(|c| c.is_ascii_digit())
        .collect();
    let start_col = if start_col_letters.is_empty() {
        0
    } else {
        col_letters_to_index(&start_col_letters)
    };
    let start_row: usize = if start_row_str.is_empty() {
        0
    } else {
        start_row_str.parse::<usize>().unwrap_or(1) - 1
    };

    let mut notes: Vec<serde_json::Value> = Vec::new();

    if let Some(row_data) = val["sheets"][0]["data"][0]["rowData"].as_array() {
        for (ri, row) in row_data.iter().enumerate() {
            if let Some(values) = row["values"].as_array() {
                for (ci, cell) in values.iter().enumerate() {
                    if let Some(note) = cell["note"].as_str() {
                        if !note.is_empty() {
                            let cell_addr = format!(
                                "{}{}",
                                col_index_to_letters(start_col + ci),
                                start_row + ri + 1
                            );
                            notes.push(json!({ "cell": cell_addr, "note": note }));
                        }
                    }
                }
            }
        }
    }

    println!(
        "{}",
        serde_json::to_string(&notes).unwrap_or_else(|_| "[]".to_string())
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// 7. +write-note
// ---------------------------------------------------------------------------

async fn handle_write_note(matches: &ArgMatches) -> Result<(), GwsError> {
    let spreadsheet_id = matches.get_one::<String>("spreadsheet").unwrap();
    let tab_name = matches.get_one::<String>("tab").unwrap();
    let cell_str = matches.get_one::<String>("cell").unwrap();
    let note_text = matches.get_one::<String>("note").unwrap();

    let (col, row) = parse_cell_ref(cell_str)?;

    let (client, token) = sheets_auth_and_client().await?;
    let sheet_id = get_sheet_id(&client, &token, spreadsheet_id, tab_name).await?;

    let requests = json!([{
        "updateCells": {
            "rows": [{
                "values": [{
                    "note": note_text
                }]
            }],
            "fields": "note",
            "range": {
                "sheetId": sheet_id,
                "startRowIndex": row,
                "endRowIndex": row + 1,
                "startColumnIndex": col,
                "endColumnIndex": col + 1
            }
        }
    }]);

    batch_update(&client, &token, spreadsheet_id, requests).await?;

    println!("{}", json!({ "cell": cell_str, "note": note_text }));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::discovery::{RestDescription, RestMethod, RestResource};
    use std::collections::HashMap;

    fn make_mock_doc() -> RestDescription {
        let mut methods = HashMap::new();
        methods.insert(
            "append".to_string(),
            RestMethod {
                scopes: vec!["https://scope".to_string()],
                ..Default::default()
            },
        );
        methods.insert(
            "get".to_string(),
            RestMethod {
                scopes: vec!["https://scope".to_string()],
                ..Default::default()
            },
        );

        let mut values_res = RestResource::default();
        values_res.methods = methods;

        let mut spreadsheets_res = RestResource::default();
        spreadsheets_res
            .resources
            .insert("values".to_string(), values_res);

        let mut resources = HashMap::new();
        resources.insert("spreadsheets".to_string(), spreadsheets_res);

        RestDescription {
            resources,
            ..Default::default()
        }
    }

    fn make_matches_append(args: &[&str]) -> ArgMatches {
        let cmd = Command::new("test")
            .arg(Arg::new("spreadsheet").long("spreadsheet"))
            .arg(Arg::new("values").long("values"))
            .arg(Arg::new("json-values").long("json-values"));
        cmd.try_get_matches_from(args).unwrap()
    }

    fn make_matches_read(args: &[&str]) -> ArgMatches {
        let cmd = Command::new("test")
            .arg(Arg::new("spreadsheet").long("spreadsheet"))
            .arg(Arg::new("range").long("range"));
        cmd.try_get_matches_from(args).unwrap()
    }

    #[test]
    fn test_build_append_request() {
        let doc = make_mock_doc();
        let config = AppendConfig {
            spreadsheet_id: "123".to_string(),
            values: vec![vec!["a".to_string(), "b".to_string(), "c".to_string()]],
        };
        let (params, body, scopes) = build_append_request(&config, &doc).unwrap();

        assert!(params.contains("123"));
        assert!(params.contains("USER_ENTERED"));
        assert!(body.contains("a"));
        assert!(body.contains("b"));
        assert_eq!(scopes[0], "https://scope");
    }

    #[test]
    fn test_build_read_request() {
        let doc = make_mock_doc();
        let config = ReadConfig {
            spreadsheet_id: "123".to_string(),
            range: "A1:B2".to_string(),
        };
        let (params, scopes) = build_read_request(&config, &doc).unwrap();

        assert!(params.contains("123"));
        assert!(params.contains("A1:B2"));
        assert_eq!(scopes[0], "https://scope");
    }

    #[test]
    fn test_parse_append_args_values() {
        let matches = make_matches_append(&["test", "--spreadsheet", "123", "--values", "a,b,c"]);
        let config = parse_append_args(&matches);
        assert_eq!(config.spreadsheet_id, "123");
        assert_eq!(config.values, vec![vec!["a", "b", "c"]]);
    }

    #[test]
    fn test_parse_append_args_json_single_row() {
        let matches = make_matches_append(&[
            "test",
            "--spreadsheet",
            "123",
            "--json-values",
            r#"["a","b","c"]"#,
        ]);
        let config = parse_append_args(&matches);
        assert_eq!(config.values, vec![vec!["a", "b", "c"]]);
    }

    #[test]
    fn test_parse_append_args_json_multi_row() {
        let matches = make_matches_append(&[
            "test",
            "--spreadsheet",
            "123",
            "--json-values",
            r#"[["Alice","100"],["Bob","200"]]"#,
        ]);
        let config = parse_append_args(&matches);
        assert_eq!(
            config.values,
            vec![vec!["Alice", "100"], vec!["Bob", "200"]]
        );
    }

    #[test]
    fn test_build_append_request_multi_row() {
        let doc = make_mock_doc();
        let config = AppendConfig {
            spreadsheet_id: "123".to_string(),
            values: vec![
                vec!["Alice".to_string(), "100".to_string()],
                vec!["Bob".to_string(), "200".to_string()],
            ],
        };
        let (_params, body, _scopes) = build_append_request(&config, &doc).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
        let values = parsed["values"].as_array().unwrap();
        assert_eq!(values.len(), 2);
        assert_eq!(values[0], json!(["Alice", "100"]));
        assert_eq!(values[1], json!(["Bob", "200"]));
    }

    #[test]
    fn test_parse_read_args() {
        let matches = make_matches_read(&["test", "--spreadsheet", "123", "--range", "A1:B2"]);
        let config = parse_read_args(&matches);
        assert_eq!(config.spreadsheet_id, "123");
        assert_eq!(config.range, "A1:B2");
    }

    #[test]
    fn test_inject_commands() {
        let helper = SheetsHelper;
        let cmd = Command::new("test");
        let doc = crate::discovery::RestDescription::default();

        let cmd = helper.inject_commands(cmd, &doc);
        let subcommands: Vec<_> = cmd.get_subcommands().map(|s| s.get_name()).collect();
        assert!(subcommands.contains(&"+append"));
        assert!(subcommands.contains(&"+read"));
    }

    // --- soul-codes helper tests ---
    // Add these test functions inside the existing `#[cfg(test)] mod tests { ... }` block.

    // -----------------------------------------------------------------------
    // A1 notation helper tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_col_letters_to_index() {
        assert_eq!(col_letters_to_index("A"), 0);
        assert_eq!(col_letters_to_index("B"), 1);
        assert_eq!(col_letters_to_index("Z"), 25);
        assert_eq!(col_letters_to_index("AA"), 26);
        assert_eq!(col_letters_to_index("AB"), 27);
        assert_eq!(col_letters_to_index("AZ"), 51);
        assert_eq!(col_letters_to_index("BA"), 52);
    }

    #[test]
    fn test_col_index_to_letters() {
        assert_eq!(col_index_to_letters(0), "A");
        assert_eq!(col_index_to_letters(1), "B");
        assert_eq!(col_index_to_letters(25), "Z");
        assert_eq!(col_index_to_letters(26), "AA");
        assert_eq!(col_index_to_letters(27), "AB");
        assert_eq!(col_index_to_letters(51), "AZ");
        assert_eq!(col_index_to_letters(52), "BA");
    }

    #[test]
    fn test_col_roundtrip() {
        for i in 0..200 {
            let letters = col_index_to_letters(i);
            assert_eq!(
                col_letters_to_index(&letters),
                i,
                "roundtrip failed for index {i} -> {letters}"
            );
        }
    }

    #[test]
    fn test_parse_cell_ref() {
        let (col, row) = parse_cell_ref("A1").unwrap();
        assert_eq!(col, 0);
        assert_eq!(row, 0);

        let (col, row) = parse_cell_ref("B5").unwrap();
        assert_eq!(col, 1);
        assert_eq!(row, 4);

        let (col, row) = parse_cell_ref("AA10").unwrap();
        assert_eq!(col, 26);
        assert_eq!(row, 9);

        let (col, row) = parse_cell_ref("Z1").unwrap();
        assert_eq!(col, 25);
        assert_eq!(row, 0);
    }

    #[test]
    fn test_parse_cell_ref_invalid() {
        assert!(parse_cell_ref("123").is_err());
        assert!(parse_cell_ref("").is_err());
    }

    #[test]
    fn test_parse_a1_range_pair() {
        let (sc, sr, ec, er) = parse_a1_range("A1:C3").unwrap();
        assert_eq!((sc, sr, ec, er), (0, 0, 3, 3));
    }

    #[test]
    fn test_parse_a1_range_single_cell() {
        let (sc, sr, ec, er) = parse_a1_range("B5").unwrap();
        assert_eq!((sc, sr, ec, er), (1, 4, 2, 5));
    }

    #[test]
    fn test_parse_a1_range_with_sheet_prefix() {
        let (sc, sr, ec, er) = parse_a1_range("Sheet1!A1:F1").unwrap();
        assert_eq!((sc, sr, ec, er), (0, 0, 6, 1));
    }

    #[test]
    fn test_parse_a1_range_wide() {
        let (sc, sr, ec, er) = parse_a1_range("A1:Z100").unwrap();
        assert_eq!((sc, sr, ec, er), (0, 0, 26, 100));
    }

    // -----------------------------------------------------------------------
    // Hex color parser tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_hex_color_basic() {
        let c = parse_hex_color("#445269").unwrap();
        // 0x44=68, 0x52=82, 0x69=105
        let r = c["red"].as_f64().unwrap();
        let g = c["green"].as_f64().unwrap();
        let b = c["blue"].as_f64().unwrap();
        assert!((r - 68.0 / 255.0).abs() < 0.001);
        assert!((g - 82.0 / 255.0).abs() < 0.001);
        assert!((b - 105.0 / 255.0).abs() < 0.001);
    }

    #[test]
    fn test_parse_hex_color_white() {
        let c = parse_hex_color("#FFFFFF").unwrap();
        assert!((c["red"].as_f64().unwrap() - 1.0).abs() < 0.001);
        assert!((c["green"].as_f64().unwrap() - 1.0).abs() < 0.001);
        assert!((c["blue"].as_f64().unwrap() - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_parse_hex_color_black() {
        let c = parse_hex_color("#000000").unwrap();
        assert!(c["red"].as_f64().unwrap().abs() < 0.001);
        assert!(c["green"].as_f64().unwrap().abs() < 0.001);
        assert!(c["blue"].as_f64().unwrap().abs() < 0.001);
    }

    #[test]
    fn test_parse_hex_color_no_hash() {
        let c = parse_hex_color("FF0000").unwrap();
        assert!((c["red"].as_f64().unwrap() - 1.0).abs() < 0.001);
        assert!(c["green"].as_f64().unwrap().abs() < 0.001);
        assert!(c["blue"].as_f64().unwrap().abs() < 0.001);
    }

    #[test]
    fn test_parse_hex_color_invalid() {
        assert!(parse_hex_color("#GGG").is_err());
        assert!(parse_hex_color("#12345").is_err());
        assert!(parse_hex_color("").is_err());
    }

    // -----------------------------------------------------------------------
    // inject_commands: verify all 7 new subcommands are registered
    // -----------------------------------------------------------------------

    #[test]
    fn test_inject_commands_new_helpers() {
        let helper = SheetsHelper;
        let cmd = Command::new("test");
        let doc = crate::discovery::RestDescription::default();

        let cmd = helper.inject_commands(cmd, &doc);
        let subcommands: Vec<&str> = cmd.get_subcommands().map(|s| s.get_name()).collect();

        // Existing
        assert!(subcommands.contains(&"+append"));
        assert!(subcommands.contains(&"+read"));

        // New
        assert!(subcommands.contains(&"+add-tab"), "missing +add-tab");
        assert!(subcommands.contains(&"+freeze"), "missing +freeze");
        assert!(
            subcommands.contains(&"+column-width"),
            "missing +column-width"
        );
        assert!(
            subcommands.contains(&"+format-header"),
            "missing +format-header"
        );
        assert!(
            subcommands.contains(&"+conditional-format"),
            "missing +conditional-format"
        );
        assert!(subcommands.contains(&"+read-notes"), "missing +read-notes");
        assert!(subcommands.contains(&"+write-note"), "missing +write-note");
    }
}
