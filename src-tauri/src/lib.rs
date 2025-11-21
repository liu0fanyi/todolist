// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
fn set_always_on_top(window: tauri::Window, always_on_top: bool) {
    println!("Setting always on top to: {}", always_on_top);
    if let Err(e) = window.set_always_on_top(always_on_top) {
        println!("Error setting always on top: {}", e);
    }
}

#[tauri::command]
fn close_window(window: tauri::Window) {
    let _ = window.close();
}

#[tauri::command]
fn start_drag(window: tauri::Window) {
    let _ = window.start_dragging();
}

mod db;

#[tauri::command]
fn load_note(app_handle: tauri::AppHandle) -> String {
    db::get_note(&app_handle).unwrap_or_default()
}

#[tauri::command]
fn save_note_content(app_handle: tauri::AppHandle, content: String) {
    let _ = db::save_note(&app_handle, content);
}

#[tauri::command]
fn load_todos(app_handle: tauri::AppHandle) -> Vec<db::TodoItem> {
    db::get_todos(&app_handle).unwrap_or_default()
}

#[tauri::command]
fn add_todo_item(app_handle: tauri::AppHandle, text: String) -> u32 {
    db::save_todo(&app_handle, text).unwrap_or(0)
}

#[tauri::command]
fn update_todo_status(app_handle: tauri::AppHandle, id: u32, completed: bool) {
    let _ = db::update_todo(&app_handle, id, completed);
}

#[tauri::command]
fn remove_todo_item(app_handle: tauri::AppHandle, id: u32) {
    let _ = db::delete_todo(&app_handle, id);
}

#[tauri::command]
fn move_todo_item(app_handle: tauri::AppHandle, id: u32, target_parent_id: Option<u32>, target_position: i32) {
    let _ = db::move_todo(&app_handle, id, target_parent_id, target_position);
}

#[tauri::command]
fn log_message(msg: String) {
    println!("[FRONTEND] {}", msg);
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            db::init_db(app.handle())?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            greet, 
            set_always_on_top, 
            close_window, 
            start_drag,
            load_note,
            save_note_content,
            load_todos,
            add_todo_item,
            update_todo_status,
            update_todo_status,
            remove_todo_item,
            remove_todo_item,
            move_todo_item,
            log_message
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
