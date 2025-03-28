use shipyard::scheduler::info::WorkloadsInfo;
use yew::prelude::*;

pub(crate) struct Home;

#[derive(Properties, PartialEq)]
pub(crate) struct Props {
    pub(crate) set_workloads: Callback<WorkloadsInfo>,
}

impl Component for Home {
    type Message = ();
    type Properties = Props;

    fn create(_ctx: &Context<Home>) -> Home {
        Home
    }

    fn view(&self, ctx: &Context<Home>) -> Html {
        let set_workloads = ctx.props().set_workloads.clone();
        let on_click_examples = move |_| {
            let example = include_bytes!("square_eater_workloads.json");

            let workload_type_usage = serde_json::from_slice::<WorkloadsInfo>(example).unwrap();
            set_workloads.emit(workload_type_usage);
        };

        html! {
            <div
                style="
                    position: absolute;
                    top: 50%;
                    left: 50%;
                    transform: translateY(-50%) translateX(-50%);
                "
            >
                <span style="user-select: none;">{"Drop Json file of "}</span>
                <code style="user-select: none;">{"shipyard::info::WorkloadsTypeUsage"}</code>
                <span style="user-select: none;">{". "}</span><br/>
                <a
                    onclick={on_click_examples}
                    style="user-select: none;"
                >
                    {"Or check out the example."}
                </a><br/><br/>
                <span>{"std::fs::write("}</span><br/>
                <span>{"\u{00a0}\u{00a0}\u{00a0}\u{00a0}\"drop_me.json\","}</span><br/>
                <span>
                    {"\u{00a0}\u{00a0}\u{00a0}\u{00a0}
                        serde_json::to_string(&world.workloads_info()).unwrap(),"}
                </span><br/>
                <span>{")"}</span><br/>
                <span>{".unwrap();"}</span>
            </div>
        }
    }
}
