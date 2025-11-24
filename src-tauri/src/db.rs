use rusqlite::{params, Connection, Result};
use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use tauri::Manager;

#[derive(Debug, Serialize, Deserialize)]
pub struct WindowState {
    pub width: f64,
    pub height: f64,
    pub x: f64,
    pub y: f64,
    pub pinned: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TodoItem {
    pub id: u32,
    pub text: String,
    pub completed: bool,
    pub parent_id: Option<u32>,
    pub position: i32,
    pub target_count: Option<i32>,
    pub current_count: i32,
}

pub fn init_db(app_handle: &AppHandle) -> Result<()> {
    let app_dir = app_handle.path().app_data_dir().unwrap();
    std::fs::create_dir_all(&app_dir).unwrap();
    let db_path = app_dir.join("sticky_notes.db");
    println!("Database path: {:?}", db_path);
    
    let conn = Connection::open(db_path)?;
    
    conn.execute(
        "CREATE TABLE IF NOT EXISTS notes (
            id INTEGER PRIMARY KEY,
            content TEXT
        )",
        [],
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS todos (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            text TEXT NOT NULL,
            completed BOOLEAN NOT NULL,
            parent_id INTEGER,
            position INTEGER DEFAULT 0,
            target_count INTEGER,
            current_count INTEGER DEFAULT 0,
            FOREIGN KEY(parent_id) REFERENCES todos(id) ON DELETE CASCADE
        )",
        [],
    )?;

    // Migration: Add columns if they don't exist (simplistic approach)
    let _ = conn.execute("ALTER TABLE todos ADD COLUMN parent_id INTEGER", []);
    let _ = conn.execute("ALTER TABLE todos ADD COLUMN position INTEGER DEFAULT 0", []);
    let _ = conn.execute("ALTER TABLE todos ADD COLUMN target_count INTEGER", []);
    let _ = conn.execute("ALTER TABLE todos ADD COLUMN current_count INTEGER DEFAULT 0", []);

    // Create window_state table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS window_state (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            width REAL NOT NULL DEFAULT 300,
            height REAL NOT NULL DEFAULT 300,
            x REAL NOT NULL DEFAULT 100,
            y REAL NOT NULL DEFAULT 100,
            pinned INTEGER NOT NULL DEFAULT 0
        )",
        [],
    )?;

    // Initialize default note if empty
    let count: i32 = conn.query_row("SELECT count(*) FROM notes", [], |row| row.get(0))?;
    if count == 0 {
        conn.execute("INSERT INTO notes (id, content) VALUES (1, '')", [])?;
    }

    Ok(())
}

pub fn get_note(app_handle: &AppHandle) -> Result<String> {
    let app_dir = app_handle.path().app_data_dir().unwrap();
    let db_path = app_dir.join("sticky_notes.db");
    let conn = Connection::open(db_path)?;
    
    let content: String = conn.query_row(
        "SELECT content FROM notes WHERE id = 1",
        [],
        |row| row.get(0),
    )?;
    
    Ok(content)
}

pub fn save_note(app_handle: &AppHandle, content: String) -> Result<()> {
    let app_dir = app_handle.path().app_data_dir().unwrap();
    let db_path = app_dir.join("sticky_notes.db");
    let conn = Connection::open(db_path)?;
    
    conn.execute(
        "UPDATE notes SET content = ?1 WHERE id = 1",
        params![content],
    )?;
    
    Ok(())
}

pub fn get_todos(app_handle: &AppHandle) -> Result<Vec<TodoItem>> {
    let app_dir = app_handle.path().app_data_dir().unwrap();
    let db_path = app_dir.join("sticky_notes.db");
    let conn = Connection::open(db_path)?;
    
    let mut stmt = conn.prepare("SELECT id, text, completed, parent_id, position, target_count, current_count FROM todos ORDER BY position ASC")?;
    let todo_iter = stmt.query_map([], |row| {
        Ok(TodoItem {
            id: row.get(0)?,
            text: row.get(1)?,
            completed: row.get(2)?,
            parent_id: row.get(3)?,
            position: row.get(4)?,
            target_count: row.get(5)?,
            current_count: row.get(6)?,
        })
    })?;

    let mut todos = Vec::new();
    for todo in todo_iter {
        todos.push(todo?);
    }
    
    Ok(todos)
}

pub fn save_todo(app_handle: &AppHandle, text: String) -> Result<u32> {
    let app_dir = app_handle.path().app_data_dir().unwrap();
    let db_path = app_dir.join("sticky_notes.db");
    let conn = Connection::open(db_path)?;
    
    // Get max position to append to end
    let max_pos: Result<i32> = conn.query_row(
        "SELECT COALESCE(MAX(position), -1) FROM todos WHERE parent_id IS NULL",
        [],
        |row| row.get(0),
    );
    let position = max_pos.unwrap_or(-1) + 1;
    
    println!("[DB] Creating new todo with position: {}", position);

    conn.execute(
        "INSERT INTO todos (text, completed, parent_id, position) VALUES (?1, ?2, ?3, ?4)",
        params![text, false, None::<u32>, position],
    )?;
    
    let id = conn.last_insert_rowid() as u32;
    println!("[DB] Created todo id={} at position={}", id, position);
    Ok(id)
}

pub fn update_todo(app_handle: &AppHandle, id: u32, completed: bool) -> Result<()> {
    let app_dir = app_handle.path().app_data_dir().unwrap();
    let db_path = app_dir.join("sticky_notes.db");
    let mut conn = Connection::open(db_path)?;
    
    let tx = conn.transaction()?;

    println!("[DB] update_todo: id={}, completed={}", id, completed);

    // 1. Update the target item
    tx.execute(
        "UPDATE todos SET completed = ?1 WHERE id = ?2",
        params![completed, id],
    )?;

    // 2. Cascade Down: Update all descendants
    // Use recursive CTE to find all descendant IDs
    let affected = tx.execute(
        "WITH RECURSIVE descendants(id) AS (
            SELECT id FROM todos WHERE parent_id = ?1
            UNION ALL
            SELECT t.id FROM todos t
            JOIN descendants d ON t.parent_id = d.id
        )
        UPDATE todos SET completed = ?2 WHERE id IN descendants",
        params![id, completed],
    )?;
    println!("[DB] Cascade Down: Updated {} descendants", affected);

    // 3. Cascade Up: Update ancestors
    let mut current_id = id;
    loop {
        // Get parent of current_id
        let parent_id: Option<u32> = tx.query_row(
            "SELECT parent_id FROM todos WHERE id = ?",
            params![current_id],
            |row| row.get(0),
        )?;

        let parent_id = match parent_id {
            Some(pid) => pid,
            None => {
                println!("[DB] Cascade Up: Reached root at id={}", current_id);
                break; // No parent, we are at root
            },
        };

        // Check siblings status
        let (total, completed_count): (i32, i32) = tx.query_row(
            "SELECT COUNT(*), COALESCE(SUM(CASE WHEN completed THEN 1 ELSE 0 END), 0)
             FROM todos WHERE parent_id = ?",
            params![parent_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;

        let new_parent_status = total > 0 && total == completed_count;
        println!("[DB] Cascade Up: Checking parent={}, total={}, completed={}, new_status={}", parent_id, total, completed_count, new_parent_status);

        // Update parent
        tx.execute(
            "UPDATE todos SET completed = ? WHERE id = ?",
            params![new_parent_status, parent_id],
        )?;

        // Move up
        current_id = parent_id;
    }
    
    tx.commit()?;
    println!("[DB] update_todo transaction committed");
    
    Ok(())
}

pub fn update_todo_text(app_handle: &AppHandle, id: u32, text: String) -> Result<()> {
    let app_dir = app_handle.path().app_data_dir().unwrap();
    let db_path = app_dir.join("sticky_notes.db");
    let conn = Connection::open(db_path)?;
    
    conn.execute(
        "UPDATE todos SET text = ?1 WHERE id = ?2",
        params![text, id],
    )?;
    
    Ok(())
}

pub fn delete_todo(app_handle: &AppHandle, id: u32) -> Result<()> {
    let app_dir = app_handle.path().app_data_dir().unwrap();
    let db_path = app_dir.join("sticky_notes.db");
    let conn = Connection::open(db_path)?;
    
    conn.execute(
        "DELETE FROM todos WHERE id = ?1",
        params![id],
    )?;
    
    Ok(())
}

pub fn move_todo(app_handle: &AppHandle, id: u32, target_parent_id: Option<u32>, target_position: i32) -> Result<()> {
    let app_dir = app_handle.path().app_data_dir().unwrap();
    let db_path = app_dir.join("sticky_notes.db");
    let mut conn = Connection::open(db_path)?;
    
    let tx = conn.transaction()?;

    // 1. Get current state
    let (current_parent_id, current_position): (Option<u32>, i32) = tx.query_row(
        "SELECT parent_id, position FROM todos WHERE id = ?",
        params![id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;

    // 2. Remove from old list (shift items up)
    if let Some(pid) = current_parent_id {
        tx.execute(
            "UPDATE todos SET position = position - 1 WHERE parent_id = ? AND position > ?",
            params![pid, current_position],
        )?;
    } else {
        tx.execute(
            "UPDATE todos SET position = position - 1 WHERE parent_id IS NULL AND position > ?",
            params![current_position],
        )?;
    }

    // 3. Make space in new list (shift items down)
    if let Some(pid) = target_parent_id {
        tx.execute(
            "UPDATE todos SET position = position + 1 WHERE parent_id = ? AND position >= ?",
            params![pid, target_position],
        )?;
    } else {
        tx.execute(
            "UPDATE todos SET position = position + 1 WHERE parent_id IS NULL AND position >= ?",
            params![target_position],
        )?;
    }

    // 4. Update the item itself
    tx.execute(
        "UPDATE todos SET parent_id = ?, position = ? WHERE id = ?",
        params![target_parent_id, target_position, id],
    )?;

    tx.commit()?;
    
    Ok(())
}

pub fn set_todo_count(app_handle: &AppHandle, id: u32, count: Option<i32>) -> Result<()> {
    let app_dir = app_handle.path().app_data_dir().unwrap();
    let db_path = app_dir.join("sticky_notes.db");
    let conn = Connection::open(db_path)?;
    
    let current_count = count.unwrap_or(0);
    
    conn.execute(
        "UPDATE todos SET target_count = ?1, current_count = ?2 WHERE id = ?3",
        params![count, current_count, id],
    )?;
    
    Ok(())
}

pub fn decrement_todo(app_handle: &AppHandle, id: u32) -> Result<()> {
    let app_dir = app_handle.path().app_data_dir().unwrap();
    let db_path = app_dir.join("sticky_notes.db");
    let conn = Connection::open(db_path)?;
    
    // Decrement count
    conn.execute(
        "UPDATE todos SET current_count = current_count - 1 WHERE id = ? AND current_count > 0",
        params![id],
    )?;
    
    // Check if reached 0
    let current_count: i32 = conn.query_row(
        "SELECT current_count FROM todos WHERE id = ?",
        params![id],
        |row| row.get(0),
    )?;
    
    if current_count <= 0 {
        // Mark as completed and trigger cascade
        update_todo(app_handle, id, true)?;
    }
    
    Ok(())
}

pub fn reset_all_todos(app_handle: &AppHandle) -> Result<()> {
    let app_dir = app_handle.path().app_data_dir().unwrap();
    let db_path = app_dir.join("sticky_notes.db");
    let conn = Connection::open(db_path)?;
    
    // Reset all todos to incomplete and reset countdown
    conn.execute(
        "UPDATE todos SET completed = 0, current_count = COALESCE(target_count, 0)",
        [],
    )?;
    
    Ok(())
}

pub fn save_window_state(
    app_handle: &AppHandle,
    width: f64,
    height: f64,
    x: f64,
    y: f64,
    pinned: bool,
) -> Result<()> {
    let app_dir = app_handle.path().app_data_dir().unwrap();
    let db_path = app_dir.join("sticky_notes.db");
    let conn = Connection::open(db_path)?;
    
    // Use INSERT OR REPLACE to upsert
    conn.execute(
        "INSERT OR REPLACE INTO window_state (id, width, height, x, y, pinned) VALUES (1, ?, ?, ?, ?, ?)",
        params![width, height, x, y, if pinned { 1 } else { 0 }],
    )?;
    
    Ok(())
}

pub fn load_window_state(app_handle: &AppHandle) -> Result<Option<WindowState>> {
    let app_dir = app_handle.path().app_data_dir().unwrap();
    let db_path = app_dir.join("sticky_notes.db");
    let conn = Connection::open(db_path)?;
    
    let result = conn.query_row(
        "SELECT width, height, x, y, pinned FROM window_state WHERE id = 1",
        [],
        |row| {
            Ok(WindowState {
                width: row.get(0)?,
                height: row.get(1)?,
                x: row.get(2)?,
                y: row.get(3)?,
                pinned: row.get::<_, i32>(4)? != 0,
            })
        },
    );
    
    match result {
        Ok(state) => Ok(Some(state)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e),
    }
}
