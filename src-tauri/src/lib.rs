use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;

struct AppState {
    conn: Mutex<Connection>,
}

#[derive(Serialize)]
struct CsvLoadResult {
    inserted_rows: usize,
}

#[derive(Serialize)]
struct TableRow {
    #[serde(rename = "ID")]
    id: i64,
    name: String,
    surname: String,
}

#[derive(Serialize)]
struct PaginatedRows {
    items: Vec<TableRow>,
    total: i64,
    page: i64,
    page_size: i64,
}

#[derive(Deserialize)]
struct ImportRequest {
    path: String,
}

fn normalize_header(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

#[tauri::command]
fn import_csv_to_memory_db(
    state: tauri::State<AppState>,
    payload: ImportRequest,
) -> Result<CsvLoadResult, String> {
    let mut conn = state.conn.lock().map_err(|e| e.to_string())?;

    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS ATable (
            ID INTEGER NOT NULL,
            name TEXT NOT NULL,
            surname TEXT NOT NULL
        );
        DELETE FROM ATable;
        ",
    )
    .map_err(|e| e.to_string())?;

    let mut reader = csv::Reader::from_path(&payload.path).map_err(|e| e.to_string())?;
    let headers = reader.headers().map_err(|e| e.to_string())?.clone();

    let mut id_idx: Option<usize> = None;
    let mut name_idx: Option<usize> = None;
    let mut surname_idx: Option<usize> = None;

    for (idx, header) in headers.iter().enumerate() {
        match normalize_header(header).as_str() {
            "id" => id_idx = Some(idx),
            "name" => name_idx = Some(idx),
            "surname" => surname_idx = Some(idx),
            _ => {}
        }
    }

    let id_idx = id_idx.ok_or_else(|| "CSV is missing required header: ID".to_string())?;
    let name_idx = name_idx.ok_or_else(|| "CSV is missing required header: name".to_string())?;
    let surname_idx =
        surname_idx.ok_or_else(|| "CSV is missing required header: surname".to_string())?;

    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let mut insert_stmt = tx
        .prepare("INSERT INTO ATable (ID, name, surname) VALUES (?1, ?2, ?3)")
        .map_err(|e| e.to_string())?;

    let mut inserted_rows = 0usize;
    for (line_no, record_result) in reader.records().enumerate() {
        let record = record_result.map_err(|e| e.to_string())?;

        let id_raw = record
            .get(id_idx)
            .ok_or_else(|| format!("Missing ID value at CSV data line {}", line_no + 2))?;
        let name = record
            .get(name_idx)
            .ok_or_else(|| format!("Missing name value at CSV data line {}", line_no + 2))?;
        let surname = record.get(surname_idx).ok_or_else(|| {
            format!("Missing surname value at CSV data line {}", line_no + 2)
        })?;

        let id = id_raw.parse::<i64>().map_err(|_| {
            format!(
                "Invalid ID '{}' at CSV data line {}. ID must be an integer.",
                id_raw,
                line_no + 2
            )
        })?;

        insert_stmt
            .execute(params![id, name, surname])
            .map_err(|e| e.to_string())?;

        inserted_rows += 1;
    }

    drop(insert_stmt);
    tx.commit().map_err(|e| e.to_string())?;

    Ok(CsvLoadResult { inserted_rows })
}

#[tauri::command]
fn get_table_page(
    state: tauri::State<AppState>,
    page: i64,
    page_size: i64,
) -> Result<PaginatedRows, String> {
    if page < 1 {
        return Err("page must be >= 1".to_string());
    }
    if page_size < 1 {
        return Err("page_size must be >= 1".to_string());
    }

    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS ATable (
            ID INTEGER NOT NULL,
            name TEXT NOT NULL,
            surname TEXT NOT NULL
        );
        ",
    )
    .map_err(|e| e.to_string())?;

    let total: i64 = conn
        .query_row("SELECT COUNT(*) FROM ATable", [], |row| row.get(0))
        .map_err(|e| e.to_string())?;

    let offset = (page - 1) * page_size;
    let mut stmt = conn
        .prepare("SELECT ID, name, surname FROM ATable ORDER BY ID LIMIT ?1 OFFSET ?2")
        .map_err(|e| e.to_string())?;

    let rows_iter = stmt
        .query_map(params![page_size, offset], |row| {
            Ok(TableRow {
                id: row.get(0)?,
                name: row.get(1)?,
                surname: row.get(2)?,
            })
        })
        .map_err(|e| e.to_string())?;

    let items: Result<Vec<_>, _> = rows_iter.collect();
    let items = items.map_err(|e| e.to_string())?;

    Ok(PaginatedRows {
        items,
        total,
        page,
        page_size,
    })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let conn = Connection::open_in_memory().expect("failed to open in-memory sqlite database");

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(AppState {
            conn: Mutex::new(conn),
        })
        .invoke_handler(tauri::generate_handler![import_csv_to_memory_db, get_table_page])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
