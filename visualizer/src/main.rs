#![recursion_limit = "1024"]

mod access_info;
mod home;
mod workload_editor;
mod workload_selector;

use access_info::AccessInfo;
use console_error_panic_hook::set_once as set_panic_hook;
use home::Home;
use shipyard::scheduler::info::WorkloadsInfo;
use std::collections::HashMap;
use wasm_bindgen::{prelude::Closure, JsCast};
use web_sys::FileReader;
use workload_editor::WorkloadEditor;
use workload_selector::WorkloadSelector;
use yew::prelude::*;

struct App {
    page: Page,
    selected_workload: Option<String>,
    workloads: WorkloadsInfo,
}

enum Msg {
    GoToPage(Page),
    SetWorkloads(WorkloadsInfo),
    ChangeSelectedWorkload(Option<String>),
}

impl Component for App {
    type Message = Msg;
    type Properties = ();

    fn create(_ctx: &Context<App>) -> App {
        App {
            page: Page::Home,
            selected_workload: None,
            workloads: WorkloadsInfo::new(),
        }
    }

    fn update(&mut self, _ctx: &Context<App>, msg: Msg) -> bool {
        match msg {
            Msg::GoToPage(page) => {
                if self.page == page {
                    return false;
                }

                self.page = page;
            }
            Msg::SetWorkloads(workloads) => {
                self.selected_workload = None;
                self.page = Page::AccessInfo;

                if self.workloads == workloads {
                    return false;
                }

                self.workloads = workloads;
            }
            Msg::ChangeSelectedWorkload(workload) => {
                if self.selected_workload == workload {
                    return false;
                }

                self.selected_workload = workload;
            }
        }

        true
    }

    fn view(&self, ctx: &Context<App>) -> Html {
        let link = ctx.link().clone();
        let on_click_title = move |_| link.send_message(Msg::GoToPage(Page::Home));
        let link = ctx.link().clone();
        let change_page = move |page| link.send_message(Msg::GoToPage(page));

        let link = ctx.link().clone();
        let set_workloads = move |workloads| link.send_message(Msg::SetWorkloads(workloads));

        let link = ctx.link().clone();
        let change_selected_workload =
            move |workload| link.send_message(Msg::ChangeSelectedWorkload(workload));

        let mut workloads = self.workloads.0.keys().cloned().collect::<Vec<_>>();
        workloads.sort_unstable();

        let selected_workload = self.selected_workload.clone();

        let link = ctx.link().clone();
        let on_drop = move |event: DragEvent| {
            // Prevent browser from opening the file
            event.stop_propagation();
            event.prevent_default();

            if let Some(items) = event.data_transfer() {
                if let Some(files) = items.files() {
                    // When multiple files are dropped, only get the first one
                    if let Some(file) = files.get(0) {
                        let file_reader = FileReader::new().unwrap();
                        let file_reader_clone = file_reader.clone();

                        let link = link.clone();
                        let on_load_end = Closure::once_into_js(move |event: ProgressEvent| {
                            if event.type_() == "loadend" {
                                let workloads = file_reader_clone.result().unwrap();
                                let workloads = workloads.as_string().unwrap();

                                if let Ok(workload_type_usage) =
                                    serde_json::from_slice::<WorkloadsInfo>(workloads.as_bytes())
                                {
                                    link.send_message(Msg::SetWorkloads(workload_type_usage));
                                }
                            }
                        });
                        file_reader.set_onloadend(Some(on_load_end.unchecked_ref()));

                        file_reader.read_as_text(&file).unwrap();
                    }
                }
            }
        };

        // Required to drop a file
        let on_drag_over = |event: DragEvent| {
            event.stop_propagation();
            event.prevent_default();
        };

        let system_to_components: HashMap<_, _> = self
            .selected_workload
            .as_ref()
            .map(|workload| {
                self.workloads.0[workload]
                    .batch_info
                    .iter()
                    .flat_map(|batch| {
                        batch.systems().map(|system| {
                            let components = system.borrow.clone();

                            (system.name.clone(), components)
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        let mut component_to_systems: HashMap<_, Vec<_>> = HashMap::new();
        if let Some(workload) = &self.selected_workload {
            for batch in &self.workloads.0[workload].batch_info {
                for system in batch.systems() {
                    for component in &system.borrow {
                        component_to_systems
                            .entry(component.name.to_string())
                            .or_default()
                            .push((system.name.clone(), component.mutability));
                    }
                }
            }
        }

        let workload_info = (self.page == Page::WorkloadEditor)
            .then(|| {
                self.selected_workload
                    .as_ref()
                    .map(|workload| self.workloads.0.get(workload).unwrap().clone())
            })
            .flatten();

        html! {
            <>
                <div style="display: flex; flex-direction: column; height: 100vh;">
                    <header
                        style="
                            position: relative;
                            min-height: 50px;
                            border-bottom: 1px solid black;
                        "
                    >
                        <a
                            onclick={on_click_title}
                            style="
                                position: absolute;
                                top: 50%;
                                left: 50%;
                                transform: translateY(-50%) translateX(-50%);
                                color: black;
                            "
                        >
                            {"Shipyard Visualizer"}
                        </a>
                    </header>

                    <div
                        id="this div"
                        style="display: flex; flex-direction: column; height: 100%;"
                        ondrop={on_drop}
                        ondragover={on_drag_over}
                    >
                        if self.page == Page::Home {
                            <Home {set_workloads} />
                        }

                        if self.page == Page::AccessInfo || self.page == Page::WorkloadEditor {
                            <WorkloadSelector
                                page={self.page}
                                {change_page}
                                {change_selected_workload}
                                {workloads}
                                {selected_workload}
                            />
                        }

                        if self.page == Page::AccessInfo {
                            <AccessInfo {system_to_components} {component_to_systems} />
                        }

                        if let Some(workload_info) = workload_info {
                            <WorkloadEditor {workload_info} />
                        }
                    </div>
                </div>
            </>
        }
    }
}

#[derive(PartialEq, Eq, Clone, Copy)]
enum Page {
    Home,
    AccessInfo,
    WorkloadEditor,
}

fn main() {
    set_panic_hook();

    yew::Renderer::<App>::new().render();
}
