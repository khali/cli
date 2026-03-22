---
"@googleworkspace/cli": minor
---

Add `gws sheets +version-tag` helper command.

Tags a named checkpoint in a spreadsheet's version history by appending a row
to the VERSION HISTORY table in the Working Notes tab. Columns written:
label | date (UTC) | Drive revision ID | ISO timestamp.

The Drive revision ID links the tag directly to Google Drive's built-in version
history (File > Version history > See version history), letting users identify
and restore exact snapshots from the CLI.

Usage:
  gws sheets +version-tag --spreadsheet SHEET_ID --label "v1.0 pre-deployment"
