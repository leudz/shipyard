use shipyard::info::{BatchInfo, Conflict, WorkloadInfo};
use wasm_bindgen::JsCast;
use web_sys::{window, HtmlDivElement};
use yew::prelude::*;

pub(crate) struct WorkloadEditor {
    batches: Vec<BatchWindow>,
    dragged_batch: Option<usize>,
}

pub(crate) enum Msg {
    Drag(i32, i32),
    BatchDragStart(usize),
    BatchDragEnd,
}

#[derive(Properties, PartialEq)]
pub(crate) struct Props {
    pub(crate) workload_info: WorkloadInfo,
}

impl Component for WorkloadEditor {
    type Message = Msg;
    type Properties = Props;

    fn create(ctx: &Context<WorkloadEditor>) -> WorkloadEditor {
        let window = window().unwrap();
        let win_width = window.inner_width().unwrap().as_f64().unwrap() as i32;
        let win_height = window.inner_height().unwrap().as_f64().unwrap() as i32;

        let mut offset = 0;
        let batches = ctx
            .props()
            .workload_info
            .batch_info
            .iter()
            .enumerate()
            .map(|(i, batch)| {
                let width = text_width(
                    &batch
                        .systems
                        .0
                        .iter()
                        .chain(&batch.systems.1)
                        .max_by(|sys1, sys2| sys1.name.len().cmp(&sys2.name.len()))
                        .unwrap()
                        .name,
                ) + 16;

                let pos = (win_width / 4 + offset, win_height / 4);

                offset += width + 50;

                BatchWindow {
                    index: i,
                    info: batch.clone(),
                    pos,
                    width,
                }
            })
            .collect();

        WorkloadEditor {
            batches,
            dragged_batch: None,
        }
    }

    fn changed(&mut self, ctx: &Context<Self>, old_props: &Self::Properties) -> bool {
        ctx.props() != old_props
    }

    fn update(&mut self, _ctx: &Context<WorkloadEditor>, msg: Msg) -> bool {
        match msg {
            Msg::Drag(x, y) => {
                if let Some(batch) = self.dragged_batch {
                    let pos = &mut self.batches[batch].pos;

                    pos.0 += x;
                    pos.1 += y;
                } else {
                    for batch in &mut self.batches {
                        batch.pos.0 -= x;
                        batch.pos.1 -= y;
                    }
                }

                true
            }
            Msg::BatchDragStart(batch) => {
                self.dragged_batch = Some(batch);

                false
            }
            Msg::BatchDragEnd => {
                self.dragged_batch = None;

                false
            }
        }
    }

    fn view(&self, ctx: &Context<WorkloadEditor>) -> Html {
        let batches: Html = self
            .batches
            .iter()
            .map(|batch| {
                let width = batch.width;
                let (x, y) = batch.pos;
                let i = batch.index;

                let systems: Html = batch
                    .info
                    .systems
                    .0
                    .iter()
                    .chain(&batch.info.systems.1)
                    .map(|system| {
                        html! {
                            <span>{&system.name}</span>
                        }
                    })
                    .collect();

                let link = ctx.link().clone();
                let on_mouse_down = move |_event: MouseEvent| {
                    link.send_message(Msg::BatchDragStart(i));
                };

                html! {
                    <div
                        onmousedown={on_mouse_down}
                        style={format!("
                            position: absolute;
                            width: {width}px;
                            top: {y}px;
                            left: {x}px;
                            text-align: center;
                            box-shadow: 0 4px 8px 0 rgba(0,0,0,0.25);
                            border-radius: 5px;
                            user-select: none;
                            background-color: pink;
                        ")}
                    >
                        <header style="border-bottom: 1px solid black;">
                            {format!("Batch {i}")}
                        </header>

                        <div style="padding: 2px 8px;">
                            {systems}
                        </div>
                    </div>
                }
            })
            .collect();

        let conflicts = self
            .batches
            .iter()
            .flat_map(|batch| {
                let (batch_x, batch_y) = batch.pos;

                batch
                    .info
                    .systems
                    .0
                    .iter()
                    .chain(&batch.info.systems.1)
                    .enumerate()
                    .filter_map(move |(i, system)| {
                        system.conflict.as_ref().map(|conflict| {
                            match conflict {
                                Conflict::Borrow {
                                    other_system,
                                    ..
                                }
                                | Conflict::OtherNotSendSync {
                                    system: other_system,
                                    ..
                                } => {
                                    let prev_batch = &self.batches[batch.index - 1];
                                    let (prev_x, prev_y) = prev_batch.pos;

                                    let src_x = prev_x + prev_batch.width;
                                    let src_y =
                                        prev_y
                                        + 24    // title
                                        + 1     // border
                                        + 2     // padding
                                        + 12    // half of the line
                                        + 28    // line + padding
                                        * prev_batch
                                            .info
                                            .systems
                                            .0
                                            .iter()
                                            .chain(&prev_batch.info.systems.1).enumerate().find_map(|(i, system)| {
                                                (system.type_id == other_system.type_id).then(|| i as i32)
                                            }).unwrap();
                                    let dst_x = batch_x;
                                    let dst_y =
                                        batch_y
                                        + 24    // title
                                        + 1     // border
                                        + 2     // padding
                                        + 12    // half of the line
                                        + 28    // line + padding
                                        * i as i32;

                                    let control_scale = ((dst_x - src_x) / 2).max(30);
                                    let src_control_x = src_x + control_scale;
                                    let src_control_y = src_y;
                                    let dst_control_x = dst_x - control_scale;
                                    let dst_control_y = dst_y;

                                    let path = format!("
                                        M {src_x} {src_y}
                                        C {src_control_x} {src_control_y},
                                        {dst_control_x} {dst_control_y},
                                        {dst_x} {dst_y}"
                                    );

                                    html! {
                                        <path d={path} stroke="black" fill="transparent" stroke-width="1" />
                                    }
                                },
                                Conflict::NotSendSync(_) => todo!(),
                            }
                        })
                    })
            })
            .collect::<Html>();

        let link = ctx.link().clone();
        let on_mouse_move = move |event: MouseEvent| {
            // When left mouse button is pressed
            if event.buttons() == 1 {
                let x = event.movement_x();
                let y = event.movement_y();

                link.send_message(Msg::Drag(x, y));
            }
        };

        let link = ctx.link().clone();
        let on_mouse_up = move |_event: MouseEvent| {
            link.send_message(Msg::BatchDragEnd);
        };

        html! {
            <div
                onmousemove={on_mouse_move}
                onmouseup={on_mouse_up}
                style="position: relative; width: 100%; height: 100%; overflow: hidden;"
            >
                {batches}
                <svg width="100%" height="100%" xmlns="http://www.w3.org/2000/svg">
                    {conflicts}
                </svg>
            </div>
        }
    }
}

struct BatchWindow {
    info: BatchInfo,
    pos: (i32, i32),
    width: i32,
    index: usize,
}

fn text_width(text: &str) -> i32 {
    let document = window().unwrap().document().unwrap();

    let element = document.create_element("div").unwrap();
    element.set_inner_html(text);
    element
        .set_attribute("style", "position: absolute;")
        .unwrap();

    let node = document.body().unwrap().append_child(&element).unwrap();
    let width = node.dyn_ref::<HtmlDivElement>().unwrap().client_width();

    node.parent_element().unwrap().remove_child(&node).unwrap();

    width
}
