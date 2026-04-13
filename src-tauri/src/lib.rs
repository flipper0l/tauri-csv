use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Manager};

const APP_STATE_FILE: &str = "app-state.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SheetSummary {
    name: String,
    columns: usize,
    rows: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct VersionMeta {
    id: String,
    label: String,
    created_at: String,
    source: String,
    change_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ProjectManifest {
    id: String,
    name: String,
    imported_at: String,
    original_path: String,
    sheets: Vec<SheetSummary>,
    versions: Vec<VersionMeta>,
    active_version_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct AppStateFile {
    current_project_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct WorkbookVersion {
    id: String,
    label: String,
    created_at: String,
    source: String,
    changes: Vec<ChangeLogEntry>,
    sheets: Vec<SheetData>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SheetData {
    name: String,
    columns: Vec<String>,
    rows: Vec<RowData>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RowData {
    key: String,
    cells: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ChangeLogEntry {
    sheet_name: String,
    row_index: usize,
    row_key: String,
    note: String,
    before: Vec<String>,
    after: Vec<String>,
}

#[derive(Debug, Serialize)]
struct ProjectView {
    project_id: String,
    name: String,
    imported_at: String,
    original_path: String,
    sheets: Vec<SheetSummary>,
    versions: Vec<VersionMeta>,
    active_version_id: String,
}

#[derive(Debug, Clone, Deserialize)]
struct SheetUpload {
    name: String,
    columns: Vec<String>,
    rows: Vec<Vec<String>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ImportWorkbookRequest {
    path: String,
    name: String,
    sheets: Vec<SheetUpload>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ColumnFilter {
    column: String,
    value: String,
}

#[derive(Debug, Serialize)]
struct PreviewRow {
    key: String,
    cells: Vec<String>,
}

#[derive(Debug, Serialize)]
struct SheetPreview {
    sheet_name: String,
    columns: Vec<String>,
    rows: Vec<PreviewRow>,
    total: usize,
    page: usize,
    page_size: usize,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PreviewRequest {
    project_id: String,
    version_id: String,
    sheet_name: String,
    page: usize,
    page_size: usize,
    filters: Vec<ColumnFilter>,
}

#[derive(Debug, Serialize)]
struct ReviewField {
    column: String,
    value: String,
}

#[derive(Debug, Serialize)]
struct PendingReviewItem {
    sheet_name: String,
    row_key: String,
    row_index: usize,
    note: String,
    fields: Vec<ReviewField>,
}

#[derive(Debug, Serialize)]
struct NotesImportResult {
    base_version_id: String,
    total_rows_with_notes: usize,
    matched_rows: usize,
    unmatched_rows: usize,
    warnings: Vec<String>,
    review_items: Vec<PendingReviewItem>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ImportNotesRequest {
    project_id: String,
    version_id: String,
    sheet_name: String,
    sheet: SheetUpload,
}

#[derive(Debug, Deserialize)]
struct ReviewedFieldInput {
    column: String,
    value: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReviewedItemInput {
    sheet_name: String,
    row_key: String,
    row_index: usize,
    note: String,
    apply: bool,
    fields: Vec<ReviewedFieldInput>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApplyNotesImportRequest {
    project_id: String,
    base_version_id: String,
    label: Option<String>,
    reviewed_items: Vec<ReviewedItemInput>,
}

#[derive(Debug, Serialize)]
struct ApplyNotesImportResult {
    project: ProjectView,
    created_version_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExportVersionRequest {
    project_id: String,
    version_id: String,
    filters: Vec<ColumnFilter>,
}

#[derive(Debug, Serialize)]
struct ExportSheetData {
    name: String,
    columns: Vec<String>,
    rows: Vec<Vec<String>>,
}

#[derive(Debug, Serialize)]
struct ExportResult {
    workbook_name: String,
    version_label: String,
    sheets: Vec<ExportSheetData>,
}

fn now_epoch_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

fn now_iso_string() -> String {
    format!("{}", now_epoch_ms())
}

fn slugify(value: &str) -> String {
    let mut out = String::new();
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else if (ch.is_ascii_whitespace() || ch == '-' || ch == '_') && !out.ends_with('-') {
            out.push('-');
        }
    }

    out.trim_matches('-').to_string()
}

fn normalize_header(value: &str) -> String {
    value
        .trim()
        .to_ascii_lowercase()
        .replace([' ', '-', '.', '/'], "_")
}

fn build_row_key(cells: &[String]) -> String {
    let joined = cells.join("\u{1f}");
    let mut hasher = Sha256::new();
    hasher.update(joined.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn data_root(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("excel-reviewer");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

fn state_file_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(data_root(app)?.join(APP_STATE_FILE))
}

fn projects_root(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = data_root(app)?.join("projects");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

fn project_dir(app: &AppHandle, project_id: &str) -> Result<PathBuf, String> {
    let dir = projects_root(app)?.join(project_id);
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

fn manifest_path(app: &AppHandle, project_id: &str) -> Result<PathBuf, String> {
    Ok(project_dir(app, project_id)?.join("project.json"))
}

fn version_path(app: &AppHandle, project_id: &str, version_id: &str) -> Result<PathBuf, String> {
    Ok(project_dir(app, project_id)?.join(format!("version-{version_id}.json")))
}

fn write_json<T: Serialize>(path: &Path, payload: &T) -> Result<(), String> {
    let json = serde_json::to_string_pretty(payload).map_err(|e| e.to_string())?;
    fs::write(path, json).map_err(|e| e.to_string())
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, String> {
    let raw = fs::read_to_string(path).map_err(|e| e.to_string())?;
    serde_json::from_str(&raw).map_err(|e| e.to_string())
}

fn save_app_state(app: &AppHandle, current_project_id: Option<String>) -> Result<(), String> {
    write_json(
        &state_file_path(app)?,
        &AppStateFile {
            current_project_id,
        },
    )
}

fn load_app_state(app: &AppHandle) -> Result<AppStateFile, String> {
    let path = state_file_path(app)?;
    if !path.exists() {
        return Ok(AppStateFile::default());
    }
    read_json(&path)
}

fn load_project_manifest(app: &AppHandle, project_id: &str) -> Result<ProjectManifest, String> {
    read_json(&manifest_path(app, project_id)?)
}

fn save_project_manifest(
    app: &AppHandle,
    manifest: &ProjectManifest,
) -> Result<ProjectManifest, String> {
    write_json(&manifest_path(app, &manifest.id)?, manifest)?;
    Ok(manifest.clone())
}

fn load_version(
    app: &AppHandle,
    project_id: &str,
    version_id: &str,
) -> Result<WorkbookVersion, String> {
    read_json(&version_path(app, project_id, version_id)?)
}

fn save_version(app: &AppHandle, project_id: &str, version: &WorkbookVersion) -> Result<(), String> {
    write_json(&version_path(app, project_id, &version.id)?, version)
}

fn manifest_to_view(manifest: &ProjectManifest) -> ProjectView {
    ProjectView {
        project_id: manifest.id.clone(),
        name: manifest.name.clone(),
        imported_at: manifest.imported_at.clone(),
        original_path: manifest.original_path.clone(),
        sheets: manifest.sheets.clone(),
        versions: manifest.versions.clone(),
        active_version_id: manifest.active_version_id.clone(),
    }
}

fn workbook_name_from_path(path: &str) -> String {
    Path::new(path)
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or("Workbook")
        .to_string()
}

fn convert_sheet_upload(upload: &SheetUpload) -> SheetData {
    let width = upload.columns.len();
    let rows = upload
        .rows
        .iter()
        .filter_map(|row| {
            let mut padded = row.clone();
            padded.resize(width, String::new());
            if padded.iter().all(|value| value.trim().is_empty()) {
                return None;
            }

            Some(RowData {
                key: build_row_key(&padded),
                cells: padded,
            })
        })
        .collect::<Vec<_>>();

    SheetData {
        name: upload.name.clone(),
        columns: upload.columns.clone(),
        rows,
    }
}

fn filters_match(columns: &[String], row: &RowData, filters: &[ColumnFilter]) -> bool {
    filters.iter().all(|filter| {
        if filter.value.trim().is_empty() {
            return true;
        }

        let normalized = normalize_header(&filter.column);
        let Some(index) = columns
            .iter()
            .position(|column| normalize_header(column) == normalized)
        else {
            return true;
        };

        row.cells
            .get(index)
            .map(|value| {
                value
                    .to_ascii_lowercase()
                    .contains(&filter.value.trim().to_ascii_lowercase())
            })
            .unwrap_or(false)
    })
}

fn note_column_index(columns: &[String]) -> Option<usize> {
    let candidates = [
        "note",
        "notes",
        "nota",
        "comment",
        "comments",
        "commento",
        "commenti",
    ];

    columns.iter().position(|column| {
        let normalized = normalize_header(column);
        candidates.iter().any(|candidate| normalized.contains(candidate))
    })
}

#[tauri::command]
fn get_current_project(app: AppHandle) -> Result<Option<ProjectView>, String> {
    let state = load_app_state(&app)?;
    let Some(project_id) = state.current_project_id else {
        return Ok(None);
    };

    let manifest = load_project_manifest(&app, &project_id)?;
    Ok(Some(manifest_to_view(&manifest)))
}

#[tauri::command]
fn import_original_workbook(
    app: AppHandle,
    payload: ImportWorkbookRequest,
) -> Result<ProjectView, String> {
    let sheets = payload
        .sheets
        .iter()
        .map(convert_sheet_upload)
        .collect::<Vec<_>>();
    if sheets.is_empty() {
        return Err("Il file Excel non contiene fogli leggibili.".to_string());
    }

    let project_id = format!(
        "{}-{}",
        slugify(&payload.name),
        now_epoch_ms()
    );
    let version_id = format!("v{}", now_epoch_ms());
    let imported_at = now_iso_string();

    let version = WorkbookVersion {
        id: version_id.clone(),
        label: "Versione 1".to_string(),
        created_at: imported_at.clone(),
        source: "Import originale".to_string(),
        changes: Vec::new(),
        sheets: sheets.clone(),
    };

    let manifest = ProjectManifest {
        id: project_id.clone(),
        name: payload.name,
        imported_at: imported_at.clone(),
        original_path: payload.path,
        sheets: sheets
            .iter()
            .map(|sheet| SheetSummary {
                name: sheet.name.clone(),
                columns: sheet.columns.len(),
                rows: sheet.rows.len(),
            })
            .collect(),
        versions: vec![VersionMeta {
            id: version_id.clone(),
            label: "Versione 1".to_string(),
            created_at: imported_at,
            source: "Import originale".to_string(),
            change_count: 0,
        }],
        active_version_id: version_id.clone(),
    };

    save_version(&app, &project_id, &version)?;
    let manifest = save_project_manifest(&app, &manifest)?;
    save_app_state(&app, Some(project_id))?;

    Ok(manifest_to_view(&manifest))
}

#[tauri::command]
fn get_sheet_preview(app: AppHandle, payload: PreviewRequest) -> Result<SheetPreview, String> {
    if payload.page == 0 || payload.page_size == 0 {
        return Err("La paginazione deve partire da 1.".to_string());
    }

    let version = load_version(&app, &payload.project_id, &payload.version_id)?;
    let Some(sheet) = version
        .sheets
        .iter()
        .find(|sheet| sheet.name == payload.sheet_name)
    else {
        return Err("Foglio non trovato nella versione selezionata.".to_string());
    };

    let filtered_rows = sheet
        .rows
        .iter()
        .filter(|row| filters_match(&sheet.columns, row, &payload.filters))
        .collect::<Vec<_>>();

    let total = filtered_rows.len();
    let start = payload.page.saturating_sub(1) * payload.page_size;
    let page_rows = filtered_rows
        .into_iter()
        .skip(start)
        .take(payload.page_size)
        .map(|row| PreviewRow {
            key: row.key.clone(),
            cells: row.cells.clone(),
        })
        .collect();

    Ok(SheetPreview {
        sheet_name: sheet.name.clone(),
        columns: sheet.columns.clone(),
        rows: page_rows,
        total,
        page: payload.page,
        page_size: payload.page_size,
    })
}

#[tauri::command]
fn import_notes_workbook(
    app: AppHandle,
    payload: ImportNotesRequest,
) -> Result<NotesImportResult, String> {
    let version = load_version(&app, &payload.project_id, &payload.version_id)?;
    let mut warnings = Vec::new();
    let mut review_items = Vec::new();
    let mut total_rows_with_notes = 0usize;
    let mut matched_rows = 0usize;
    let mut unmatched_rows = 0usize;

    let Some(source_sheet) = version
        .sheets
        .iter()
        .find(|sheet| sheet.name == payload.sheet_name)
    else {
        return Err("Il foglio selezionato non esiste nella versione corrente.".to_string());
    };

    let returned_sheet = convert_sheet_upload(&payload.sheet);

    let Some(note_index) = note_column_index(&returned_sheet.columns) else {
        return Err(format!(
            "Nessuna colonna note riconosciuta nel file di ritorno per il foglio '{}'.",
            source_sheet.name
        ));
    };

    let mut column_index_by_name = HashMap::new();
    for (index, column) in returned_sheet.columns.iter().enumerate() {
        column_index_by_name.insert(normalize_header(column), index);
    }

    let mut missing_columns = Vec::new();
    for column in &source_sheet.columns {
        if !column_index_by_name.contains_key(&normalize_header(column)) {
            missing_columns.push(column.clone());
        }
    }

    if !missing_columns.is_empty() {
        return Err(format!(
            "Nel file di ritorno per il foglio '{}' mancano colonne chiave: {}.",
            source_sheet.name,
            missing_columns.join(", ")
        ));
    }

    let mut base_occurrences: HashMap<String, Vec<usize>> = HashMap::new();
    for (row_index, row) in source_sheet.rows.iter().enumerate() {
        base_occurrences
            .entry(row.key.clone())
            .or_default()
            .push(row_index);
    }

    let mut returned_occurrences: HashMap<String, usize> = HashMap::new();
    for row in &returned_sheet.rows {
        let note = row.cells.get(note_index).cloned().unwrap_or_default();
        if note.trim().is_empty() {
            continue;
        }

        total_rows_with_notes += 1;

        let aligned_cells = source_sheet
            .columns
            .iter()
            .map(|column| {
                let index = column_index_by_name[&normalize_header(column)];
                row.cells.get(index).cloned().unwrap_or_default()
            })
            .collect::<Vec<_>>();

        let row_key = build_row_key(&aligned_cells);
        let occurrence = returned_occurrences.entry(row_key.clone()).or_insert(0usize);

        let Some(base_indices) = base_occurrences.get(&row_key) else {
            unmatched_rows += 1;
            warnings.push(format!(
                "Riga con nota non trovata nel foglio '{}': '{}'.",
                source_sheet.name, note
            ));
            *occurrence += 1;
            continue;
        };

        let Some(row_index) = base_indices.get(*occurrence).copied() else {
            unmatched_rows += 1;
            warnings.push(format!(
                "Riga duplicata non allineata nel foglio '{}': '{}'.",
                source_sheet.name, note
            ));
            *occurrence += 1;
            continue;
        };

        let source_row = &source_sheet.rows[row_index];
        review_items.push(PendingReviewItem {
            sheet_name: source_sheet.name.clone(),
            row_key: source_row.key.clone(),
            row_index,
            note,
            fields: source_sheet
                .columns
                .iter()
                .zip(source_row.cells.iter())
                .map(|(column, value)| ReviewField {
                    column: column.clone(),
                    value: value.clone(),
                })
                .collect(),
        });
        matched_rows += 1;
        *occurrence += 1;
    }

    if total_rows_with_notes == 0 {
        warnings.push(format!(
            "Nessuna riga con note trovata nel file di ritorno per il foglio '{}'.",
            source_sheet.name
        ));
    }

    Ok(NotesImportResult {
        base_version_id: payload.version_id,
        total_rows_with_notes,
        matched_rows,
        unmatched_rows,
        warnings,
        review_items,
    })
}

#[tauri::command]
fn apply_notes_import(
    app: AppHandle,
    payload: ApplyNotesImportRequest,
) -> Result<ApplyNotesImportResult, String> {
    let mut manifest = load_project_manifest(&app, &payload.project_id)?;
    let mut version = load_version(&app, &payload.project_id, &payload.base_version_id)?;

    let mut applied_changes = Vec::new();
    for reviewed_item in payload.reviewed_items {
        if !reviewed_item.apply {
            continue;
        }

        let Some(sheet) = version
            .sheets
            .iter_mut()
            .find(|sheet| sheet.name == reviewed_item.sheet_name)
        else {
            continue;
        };

        let Some(row) = sheet.rows.get_mut(reviewed_item.row_index) else {
            continue;
        };

        if row.key != reviewed_item.row_key {
            continue;
        }

        let before = row.cells.clone();
        let mut after = before.clone();
        for field in reviewed_item.fields {
            if let Some(index) = sheet
                .columns
                .iter()
                .position(|column| normalize_header(column) == normalize_header(&field.column))
            {
                after[index] = field.value;
            }
        }

        row.cells = after.clone();
        row.key = build_row_key(&after);
        applied_changes.push(ChangeLogEntry {
            sheet_name: sheet.name.clone(),
            row_index: reviewed_item.row_index,
            row_key: row.key.clone(),
            note: reviewed_item.note,
            before,
            after,
        });
    }

    let new_version_id = format!("v{}", now_epoch_ms());
    let label = payload
        .label
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| format!("Versione {}", manifest.versions.len() + 1));
    let created_at = now_iso_string();

    version.id = new_version_id.clone();
    version.label = label.clone();
    version.created_at = created_at.clone();
    version.source = "Import note revisionato".to_string();
    version.changes = applied_changes.clone();

    save_version(&app, &payload.project_id, &version)?;

    manifest.active_version_id = new_version_id.clone();
    manifest.versions.push(VersionMeta {
        id: new_version_id.clone(),
        label,
        created_at,
        source: "Import note revisionato".to_string(),
        change_count: applied_changes.len(),
    });
    let manifest = save_project_manifest(&app, &manifest)?;
    save_app_state(&app, Some(payload.project_id))?;

    Ok(ApplyNotesImportResult {
        project: manifest_to_view(&manifest),
        created_version_id: new_version_id,
    })
}

#[tauri::command]
fn export_version(app: AppHandle, payload: ExportVersionRequest) -> Result<ExportResult, String> {
    let version = load_version(&app, &payload.project_id, &payload.version_id)?;
    let manifest = load_project_manifest(&app, &payload.project_id)?;
    let version_meta = manifest
        .versions
        .iter()
        .find(|version_meta| version_meta.id == payload.version_id)
        .ok_or_else(|| "Versione non trovata per l'export.".to_string())?;
    let mut sheets = Vec::new();

    for sheet in &version.sheets {
        let rows = sheet
            .rows
            .iter()
            .filter(|row| filters_match(&sheet.columns, row, &payload.filters))
            .collect::<Vec<_>>();
        sheets.push(ExportSheetData {
            name: sheet.name.clone(),
            columns: sheet.columns.clone(),
            rows: rows.iter().map(|row| row.cells.clone()).collect(),
        });
    }

    Ok(ExportResult {
        workbook_name: manifest.name,
        version_label: version_meta.label.clone(),
        sheets,
    })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            get_current_project,
            import_original_workbook,
            get_sheet_preview,
            import_notes_workbook,
            apply_notes_import,
            export_version
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
