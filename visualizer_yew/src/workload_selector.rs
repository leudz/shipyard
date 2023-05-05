use crate::Page;
use wasm_bindgen::JsCast;
use yew::prelude::*;

pub(crate) struct WorkloadSelector;

#[derive(Properties, PartialEq)]
pub(crate) struct Props {
    pub(crate) page: Page,
    pub(crate) change_page: Callback<Page>,
    pub(crate) change_selected_workload: Callback<Option<String>>,
    pub(crate) workloads: Vec<String>,
    pub(crate) selected_workload: Option<String>,
}

impl Component for WorkloadSelector {
    type Message = ();
    type Properties = Props;

    fn create(_ctx: &Context<WorkloadSelector>) -> WorkloadSelector {
        WorkloadSelector
    }

    fn changed(&mut self, ctx: &Context<Self>, old_props: &Self::Properties) -> bool {
        ctx.props().selected_workload != old_props.selected_workload
            || ctx.props().page != old_props.page
            || ctx.props().workloads != old_props.workloads
    }

    fn view(&self, ctx: &Context<WorkloadSelector>) -> Html {
        let change_page = ctx.props().change_page.clone();
        let on_click_access_info = move |_| change_page.emit(Page::AccessInfo);
        let change_page = ctx.props().change_page.clone();
        let on_click_editor = move |_| change_page.emit(Page::WorkloadEditor);

        let change_workload = ctx.props().change_selected_workload.clone();
        let on_change_workload = move |event: Event| {
            let target = event.target().unwrap();
            let select: &web_sys::HtmlSelectElement = target.dyn_ref().unwrap();
            let workload = select.value();

            if workload == "None" {
                change_workload.emit(None);
            } else {
                change_workload.emit(Some(workload));
            }
        };

        let workloads = std::iter::once(html! {
            <option
                selected={ctx.props().selected_workload.is_none()}
            >
                {"None"}
            </option>
        })
        .chain(ctx.props().workloads.iter().map(|workload| {
            html! {
                <option
                    value={workload.clone()}
                    selected={Some(workload) == ctx.props().selected_workload.as_ref()}
                >
                    {workload}
                </option>
            }
        }))
        .collect::<Html>();

        html! {
            <div>
                <span>{"Selected workload: "}</span>
                <select onchange={on_change_workload} style="margin-right: 5px;">{workloads}</select>
                if ctx.props().page != Page::AccessInfo {
                    <button onclick={on_click_access_info} >{"Access Info"}</button>
                }
                if ctx.props().page != Page::WorkloadEditor {
                    <button onclick={on_click_editor}>{"Editor"}</button>
                }
            </div>
        }
    }
}
