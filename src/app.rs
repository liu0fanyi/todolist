use leptos::ev::SubmitEvent;
use leptos::prelude::*;
use leptos::task::spawn_local;
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "core"])]
    async fn invoke(cmd: &str, args: JsValue) -> JsValue;
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SetAlwaysOnTopArgs {
    always_on_top: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TodoItem {
    pub id: u32,
    pub text: String,
    pub completed: bool,
    pub parent_id: Option<u32>,
    pub position: i32,
    pub target_count: Option<i32>,
    pub current_count: i32,
}

#[derive(Serialize, Deserialize)]
struct SaveNoteArgs {
    content: String,
}

#[derive(Serialize, Deserialize)]
struct AddTodoArgs {
    text: String,
}

#[derive(Serialize, Deserialize)]
struct UpdateTodoArgs {
    id: u32,
    completed: bool,
}

#[derive(Serialize, Deserialize)]
struct RemoveTodoArgs {
    id: u32,
}

#[derive(Serialize, Deserialize)]
struct SetTodoCountArgs {
    id: u32,
    count: Option<i32>,
}

#[derive(Serialize, Deserialize)]
struct DecrementTodoArgs {
    id: u32,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MoveTodoArgs {
    id: u32,
    target_parent_id: Option<u32>,
    target_position: i32,
}

#[derive(Serialize, Deserialize)]
struct LogArgs {
    msg: String,
}

#[derive(Serialize, Deserialize)]
struct ResetAllArgs {}

#[component]
pub fn App() -> impl IntoView {
    let (pinned, set_pinned) = create_signal(false);
    let (content, set_content) = create_signal(String::new());
    let (editing, set_editing) = create_signal(true);
    let (todos, set_todos) = create_signal(Vec::<TodoItem>::new());
    let (mode, set_mode) = create_signal("todo");
    
    // Global drag state
    let (dragging_id, set_dragging_id) = create_signal(None::<u32>);
    let (drop_target_id, set_drop_target_id) = create_signal(None::<u32>);
    let (drop_position, set_drop_position) = create_signal(0.5); // 0.0-1.0 for position detection

    let log = move |msg: String| {
        spawn_local(async move {
            let args = serde_wasm_bindgen::to_value(&LogArgs { msg }).unwrap();
            invoke("log_message", args).await;
        });
    };

    // Load initial data
    Effect::new(move |_| {
        spawn_local(async move {
            let saved_content: String =
                serde_wasm_bindgen::from_value(invoke("load_note", JsValue::NULL).await)
                    .unwrap_or_default();
            set_content.set(saved_content);

            let saved_todos: Vec<TodoItem> =
                serde_wasm_bindgen::from_value(invoke("load_todos", JsValue::NULL).await)
                    .unwrap_or_default();
            set_todos.set(saved_todos);
        });
    });

    let toggle_pin = move |_| {
        spawn_local(async move {
            let new_pinned = !pinned.get_untracked();
            let args = serde_wasm_bindgen::to_value(&SetAlwaysOnTopArgs {
                always_on_top: new_pinned,
            })
            .unwrap();
            invoke("set_always_on_top", args).await;
            set_pinned.set(new_pinned);
        });
    };

    let close = move |_| {
        spawn_local(async move {
            invoke("close_window", JsValue::NULL).await;
        });
    };

    /*
    let toggle_edit = move |_| {
        set_editing.update(|e| *e = !*e);
    };
    */

    // Global mouseup handler for drag and drop
    Effect::new(move |_| {
        let window = web_sys::window().unwrap();
        let log_clone = log.clone();
        
        let on_mouseup = Closure::<dyn FnMut(_)>::new(move |_ev: web_sys::MouseEvent| {
            log_clone(format!("🔵 Global mouseup triggered"));
            
            if let Some(dragged_id) = dragging_id.get_untracked() {
                log_clone(format!("🔵 Dragging ID: {}", dragged_id));
                
                if let Some(target_id) = drop_target_id.get_untracked() {
                    log_clone(format!("🔵 Drop target ID: {}", target_id));
                    
                    if dragged_id != target_id {
                        log_clone(format!("🟢 Drop {} on {}", dragged_id, target_id));
                        
                        // Get target todo info
                        let pos: f64 = drop_position.get_untracked();
                        let pos = pos.max(0.0).min(1.0); // Clamp to 0-1
                        log_clone(format!("🔵 Drop position: {:.2}", pos));
                        
                        let current_todos = todos.get_untracked();
                        
                        // Log all todos for better debugging
                        log_clone(format!("📋 All todos: {}", current_todos.iter()
                            .map(|t| format!("id={} '{}' p={:?} pos={}", t.id, t.text, t.parent_id, t.position))
                            .collect::<Vec<_>>()
                            .join(", ")));
                        
                        let (final_parent, mut final_pos) = if let Some(target_todo) = current_todos.iter().find(|t| t.id == target_id) {
                             log_clone(format!("📋 Target todo: id={} '{}' parent={:?} pos={}", target_todo.id, target_todo.text, target_todo.parent_id, target_todo.position));
                             
                             // Check if target is a descendant of dragged item (would create a cycle)
                             // This must be checked BEFORE any position calculation
                             let is_descendant = {
                                 let mut check_id = Some(target_id);
                                 let mut found = false;
                                 while let Some(current_id) = check_id {
                                     if current_id == dragged_id {
                                         found = true;
                                         break;
                                     }
                                     check_id = current_todos.iter()
                                         .find(|t| t.id == current_id)
                                         .and_then(|t| t.parent_id);
                                 }
                                 found
                             };
                             
                             if is_descendant {
                                 log_clone(format!("⚠️ Cannot drop parent into/near its own child/descendant, skipping"));
                                 set_dragging_id.set(None);
                                 set_drop_target_id.set(None);
                                 return;
                             }
                             
                             let target_parent_id = target_todo.parent_id;
                             let target_position = target_todo.position;
                             
                             let pos: f64 = drop_position.get_untracked();
                             let pos = pos.max(0.0).min(1.0);
                             
                             if pos < 0.25 {
                                 log_clone(format!("📍 Dropping BEFORE (parent: {:?}, pos: {})", target_parent_id, target_position));
                                 (target_parent_id, target_position)
                             } else if pos > 0.75 {
                                 log_clone(format!("📍 Dropping AFTER (parent: {:?}, pos: {})", target_parent_id, target_position + 1));
                                 (target_parent_id, target_position + 1)
                             } else {
                                 log_clone(format!("📍 Dropping as CHILD (parent: {}, pos: 0)", target_id));
                                 (Some(target_id), 0)
                             }
                        } else {
                             log_clone(format!("❌ Target todo not found!"));
                             return;
                        };
                            
                            // Check if source and target are the same
                            if let Some(dragged_todo) = current_todos.iter().find(|t| t.id == dragged_id) {
                                log_clone(format!("📋 Dragged todo: id={} '{}' parent={:?} pos={}", dragged_todo.id, dragged_todo.text, dragged_todo.parent_id, dragged_todo.position));
                                log_clone(format!("📋 Target: parent={:?}, pos={}", final_parent, final_pos));
                                
                                if dragged_todo.parent_id == final_parent && dragged_todo.position == final_pos {
                                    log_clone(format!("⚠️ Source and target are the same, skipping"));
                                    set_dragging_id.set(None);
                                    set_drop_target_id.set(None);
                                    return;
                                }

                                // Adjust position if dragging downwards in the same list
                                // When we remove the item, indices shift, so we need to decrement the target position
                                if dragged_todo.parent_id == final_parent && dragged_todo.position < final_pos {
                                    log_clone(format!("⬇️ Dragging down in same list, adjusting pos from {} to {}", final_pos, final_pos - 1));
                                    final_pos -= 1;
                                }
                            }
                            
                            // Log all todos for debugging
                            log_clone(format!("📋 All todos: {}", current_todos.iter()
                                .map(|t| format!("id={} p={:?} pos={}", t.id, t.parent_id, t.position))
                                .collect::<Vec<_>>()
                                .join(", ")));
                            
                            log_clone(format!("🚀 Calling move_todo_item..."));
                            
                            // Call backend
                            let log_async = log_clone.clone();
                            spawn_local(async move {
                                let args = serde_wasm_bindgen::to_value(&MoveTodoArgs {
                                    id: dragged_id,
                                    target_parent_id: final_parent,
                                    target_position: final_pos
                                }).unwrap();
                                
                                log_async(format!("📤 Invoking backend with id={}, parent={:?}, pos={}", dragged_id, final_parent, final_pos));
                                
                                // Log to browser console for debugging
                                web_sys::console::log_1(&JsValue::from_str(&format!("[JS] Calling move_todo_item with id={}, parent={:?}, pos={}", dragged_id, final_parent, final_pos)));
                                
                                // Call backend with error handling
                                let result = invoke("move_todo_item", args).await;
                                
                                web_sys::console::log_2(&JsValue::from_str("[JS] invoke returned:"), &result);
                                
                                // Check if there was an error
                                if result.is_undefined() || result.is_null() {
                                    log_async(format!("✅ Backend call complete (void return)"));
                                } else {
                                    log_async(format!("✅ Backend call complete: {:?}", result));
                                }
                                
                                // Reload todos
                                log_async(format!("🔄 Reloading todos..."));
                                let saved_todos: Vec<TodoItem> = serde_wasm_bindgen::from_value(
                                    invoke("load_todos", JsValue::NULL).await
                                ).unwrap_or_default();
                                let count = saved_todos.len();
                                set_todos.set(saved_todos);
                                log_async(format!("✅ Todos reloaded, count: {}", count));
                            });

                    } else {
                        log_clone(format!("⚠️ Dragging onto self, ignoring"));
                    }
                } else {
                    log_clone(format!("⚠️ No drop target"));
                }
                // Clear drag state
                set_dragging_id.set(None);
                set_drop_target_id.set(None);
            } else {
                log_clone(format!("⚠️ No dragging ID"));
            }
        });
        
        let _ = window.add_event_listener_with_callback("mouseup", on_mouseup.as_ref().unchecked_ref());
        on_mouseup.forget();
    });

    /*
    let toggle_mode = move |_| {
        set_mode.update(|m| *m = if *m == "note" { "todo" } else { "note" });
    };
    */

    let update_note = move |ev| {
        let val = event_target_value(&ev);
        set_content.set(val.clone());
        spawn_local(async move {
            let args = serde_wasm_bindgen::to_value(&SaveNoteArgs { content: val }).unwrap();
            invoke("save_note_content", args).await;
        });
    };

    let add_todo = move |ev: SubmitEvent| {
        ev.prevent_default();
        let input = event_target::<web_sys::HtmlFormElement>(&ev)
            .elements()
            .named_item("todo-input")
            .unwrap()
            .dyn_into::<web_sys::HtmlInputElement>()
            .unwrap();
        let text = input.value();
        if !text.is_empty() {
            spawn_local(async move {
                let args =
                    serde_wasm_bindgen::to_value(&AddTodoArgs { text: text.clone() }).unwrap();
                let id: u32 = serde_wasm_bindgen::from_value(invoke("add_todo_item", args).await)
                    .unwrap_or(0);

                if id != 0 {
                    set_todos.update(|t| {
                        t.push(TodoItem {
                            id,
                            text: text.clone(),
                            completed: false,
                            parent_id: None,
                            position: 0,
                            target_count: None,
                            current_count: 0,
                        })
                    });
                }
            });
            input.set_value("");
        }
    };

    let toggle_todo = move |id: u32| {
        // Optimistic update
        set_todos.update(|t| {
            if let Some(item) = t.iter_mut().find(|i| i.id == id) {
                item.completed = !item.completed;
            }
        });

        spawn_local(async move {
            // Let's re-read the item to get the intended state
            let completed = todos.get_untracked().iter().find(|i| i.id == id).map(|i| i.completed).unwrap_or(false);
            
            log(format!("🔄 Toggling todo {} to {}", id, completed));

            let args = serde_wasm_bindgen::to_value(&UpdateTodoArgs { id, completed }).unwrap();
            invoke("update_todo_status", args).await;
            
            // Reload todos to get cascading updates
            log(format!("🔄 Reloading todos after toggle..."));
            let saved_todos: Vec<TodoItem> = serde_wasm_bindgen::from_value(
                invoke("load_todos", JsValue::NULL).await
            ).unwrap_or_default();
            set_todos.set(saved_todos);
            log(format!("✅ Todos reloaded after toggle"));
        });
    };

    let delete_todo = move |id: u32| {
        set_todos.update(|t| t.retain(|i| i.id != id));
        spawn_local(async move {
            let args = serde_wasm_bindgen::to_value(&RemoveTodoArgs { id }).unwrap();
            invoke("remove_todo_item", args).await;
        });
    };

    let start_drag = move |ev: web_sys::MouseEvent| {
        if pinned.get_untracked() {
            return;
        }
        if ev.buttons() == 1 {
            spawn_local(async move {
                invoke("start_drag", JsValue::NULL).await;
            });
        }
    };

    let render_markdown = move || {
        let markdown_input = content.get();
        let parser = pulldown_cmark::Parser::new(&markdown_input);
        let mut html_output = String::new();
        pulldown_cmark::html::push_html(&mut html_output, parser);
        html_output
    };

    let set_todo_count = move |id: u32, count: Option<i32>| {
        spawn_local(async move {
            let args = serde_wasm_bindgen::to_value(&SetTodoCountArgs { id, count }).unwrap();
            invoke("set_todo_count", args).await;
            // Reload todos
            let saved_todos: Vec<TodoItem> = serde_wasm_bindgen::from_value(
                invoke("load_todos", JsValue::NULL).await
            ).unwrap_or_default();
            set_todos.set(saved_todos);
        });
    };

    let decrement_todo = move |id: u32| {
        spawn_local(async move {
            let args = serde_wasm_bindgen::to_value(&DecrementTodoArgs { id }).unwrap();
            invoke("decrement_todo", args).await;
            // Reload todos
            let saved_todos: Vec<TodoItem> = serde_wasm_bindgen::from_value(
                invoke("load_todos", JsValue::NULL).await
            ).unwrap_or_default();
            set_todos.set(saved_todos);
        });
    };

    let reset_all_todos = move |_| {
        spawn_local(async move {
            // Call backend to reset all todos
            invoke("reset_all_todos", JsValue::NULL).await;
            // Reload todos
            let saved_todos: Vec<TodoItem> = serde_wasm_bindgen::from_value(
                invoke("load_todos", JsValue::NULL).await
            ).unwrap_or_default();
            set_todos.set(saved_todos);
        });
    };



    view! {
        <main class="h-screen w-screen bg-yellow-100 flex flex-col overflow-hidden rounded-lg shadow-lg border border-yellow-300">
            <div
                class="h-8 bg-yellow-200 flex justify-between items-center px-2 cursor-move select-none"
                on:mousedown=start_drag
            >
                <span class="text-xs text-yellow-800 font-bold pointer-events-none">"TodoList"</span>
                <div class="flex gap-1">
                    <button
                        on:click=reset_all_todos
                        on:mousedown=move |ev| ev.stop_propagation()
                        class="px-2 py-0.5 text-xs rounded hover:bg-yellow-300 text-yellow-700 transition-colors"
                        title="Reset all items to incomplete"
                    >
                        "↻"
                    </button>
                    /*
                    <button
                        on:click=toggle_mode
                        on:mousedown=move |ev| ev.stop_propagation()
                        class="p-1 rounded hover:bg-yellow-300 text-yellow-600 transition-colors text-xs"
                    >
                        {move || if mode.get() == "note" { "📝" } else { "✅" }}
                    </button>
                    {move || if mode.get() == "note" {
                        view! {
                            <button
                                on:click=toggle_edit
                                on:mousedown=move |ev| ev.stop_propagation()
                                class="p-1 rounded hover:bg-yellow-300 text-yellow-600 transition-colors"
                            >
                                {move || if editing.get() { "👁️" } else { "✏️" }}
                            </button>
                        }.into_any()
                    } else {
                        view! { <span class="w-6"></span> }.into_any()
                    }}
                    */
                    <button
                        on:click=toggle_pin
                        on:mousedown=move |ev| ev.stop_propagation()
                        class=move || format!("p-1 rounded hover:bg-yellow-300 transition-colors {}", if pinned.get() { "text-red-600" } else { "text-yellow-600" })
                    >
                        {move || if pinned.get() { "📌" } else { "📍" }}
                    </button>
                    <button
                        on:click=close
                        on:mousedown=move |ev| ev.stop_propagation()
                        class="p-1 rounded hover:bg-red-400 hover:text-white text-yellow-600 transition-colors"
                    >
                        "✕"
                    </button>
                </div>
            </div>

            <div class="flex-1 p-2 overflow-auto">
                {move || if mode.get() == "note" {
                    if editing.get() {
                        view! {
                            <textarea
                                class="w-full h-full bg-transparent resize-none outline-none text-gray-800 font-sans text-sm"
                                placeholder="Type your note here..."
                                on:input=update_note
                                prop:value=content
                            ></textarea>
                        }.into_any()
                    } else {
                        view! {
                            <div
                                class="prose prose-sm max-w-none text-gray-800 prose-p:my-1 prose-headings:my-2"
                                inner_html=render_markdown()
                            ></div>
                        }.into_any()
                    }.into_any()
                } else {
                    view! {
                        <div class="flex flex-col h-full">
                            <form on:submit=add_todo class="flex gap-2 mb-2">
                                <input
                                    name="todo-input"
                                    class="flex-1 bg-white/50 border-none rounded px-2 py-1 text-sm outline-none focus:bg-white"
                                    placeholder="Add todo..."
                                    autocomplete="off"
                                />
                                <button type="submit" class="text-green-600 hover:text-green-700 font-bold">"+"</button>
                            </form>
                            <div class="flex-col gap-1 overflow-auto">
                                <TodoList
                                    todos=todos.into()
                                    parent_id=None
                                    toggle_todo=toggle_todo
                                    delete_todo=delete_todo
                                    log=log
                                    on_drop=move |dragged_id, target_parent_id, target_pos| {
                                        log(format!("Dropped {} -> {:?}", dragged_id, target_parent_id));
                                        spawn_local(async move {
                                            let args = serde_wasm_bindgen::to_value(&MoveTodoArgs {
                                                id: dragged_id,
                                                target_parent_id,
                                                target_position: target_pos
                                            }).unwrap();
                                            invoke("move_todo_item", args).await;
                                            let saved_todos: Vec<TodoItem> = serde_wasm_bindgen::from_value(
                                                invoke("load_todos", JsValue::NULL).await
                                            ).unwrap_or_default();
                                            set_todos.set(saved_todos);
                                        });
                                    }
                                    dragging_id=dragging_id
                                    set_dragging_id=set_dragging_id
                                    drop_target_id=drop_target_id
                                    set_drop_target_id=set_drop_target_id
                                    drop_position=drop_position
                                    set_drop_position=set_drop_position
                                    set_todo_count=set_todo_count
                                    decrement_todo=decrement_todo
                                />

                            </div>
                        </div>
                    }.into_any()
                }}
            </div>
        </main>
    }
}

#[component]
fn TodoList<F1, F2, F3, F4, F5, F6>(
    todos: Signal<Vec<TodoItem>>,
    parent_id: Option<u32>,
    toggle_todo: F1,
    delete_todo: F2,
    log: F4,
    on_drop: F3,
    dragging_id: ReadSignal<Option<u32>>,
    set_dragging_id: WriteSignal<Option<u32>>,
    drop_target_id: ReadSignal<Option<u32>>,
    set_drop_target_id: WriteSignal<Option<u32>>,
    drop_position: ReadSignal<f64>,
    set_drop_position: WriteSignal<f64>,
    set_todo_count: F5,
    decrement_todo: F6,
) -> impl IntoView
where
    F1: Fn(u32) + Clone + Send + 'static,
    F2: Fn(u32) + Clone + Send + 'static,
    F3: Fn(u32, Option<u32>, i32) + Clone + Send + 'static,
    F4: Fn(String) + Clone + Send + 'static,
    F5: Fn(u32, Option<i32>) + Clone + Send + 'static,
    F6: Fn(u32) + Clone + Send + 'static,
{

    view! {
        <ul class="flex flex-col gap-2 pl-4 border-l-2 border-gray-100">
            <For
                each=move || {
                    todos.get()
                        .into_iter()
                        .filter(|t| t.parent_id == parent_id)
                        .collect::<Vec<_>>()
                }
                key=|todo| todo.id
                children=move |todo| {
                    view! {
                        <TodoItemView 
                            todo=todo 
                            all_todos=todos
                            toggle_todo=toggle_todo.clone() 
                            delete_todo=delete_todo.clone() 
                            log=log.clone() 
                            on_drop=on_drop.clone()
                            dragging_id=dragging_id
                            set_dragging_id=set_dragging_id
                            drop_target_id=drop_target_id
                            set_drop_target_id=set_drop_target_id
                            drop_position=drop_position
                            set_drop_position=set_drop_position
                            set_todo_count=set_todo_count.clone()
                            decrement_todo=decrement_todo.clone()
                        />
                    }
                }
            />
        </ul>
    }
}

#[component]
fn TodoItemView<F1, F2, F3, F4, F5, F6>(
    todo: TodoItem,
    all_todos: Signal<Vec<TodoItem>>,
    toggle_todo: F1,
    delete_todo: F2,
    log: F4,
    on_drop: F3,
    dragging_id: ReadSignal<Option<u32>>,
    set_dragging_id: WriteSignal<Option<u32>>,
    drop_target_id: ReadSignal<Option<u32>>,
    set_drop_target_id: WriteSignal<Option<u32>>,
    drop_position: ReadSignal<f64>,
    set_drop_position: WriteSignal<f64>,
    set_todo_count: F5,
    decrement_todo: F6,
) -> AnyView
where
    F1: Fn(u32) + Clone + Send + 'static,
    F2: Fn(u32) + Clone + Send + 'static,
    F3: Fn(u32, Option<u32>, i32) + Clone + Send + 'static,
    F4: Fn(String) + Clone + Send + 'static,
    F5: Fn(u32, Option<i32>) + Clone + Send + 'static,
    F6: Fn(u32) + Clone + Send + 'static,
{
    let id = todo.id;
    
    // Create a derived signal for the current todo to ensure reactivity
    // This fixes the issue where the component doesn't update when the parent list changes
    let current_todo = create_memo(move |_| {
        all_todos.get()
            .into_iter()
            .find(|t| t.id == id)
            .unwrap_or(todo.clone())
    });

    // Mouse down - start drag
    let on_mousedown = {
        let log = log.clone();
        move |ev: web_sys::MouseEvent| {
            if ev.button() == 0 { // Left click only
                set_dragging_id.set(Some(id));
                log(format!("Start dragging: {}", id));
                ev.prevent_default();
                ev.stop_propagation();
            }
        }
    };

    // Mouse enter - track potential drop target
    let update_position = move |ev: &web_sys::MouseEvent| {
        if dragging_id.get_untracked().is_some() {
            set_drop_target_id.set(Some(id));
            
            // Calculate relative position (0.0 = top, 1.0 = bottom)
            if let Some(target) = ev.current_target() {
                if let Some(element) = target.dyn_ref::<web_sys::HtmlElement>() {
                    let rect = element.get_bounding_client_rect();
                    let y = ev.client_y() as f64;
                    let top = rect.top();
                    let height = rect.height();
                    
                    if height > 0.0 {
                        let relative_y = ((y - top) / height).max(0.0).min(1.0);
                        set_drop_position.set(relative_y);
                    }
                }
            }
        }
    };

    let on_mouseenter = {
        let update_position = update_position.clone();
        move |ev: web_sys::MouseEvent| {
            update_position(&ev);
        }
    };

    let on_mousemove = {
        let update_position = update_position.clone();
        move |ev: web_sys::MouseEvent| {
            update_position(&ev);
            ev.stop_propagation();
        }
    };

    // Visual feedback based on drag state
    let item_class = move || {
        let mut classes = vec![
            "flex flex-col p-2 rounded shadow-sm border transition-all duration-200 select-none".to_string(),
            "bg-white".to_string(),
        ];

        if dragging_id.get() == Some(id) {
            classes.push("opacity-50 scale-95 ring-2 ring-blue-400".to_string());
        }

        if drop_target_id.get() == Some(id) {
            let pos = drop_position.get();
            if pos < 0.25 {
                // Dropping BEFORE - blue top border
                classes.push("border-t-4".to_string());
                classes.push("border-blue-500".to_string());
            } else if pos > 0.75 {
                // Dropping AFTER - blue bottom border
                classes.push("border-b-4".to_string());
                classes.push("border-blue-500".to_string());
            } else {
                // Dropping as CHILD - amber/yellow background with ring for visibility
                classes.push("bg-amber-100".to_string());
                classes.push("ring-2".to_string());
                classes.push("ring-amber-400".to_string());
                classes.push("border-amber-300".to_string());
            }
        } else {
            classes.push("hover:bg-yellow-50".to_string());
        }
        
        classes.join(" ")
    };

    view! {
        <li 
            class=item_class
            on:mousedown=on_mousedown
            on:mouseenter=on_mouseenter
            on:mousemove=on_mousemove
        >
            <div class="flex items-center gap-2 select-none">
                <span class="text-gray-400 cursor-grab">"⠿"</span>
                
                {
                    let toggle_todo = toggle_todo.clone();
                    let decrement_todo = decrement_todo.clone();
                    move || {
                    let t = current_todo.get();
                    let is_countdown = t.target_count.unwrap_or(0) > 0;
                    
                    if is_countdown && !t.completed {
                        view! {
                            <button
                                class="w-6 h-6 flex items-center justify-center bg-blue-100 text-blue-600 rounded-full text-xs font-bold hover:bg-blue-200 transition-colors"
                                on:click={
                                    let dec = decrement_todo.clone();
                                    move |ev| {
                                        ev.stop_propagation();
                                        dec(id);
                                    }
                                }
                                on:mousedown=move |ev| ev.stop_propagation()
                            >
                                {t.current_count}
                            </button>
                        }.into_any()
                    } else {
                        view! {
                            <input 
                                type="checkbox" 
                                prop:checked=move || current_todo.get().completed 
                                on:change={
                                    let toggle = toggle_todo.clone();
                                    move |_| toggle(id)
                                }
                                on:mousedown=move |ev| ev.stop_propagation()
                                class="cursor-pointer" 
                            />
                        }.into_any()
                    }
                }}

                <span class=move || format!(
                    "flex-1 text-sm {}", 
                    if current_todo.get().completed { "line-through text-gray-500" } else { "text-gray-800" }
                )>
                    {move || current_todo.get().text}
                </span>

                <input
                    type="number"
                    class="w-12 p-1 text-xs border rounded text-center text-gray-500"
                    placeholder="#"
                    prop:value=move || current_todo.get().target_count.map(|c| c.to_string()).unwrap_or_default()
                    on:change={
                        let set_count = set_todo_count.clone();
                        move |ev| {
                            let input = event_target::<web_sys::HtmlInputElement>(&ev);
                            let val = input.value();
                            let count = val.parse::<i32>().ok().filter(|&c| c > 0);
                            set_count(id, count);
                        }
                    }
                    on:mousedown=move |ev| ev.stop_propagation()
                    on:click=move |ev| ev.stop_propagation()
                />

                <button 
                    on:click={
                        let del = delete_todo.clone();
                        move |_| del(id)
                    }
                    class="text-red-400 hover:text-red-600 text-xs"
                    on:mousedown=move |ev| ev.stop_propagation()
                >"×"</button>
            </div>
            <TodoList 
                todos=all_todos 
                parent_id=Some(id) 
                toggle_todo=toggle_todo 
                delete_todo=delete_todo 
                log=log 
                on_drop=on_drop
                dragging_id=dragging_id
                set_dragging_id=set_dragging_id
                drop_target_id=drop_target_id
                set_drop_target_id=set_drop_target_id
                drop_position=drop_position
                set_drop_position=set_drop_position
                set_todo_count=set_todo_count
                decrement_todo=decrement_todo
            />
        </li>
    }.into_any()
}
