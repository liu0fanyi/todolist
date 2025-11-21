use rusqlite::{params, Connection, Result};
use serde::{Deserialize, Serialize};

use tauri::AppHandle;
use tauri::Manager;

#[derive(Debug, Serialize, Deserialize)]
pub struct TodoItem {
    pub id: u32,
    pub text: String,
    pub completed: bool,
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
            completed BOOLEAN NOT NULL
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
    
    let mut stmt = conn.prepare("SELECT id, text, completed FROM todos")?;
    let todo_iter = stmt.query_map([], |row| {
        Ok(TodoItem {
            id: row.get(0)?,
            text: row.get(1)?,
            completed: row.get(2)?,
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
    
    conn.execute(
        "INSERT INTO todos (text, completed) VALUES (?1, ?2)",
        params![text, false],
    )?;
    
    let id = conn.last_insert_rowid() as u32;
    Ok(id)
}

pub fn update_todo(app_handle: &AppHandle, id: u32, completed: bool) -> Result<()> {
    let app_dir = app_handle.path().app_data_dir().unwrap();
    let db_path = app_dir.join("sticky_notes.db");
    let conn = Connection::open(db_path)?;
    
    conn.execute(
        "UPDATE todos SET completed = ?1 WHERE id = ?2",
        params![completed, id],
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
