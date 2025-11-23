// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
use tauri::Manager;
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
    println!("[BACKEND] move_todo_item called: id={}, parent={:?}, pos={}", id, target_parent_id, target_position);
    match db::move_todo(&app_handle, id, target_parent_id, target_position) {
        Ok(_) => println!("[BACKEND] ✅ move_todo succeeded"),
        Err(e) => println!("[BACKEND] ❌ move_todo failed: {}", e),
    }
}

#[tauri::command]
fn log_message(msg: String) {
    println!("[FRONTEND] {}", msg);
}

#[tauri::command]
fn set_todo_count(app_handle: tauri::AppHandle, id: u32, count: Option<i32>) {
    let _ = db::set_todo_count(&app_handle, id, count);
}

#[tauri::command]
fn decrement_todo(app_handle: tauri::AppHandle, id: u32) {
    let _ = db::decrement_todo(&app_handle, id);
}

#[tauri::command]
fn reset_all_todos(app_handle: tauri::AppHandle) {
    let _ = db::reset_all_todos(&app_handle);
}

#[tauri::command]
fn save_window_state(
    app_handle: tauri::AppHandle,
    width: f64,
    height: f64,
    x: f64,
    y: f64,
    pinned: bool,
) {
    let _ = db::save_window_state(&app_handle, width, height, x, y, pinned);
}

#[tauri::command]
fn load_window_state(app_handle: tauri::AppHandle) -> Option<db::WindowState> {
    db::load_window_state(&app_handle).ok().flatten()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::Moved(_) | tauri::WindowEvent::Resized(_) = event {
                let win = window.clone();
                // Spawn a task to save state to avoid blocking the event loop
                // In a real app, you might want to debounce this
                std::thread::spawn(move || {
                    if let Ok(factor) = win.scale_factor() {
                        if let (Ok(pos), Ok(size)) = (win.outer_position(), win.inner_size()) {
                            let logical_pos = pos.to_logical::<f64>(factor);
                            let logical_size = size.to_logical::<f64>(factor);
                            // We need to get the pinned state too.
                            // Since we can't easily get it from the window struct directly without a getter (which exists but might not be exposed easily in all versions),
                            // we'll assume we just update x, y, width, height and keep pinned as is?
                            // Actually db::save_window_state overwrites everything.
                            // We should probably fetch the current pinned state from DB or just pass it if we can get it.
                            // window.is_always_on_top() is available?
                            // Let's check if we can get always_on_top state.
                            // If not, we might overwrite pinned with false if we don't know.
                            // Wait, db::save_window_state takes pinned.
                            // Let's try to read the current pinned state from the window if possible.
                            // window.is_always_on_top() -> Result<bool> (Tauri 2.0?)
                            // In Tauri 1.x it wasn't easily available.
                            // If we can't get it, we should modify db::save_window_state to allow partial updates or read-modify-write.
                            
                            // For now, let's try to get it.
                            // If not, we'll read from DB first.
                            let app_handle = win.app_handle();
                            let pinned = if let Ok(Some(state)) = db::load_window_state(app_handle) {
                                state.pinned
                            } else {
                                false
                            };

                            let _ = db::save_window_state(
                                app_handle,
                                logical_size.width,
                                logical_size.height,
                                logical_pos.x,
                                logical_pos.y,
                                pinned
                            );
                        }
                    }
                });
            }
        })
        .setup(|app| {
            db::init_db(app.handle())?;
            
            // Restore window state
            if let Some(window) = app.get_webview_window("main") {
                 if let Ok(Some(state)) = db::load_window_state(app.handle()) {
                     let _ = window.set_size(tauri::Size::Logical(tauri::LogicalSize { width: state.width, height: state.height }));
                     let _ = window.set_position(tauri::Position::Logical(tauri::LogicalPosition { x: state.x, y: state.y }));
                     let _ = window.set_always_on_top(state.pinned);
                 }
            }
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
            remove_todo_item,
            move_todo_item,
            log_message,
            set_todo_count,
            decrement_todo,
            reset_all_todos,
            save_window_state,
            load_window_state
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
