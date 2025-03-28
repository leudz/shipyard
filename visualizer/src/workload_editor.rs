use shipyard::scheduler::info::{BatchInfo, Conflict, WorkloadInfo};
use wasm_bindgen::JsCast;
use web_sys::{window, HtmlDivElement, HtmlInputElement};
use yew::prelude::*;

pub(crate) struct WorkloadEditor {
    batches: Vec<BatchWindow>,
    dragged_batch: Option<usize>,
    zoom: f32,
    drag_ignore: bool,
    conflict: bool,
    before_after: bool,
}

pub(crate) enum Msg {
    Drag(i32, i32),
    BatchDragStart(usize),
    BatchDragEnd,
    Zoom(f32),
    ZoomDelta(f32),
    DragIgnoreStart,
    DragIgnoreEnd,
    ToggleConflict,
    ToggleBeforeAfter,
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
                        .systems()
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
            zoom: 1.0,
            drag_ignore: false,
            conflict: true,
            before_after: true,
        }
    }

    fn changed(&mut self, ctx: &Context<Self>, old_props: &Self::Properties) -> bool {
        ctx.props() != old_props
    }

    fn update(&mut self, _ctx: &Context<WorkloadEditor>, msg: Msg) -> bool {
        match msg {
            Msg::Drag(x, y) => {
                if self.drag_ignore {
                    return false;
                }

                if let Some(batch) = self.dragged_batch {
                    let pos = &mut self.batches[batch].pos;

                    pos.0 += (x as f32 / self.zoom) as i32;
                    pos.1 += (y as f32 / self.zoom) as i32;
                } else {
                    for batch in &mut self.batches {
                        batch.pos.0 += (x as f32 / self.zoom) as i32;
                        batch.pos.1 += (y as f32 / self.zoom) as i32;
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
            Msg::Zoom(zoom) => {
                self.zoom = 10.0f32.powf(zoom);

                true
            }
            Msg::ZoomDelta(delta) => {
                self.zoom -= delta / 1000.0;
                self.zoom = self.zoom.clamp(0.1, 10.0);

                true
            }
            Msg::DragIgnoreStart => {
                self.drag_ignore = true;

                false
            }
            Msg::DragIgnoreEnd => {
                self.drag_ignore = false;

                false
            }
            Msg::ToggleConflict => {
                self.conflict = !self.conflict;

                true
            }
            Msg::ToggleBeforeAfter => {
                self.before_after = !self.before_after;

                true
            }
        }
    }

    fn view(&self, ctx: &Context<WorkloadEditor>) -> Html {
        let batches: Html = self.batch_windows(ctx);
        let conflicts = self.conflict_paths();
        let after = self.before_after_paths();

        let link = ctx.link().clone();
        let on_input_slider = move |event: InputEvent| {
            link.send_message(Msg::Zoom(
                event
                    .target()
                    .unwrap()
                    .dyn_ref::<HtmlInputElement>()
                    .unwrap()
                    .value()
                    .parse::<f64>()
                    .unwrap() as f32,
            ));
        };

        let link1 = ctx.link().clone();
        let link2 = ctx.link().clone();
        let link3 = ctx.link().clone();
        let controls = html! {
            <div style="position: absolute; bottom: 0; right: 0; z-index: 1;">
                <div style="position: absolute; right: 0;">
                    <label for="conflicts">{"Conflicts "}</label>
                    <input
                        type="checkbox"
                        name="conflicts"
                        checked={self.conflict}
                        onchange={move |_| {
                            link1.send_message(Msg::ToggleConflict);
                        }}
                    />
                </div><br/>
                <div style="position: absolute; right: 0;">
                    <label for="before_after">{"Before/After "}</label>
                    <input
                        type="checkbox"
                        name="before_after"
                        checked={self.before_after}
                        onchange={move |_| {
                            link2.send_message(Msg::ToggleBeforeAfter);
                        }}
                    />
                </div><br/>
                <input
                    type="range"
                    min="-1"
                    max="1"
                    step="0.01"
                    value={self.zoom.log10().to_string()}
                    oninput={on_input_slider}
                    onmousedown={move |_| {
                        link3.send_message(Msg::DragIgnoreStart);
                    }}
                />
            </div>
        };

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
            link.send_message(Msg::DragIgnoreEnd);
        };

        let link = ctx.link().clone();
        let on_mouse_wheel = move |event: WheelEvent| {
            let delta = event.delta_y();

            link.send_message(Msg::ZoomDelta(delta as f32));
        };

        let zoom = self.zoom;

        html! {
            <div
                onmousemove={on_mouse_move}
                onmouseup={on_mouse_up}
                onwheel={on_mouse_wheel}
                style={format!("position: relative; width: 100%; height: 100%; overflow: hidden;")}
            >
                {controls}
                <div
                    style={
                        format!("
                            width: 100%;
                            height: 100%;
                            transform: scale({zoom});
                            overflow: visible;"
                        )
                    }
                >
                    {batches}

                    <svg
                        width="100%"
                        height="100%"
                        style="overflow: visible;"
                        xmlns="http://www.w3.org/2000/svg"
                    >
                        {conflicts}
                        {after}
                    </svg>
                </div>
            </div>
        }
    }
}

impl WorkloadEditor {
    fn batch_windows(&self, ctx: &Context<WorkloadEditor>) -> Html {
        self.batches
            .iter()
            .map(|batch| {
                let width = batch.width;
                let (x, y) = batch.pos;
                let i = batch.index;

                let systems: Html = batch
                    .info
                    .systems()
                    .map(|system| {
                        html! {
                            <>
                                <span>{&system.name}</span><br/>
                            </>
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

                        <div>
                            {systems}
                        </div>
                    </div>
                }
            })
            .collect()
    }

    fn conflict_paths(&self) -> Html {
        if !self.conflict {
            return html! {};
        }

        self.batches
            .iter()
            .flat_map(|batch| {
                let (batch_x, batch_y) = batch.pos;

                batch
                    .info
                    .systems()
                    .enumerate()
                    .filter_map(move |(i, system)| {
                        system.conflict.as_ref().and_then(|conflict| {
                            match conflict {
                                Conflict::Borrow { other_system, .. }
                                | Conflict::OtherNotSendSync {
                                    system: other_system,
                                    ..
                                } => {
                                    let prev_batch =
                                        self.batches.get(batch.index.checked_sub(1)?).unwrap();
                                    let (prev_x, prev_y) = prev_batch.pos;

                                    let src_x;
                                    let src_y;
                                    let dst_x;
                                    let dst_y;

                                    if let Some(prev_i) =
                                        prev_batch.info.systems().enumerate().find_map(
                                            |(i, system)| {
                                                (system.type_id == other_system.type_id)
                                                    .then(|| i as i32)
                                            },
                                        )
                                    {
                                        src_x = prev_x + prev_batch.width;
                                        src_y = prev_y
                                        + 24    // title
                                        + 1     // border
                                        + 2     // padding
                                        + 12    // half of the line
                                        + 24    // line
                                        * prev_i;
                                        dst_x = batch_x;
                                        dst_y = batch_y
                                        + 24    // title
                                        + 1     // border
                                        + 2     // padding
                                        + 12    // half of the line
                                        + 24    // line
                                        * i as i32;
                                    } else {
                                        let next_batch = self.batches.get(batch.index + 1)?;
                                        let (next_x, next_y) = next_batch.pos;

                                        src_x = batch_x + batch.width;
                                        src_y = batch_y
                                            + 24    // title
                                            + 1     // border
                                            + 2     // padding
                                            + 12    // half of the line
                                            + 24    // line
                                            * i as i32;
                                        dst_x = next_x;
                                        dst_y = next_y
                                        + 24    // title
                                        + 1     // border
                                        + 2     // padding
                                        + 12    // half of the line
                                        + 24    // line
                                        * next_batch
                                        .info
                                        .systems().enumerate().find_map(|(i, system)| {
                                            (
                                                system.type_id == other_system.type_id
                                            ).then(|| i as i32)
                                        })?;
                                    }

                                    let control_scale = ((dst_x - src_x) / 2).max(30);
                                    let src_control_x = src_x + control_scale;
                                    let src_control_y = src_y;
                                    let dst_control_x = dst_x - control_scale;
                                    let dst_control_y = dst_y;

                                    let path = format!(
                                        "
                                        M {src_x} {src_y}
                                        C {src_control_x} {src_control_y},
                                        {dst_control_x} {dst_control_y},
                                        {dst_x} {dst_y}"
                                    );

                                    Some(html! {
                                        <path
                                            d={path}
                                            stroke="black"
                                            fill="transparent"
                                            stroke-width="1"
                                        />
                                    })
                                }
                                Conflict::NotSendSync(_) => {
                                    let prev_batch =
                                        self.batches.get(batch.index.checked_sub(1)?).unwrap();
                                    let (prev_x, prev_y) = prev_batch.pos;

                                    let src_x = prev_x + prev_batch.width;
                                    let src_y = prev_y;
                                    let dst_x = batch_x;
                                    let dst_y = batch_y
                                        + 24    // title
                                        + 1     // border
                                        + 2     // padding
                                        + 12    // half of the line
                                        + 24    // line
                                        * i as i32;

                                    let control_scale = ((dst_x - src_x) / 2).max(30);
                                    let src_control_x = src_x + control_scale;
                                    let src_control_y = src_y;
                                    let dst_control_x = dst_x - control_scale;
                                    let dst_control_y = dst_y;

                                    let path = format!(
                                        "
                                        M {src_x} {src_y}
                                        C {src_control_x} {src_control_y},
                                        {dst_control_x} {dst_control_y},
                                        {dst_x} {dst_y}"
                                    );

                                    Some(html! {
                                        <path
                                            d={path}
                                            stroke="black"
                                            fill="transparent"
                                            stroke-width="1"
                                        />
                                    })
                                }
                            }
                        })
                    })
            })
            .collect::<Html>()
    }

    fn before_after_paths(&self) -> Html {
        if !self.before_after {
            return html! {};
        }

        self.batches
            .iter()
            .flat_map(|batch| {
                let (batch_x, batch_y) = batch.pos;

                batch
                    .info
                    .systems()
                    .enumerate()
                    .flat_map(move |(i, system)| {
                        system.after.iter().flat_map(move |before| {
                            (self.batches).iter().filter_map(move |other_batch| {
                                let (prev_x, prev_y) = other_batch.pos;

                                let src_x = prev_x + other_batch.width;
                                let src_y = prev_y
                                    + 24    // title
                                    + 1     // border
                                    + 2     // padding
                                    + 12    // half of the line
                                    + 24    // line
                                    * other_batch
                                        .info
                                        .systems()
                                        .enumerate().find_map(|(i, system)| {
                                            (
                                                &system.name
                                                == before
                                                .strip_prefix("System(")
                                                .unwrap_or(&before)
                                                .strip_suffix(")")
                                                .unwrap_or(&before)
                                            )
                                            .then(|| i as i32)
                                        })?;
                                let dst_x = batch_x;
                                let dst_y = batch_y
                                    + 24    // title
                                    + 1     // border
                                    + 2     // padding
                                    + 12    // half of the line
                                    + 24    // line
                                    * i as i32;

                                let control_scale = ((dst_x - src_x) / 2).max(30);
                                let src_control_x = src_x + control_scale;
                                let src_control_y = src_y;
                                let dst_control_x = dst_x - control_scale;
                                let dst_control_y = dst_y;

                                let path = format!(
                                    "
                                    M {src_x} {src_y}
                                    C {src_control_x} {src_control_y},
                                    {dst_control_x} {dst_control_y},
                                    {dst_x} {dst_y}"
                                );

                                Some(html! {
                                    <path
                                        d={path}
                                        stroke="black"
                                        fill="transparent"
                                        stroke-width="1"
                                    />
                                })
                            })
                        })
                    })
            })
            .collect::<Html>()
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
    element.set_inner_html(&text.replace('<', "&lt;").replace(">", "&gt;"));
    element
        .set_attribute("style", "position: absolute;")
        .unwrap();

    let node = document.body().unwrap().append_child(&element).unwrap();
    let width = node.dyn_ref::<HtmlDivElement>().unwrap().client_width();

    node.parent_element().unwrap().remove_child(&node).unwrap();

    width
}
