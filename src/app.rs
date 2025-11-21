use leptos::*;
use leptos::ev::SubmitEvent;
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

#[component]
pub fn App() -> impl IntoView {
    let (pinned, set_pinned) = create_signal(false);
    let (content, set_content) = create_signal(String::new());
    let (editing, set_editing) = create_signal(true);
    let (todos, set_todos) = create_signal(Vec::<TodoItem>::new());
    let (mode, set_mode) = create_signal("note"); // "note" or "todo"

    // Load initial data
    create_effect(move |_| {
        spawn_local(async move {
            let saved_content: String = serde_wasm_bindgen::from_value(
                invoke("load_note", JsValue::NULL).await
            ).unwrap_or_default();
            set_content.set(saved_content);

            let saved_todos: Vec<TodoItem> = serde_wasm_bindgen::from_value(
                invoke("load_todos", JsValue::NULL).await
            ).unwrap_or_default();
            set_todos.set(saved_todos);
        });
    });

    let toggle_pin = move |_| {
        spawn_local(async move {
            let new_pinned = !pinned.get_untracked();
            let args = serde_wasm_bindgen::to_value(&SetAlwaysOnTopArgs { always_on_top: new_pinned }).unwrap();
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
                let args = serde_wasm_bindgen::to_value(&AddTodoArgs { text: text.clone() }).unwrap();
                let id: u32 = serde_wasm_bindgen::from_value(
                    invoke("add_todo_item", args).await
                ).unwrap_or(0);
                
                if id != 0 {
                    set_todos.update(|t| {
                        t.push(TodoItem {
                            id,
                            text: text.clone(),
                            completed: false,
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
                    let args = serde_wasm_bindgen::to_value(&UpdateTodoArgs { id, completed }).unwrap();
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
            // Header / Drag Region
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
                        title=move || if mode.get() == "note" { "Switch to Todo" } else { "Switch to Note" }
                    >
                        {move || if mode.get() == "note" { "📝" } else { "✅" }}
                    </button>
                    {move || if mode.get() == "note" {
                        view! {
                            <button 
                                on:click=toggle_edit
                                on:mousedown=move |ev| ev.stop_propagation()
                                class="p-1 rounded hover:bg-yellow-300 text-yellow-600 transition-colors"
                                title=move || if editing.get() { "View Mode" } else { "Edit Mode" }
                            >
                                {move || if editing.get() { "👁️" } else { "✏️" }}
                            </button>
                        }.into_view()
                    } else {
                        view! { <span class="w-6"></span> }.into_view()
                    }}
                    <button 
                        on:click=toggle_pin 
                        on:mousedown=move |ev| ev.stop_propagation()
                        class=move || format!("p-1 rounded hover:bg-yellow-300 transition-colors {}", if pinned.get() { "text-red-600" } else { "text-yellow-600" })
                        title="Toggle Always on Top"
                    >
                        {move || if pinned.get() { "📌" } else { "📍" }}
                    </button>
                    <button 
                        on:click=close 
                        on:mousedown=move |ev| ev.stop_propagation()
                        class="p-1 rounded hover:bg-red-400 hover:text-white text-yellow-600 transition-colors"
                        title="Close"
                    >
                        "✕"
                    </button>
                </div>
            </div>
            
            // Content
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
                        }.into_view()
                    } else {
                        view! {
                            <div 
                                class="prose prose-sm max-w-none text-gray-800 prose-p:my-1 prose-headings:my-2"
                                inner_html=render_markdown()
                            ></div>
                        }.into_view()
                    }
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
                            <ul class="flex-col gap-1 overflow-auto">
                                <For
                                    each=move || todos.get()
                                    key=|todo| todo.id
                                    children=move |todo| {
                                        let id = todo.id;
                                        view! {
                                            <li class="flex items-center gap-2 group hover:bg-yellow-200/50 p-1 rounded">
                                                <input 
                                                    type="checkbox" 
                                                    checked=todo.completed
                                                    on:change=move |_| toggle_todo(id)
                                                    class="cursor-pointer"
                                                />
                                                <span class=move || format!("flex-1 text-sm {}", if todo.completed { "line-through text-gray-500" } else { "text-gray-800" })>
                                                    {todo.text}
                                                </span>
                                                <button 
                                                    on:click=move |_| delete_todo(id)
                                                    class="text-red-400 hover:text-red-600 opacity-0 group-hover:opacity-100 transition-opacity"
                                                >
                                                    "×"
                                                </button>
                                            </li>
                                        }
                                    }
                                />
                            </ul>
                        </div>
                    }.into_view()
                }}
            </div>
        </main>
    }
}
