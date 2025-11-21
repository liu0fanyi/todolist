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

#[derive(Clone, Debug, Serialize, Deserialize)]
struct TodoItem {
    id: u32,
    text: String,
    completed: bool,
    parent_id: Option<u32>,
    position: i32,
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
struct MoveTodoArgs {
    id: u32,
    target_parent_id: Option<u32>,
    target_position: i32,
}

#[derive(Serialize, Deserialize)]
struct LogArgs {
    msg: String,
}

#[component]
pub fn App() -> impl IntoView {
    let (pinned, set_pinned) = create_signal(false);
    let (content, set_content) = create_signal(String::new());
    let (editing, set_editing) = create_signal(true);
    let (todos, set_todos) = create_signal(Vec::<TodoItem>::new());
    let (mode, set_mode) = create_signal("note");
    
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

    let toggle_edit = move |_| {
        set_editing.update(|e| *e = !*e);
    };

    // Global mouseup handler for drag and drop
    Effect::new(move |_| {
        let window = web_sys::window().unwrap();
        let log_clone = log.clone();
        
        let on_mouseup = Closure::<dyn FnMut(_)>::new(move |_ev: web_sys::MouseEvent| {
            if let Some(dragged_id) = dragging_id.get() {
                if let Some(target_id) = drop_target_id.get() {
                    if dragged_id != target_id {
                        log_clone(format!("Drop {} on {}", dragged_id, target_id));
                        
                        // Get target todo info
                        let pos = drop_position.get();
                        let current_todos = todos.get();
                        
                        if let Some(target_todo) = current_todos.iter().find(|t| t.id == target_id) {
                            let target_parent_id = target_todo.parent_id;
                            let target_position = target_todo.position;
                            
                            // Determine drop type based on position
                            let (final_parent, final_pos) = if pos < 0.25 {
                                // Drop before
                                (target_parent_id, target_position)
                            } else if pos > 0.75 {
                                // Drop after
                                (target_parent_id, target_position + 1)
                            } else {
                                // Nest as child
                                (Some(target_id), 0)
                            };
                            
                            // Call backend
                            spawn_local(async move {
                                let args = serde_wasm_bindgen::to_value(&MoveTodoArgs {
                                    id: dragged_id,
                                    target_parent_id: final_parent,
                                    target_position: final_pos
                                }).unwrap();
                                invoke("move_todo_item", args).await;
                                
                                // Reload todos
                                let saved_todos: Vec<TodoItem> = serde_wasm_bindgen::from_value(
                                    invoke("load_todos", JsValue::NULL).await
                                ).unwrap_or_default();
                                set_todos.set(saved_todos);
                            });
                        }
                    }
                }
                // Clear drag state
                set_dragging_id.set(None);
                set_drop_target_id.set(None);
            }
        });
        
        let _ = window.add_event_listener_with_callback("mouseup", on_mouseup.as_ref().unchecked_ref());
        on_mouseup.forget();
    });

    let toggle_mode = move |_| {
        set_mode.update(|m| *m = if *m == "note" { "todo" } else { "note" });
    };

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
                        })
                    });
                }
            });
            input.set_value("");
        }
    };

    let toggle_todo = move |id: u32| {
        set_todos.update(|t| {
            if let Some(item) = t.iter_mut().find(|i| i.id == id) {
                item.completed = !item.completed;
                let completed = item.completed;
                spawn_local(async move {
                    let args =
                        serde_wasm_bindgen::to_value(&UpdateTodoArgs { id, completed }).unwrap();
                    invoke("update_todo_status", args).await;
                });
            }
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

    view! {
        <main class="h-screen w-screen bg-yellow-100 flex flex-col overflow-hidden rounded-lg shadow-lg border border-yellow-300">
            <div
                class="h-8 bg-yellow-200 flex justify-between items-center px-2 cursor-move select-none"
                on:mousedown=start_drag
            >
                <span class="text-xs text-yellow-800 font-bold pointer-events-none">"Sticky Note"</span>
                <div class="flex gap-1">
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
fn TodoList<F1, F2, F3, F4>(
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
) -> AnyView
where
    F1: Fn(u32) + Clone + Send + 'static,
    F2: Fn(u32) + Clone + Send + 'static,
    F3: Fn(u32, Option<u32>, i32) + Clone + Send + 'static,
    F4: Fn(String) + Clone + Send + 'static,
{
    let filtered_todos = move || {
        let current_todos = todos.get();
        let mut items: Vec<TodoItem> = current_todos
            .into_iter()
            .filter(|t| t.parent_id == parent_id)
            .collect();
        items.sort_by_key(|t| t.position);
        items
    };

    view! {
        <ul class="pl-4">
            <For
                each=filtered_todos
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
                        />
                    }
                }
            />
        </ul>
    }.into_any()
}

#[component]
fn TodoItemView<F1, F2, F3, F4>(
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
) -> AnyView
where
    F1: Fn(u32) + Clone + Send + 'static,
    F2: Fn(u32) + Clone + Send + 'static,
    F3: Fn(u32, Option<u32>, i32) + Clone + Send + 'static,
    F4: Fn(String) + Clone + Send + 'static,
{
    let id = todo.id;
    let parent_id = todo.parent_id;
    let position = todo.position;

    // Mouse down - start drag
    let on_mousedown = {
        let log = log.clone();
        move |ev: web_sys::MouseEvent| {
            if ev.button() == 0 { // Left click only
                set_dragging_id.set(Some(id));
                log(format!("Start dragging: {}", id));
                ev.prevent_default();
            }
        }
    };

    // Mouse enter - track potential drop target
    let on_mouseenter = move |ev: web_sys::MouseEvent| {
        if dragging_id.get().is_some() {
            set_drop_target_id.set(Some(id));
            
            // Calculate relative position (0.0 to 1.0)
            if let Some(target) = ev.current_target() {
                if let Some(element) = target.dyn_ref::<web_sys::HtmlElement>() {
                    let rect = element.get_bounding_client_rect();
                    let y = ev.client_y() as f64;
                    let rel_y = (y - rect.top()) / rect.height();
                    set_drop_position.set(rel_y);
                }
            }
        }
    };

    // Visual feedback based on drag state
    let item_class = move || {
        let is_dragging = dragging_id.get() == Some(id);
        let is_drop_target = drop_target_id.get() == Some(id) && dragging_id.get().is_some();
        
        let mut classes = vec!["flex", "flex-col", "gap-1", "p-2", "rounded", "transition-all"];
        
        if is_dragging {
            classes.push("opacity-50");
            classes.push("cursor-grabbing");
        } else if is_drop_target {
            let pos = drop_position.get();
            if pos < 0.25 {
                classes.push("border-t-4");
                classes.push("border-blue-500");
            } else if pos > 0.75 {
                classes.push("border-b-4");
                classes.push("border-blue-500");
            } else {
                classes.push("bg-blue-50");
                classes.push("ring-2");
                classes.push("ring-blue-500");
            }
        } else {
            classes.push("hover:bg-yellow-50");
        }
        
        classes.join(" ")
    };

    view! {
        <li 
            class=item_class
            on:mousedown=on_mousedown
            on:mouseenter=on_mouseenter
        >
            <div class="flex items-center gap-2 select-none">
                <span class="text-gray-400 cursor-grab">"⠿"</span>
                <input 
                    type="checkbox" 
                    checked=todo.completed 
                    on:change={
                        let toggle = toggle_todo.clone();
                        move |_| toggle(id)
                    }
                    class="cursor-pointer" 
                />
                <span class=move || format!(
                    "flex-1 text-sm {}",
                    if todo.completed { "line-through text-gray-500" } else { "text-gray-800" }
                )>
                    {todo.text.clone()}
                </span>
                <button 
                    on:click={
                        let del = delete_todo.clone();
                        move |_| del(id)
                    }
                    class="text-red-400 hover:text-red-600 text-xs"
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
            />
        </li>
    }.into_any()
}
