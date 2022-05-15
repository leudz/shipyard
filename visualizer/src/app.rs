use crate::syntax_highlight::code_view_ui;
use shipyard::{
    info::{TypeInfo, WorkloadsTypeUsage},
    Mutability,
};
use std::{borrow::Cow, collections::HashMap, io::Read};

pub struct MyApp {
    workloads: Option<WorkloadsTypeUsage>,
    selected_workload: Option<String>,
    info_display: InfoDisplay,
    info: Info,
    info_selection: InfoSelection,
    selected_system: Option<String>,
    selected_component: Option<String>,
}

impl Default for MyApp {
    fn default() -> MyApp {
        MyApp {
            workloads: None,
            selected_workload: None,
            info_display: InfoDisplay {
                systems: Vec::new(),
                components: Vec::new(),
            },
            info: Info {
                system_to_components: HashMap::new(),
                component_to_systems: HashMap::new(),
            },
            info_selection: InfoSelection {
                systems: HashMap::new(),
                components: HashMap::new(),
            },
            selected_system: None,
            selected_component: None,
        }
    }
}

struct InfoDisplay {
    systems: Vec<String>,
    components: Vec<String>,
}

struct Info {
    system_to_components: HashMap<String, Vec<TypeInfo>>,
    component_to_systems: HashMap<String, Vec<(String, Mutability)>>,
}

struct InfoSelection {
    systems: HashMap<String, Mutability>,
    components: HashMap<String, Mutability>,
}

#[rustfmt::skip]
const CODE: &str = 
r#"std::fs::write(
    "drop_me.json",
    serde_json::to_string(&world.workloads_type_usage()).unwrap(),
)
.unwrap();"#;

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::Area::new("Light switch")
            .fixed_pos((0.0, 0.0))
            .show(ctx, |ui| {
                egui::widgets::global_dark_light_mode_switch(ui);
            });

        if self.workloads.is_none() {
            egui::Area::new("Drop")
                .fixed_pos(ctx.available_rect().center())
                .anchor(egui::Align2::CENTER_CENTER, (0.0, 0.0))
                .show(ctx, |ui| {
                    if ui.style().visuals.dark_mode {
                        ui.style_mut().visuals.override_text_color =
                            Some(egui::color::Color32::from_gray(255));
                    }

                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            ui.style_mut().spacing.item_spacing.x = 0.0;
                            ui.heading("Drop Json file of ");
                            ui.heading(
                                egui::RichText::new("shipyard::info::WorkloadsTypeUsage").code(),
                            );
                            ui.heading(".");
                        });

                        if ui.button("Or check out the example.").clicked() {
                            ctx.input_mut().raw.dropped_files.push(egui::DroppedFile {
                                path: Some("square_eater_workloads.json".into()),
                                name: "square_eater_workloads.json".to_string(),
                                last_modified: None,
                                bytes: Some(
                                    include_bytes!("square_eater_workloads.json")[..].into(),
                                ),
                            })
                        }

                        ui.add_space(15.0);

                        code_view_ui(ui, CODE);
                    });
                });
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            if ui.style().visuals.dark_mode {
                ui.style_mut().visuals.override_text_color =
                    Some(egui::color::Color32::from_gray(255));
            }

            ui.vertical_centered(|ui| {
                if ui
                    .add(
                        egui::Button::new(egui::RichText::new("Shipyard Visualizer").heading())
                            .frame(false),
                    )
                    .clicked()
                {
                    self.workloads = None;
                }
            });

            ui.add_space(10.0);

            if let Some(workloads) = &self.workloads {
                ui.horizontal(|ui| {
                    ui.label("Selected workload:");

                    egui::ComboBox::from_label("")
                        .width(300.0)
                        .selected_text(format!(
                            "{:?}",
                            self.selected_workload
                                .clone()
                                .unwrap_or_else(|| "None".to_string())
                        ))
                        .show_ui(ui, |ui| {
                            workloads.0.keys().for_each(|workload_name| {
                                if ui
                                    .add(egui::SelectableLabel::new(
                                        self.selected_workload.as_ref() == Some(workload_name),
                                        workload_name,
                                    ))
                                    .clicked()
                                {
                                    self.selected_workload = Some(workload_name.clone());

                                    let workload = workloads.0.get(workload_name).unwrap();
                                    let mut system_to_components = HashMap::new();
                                    let mut component_to_systems: HashMap<
                                        String,
                                        Vec<(String, Mutability)>,
                                    > = HashMap::new();
                                    let mut systems_display: Vec<String> = Vec::new();
                                    let mut components_display: Vec<TypeInfo> = Vec::new();

                                    for (system, components) in workload {
                                        let mut components = components.clone();
                                        components.sort();
                                        components.dedup_by(|a, b| a.storage_id == b.storage_id);

                                        for component in &components {
                                            let name = component
                                                .name
                                                .trim_start_matches("shipyard::")
                                                .trim_start_matches("sparse_set::")
                                                .trim_start_matches("unique::")
                                                .trim_start_matches("all_storages::")
                                                .trim_start_matches("entities::")
                                                .to_string();

                                            component_to_systems
                                                .entry(name)
                                                .or_default()
                                                .push((system.to_string(), component.mutability));
                                        }

                                        system_to_components
                                            .insert(system.to_string(), components.clone());

                                        if let Err(index) = systems_display
                                            .binary_search_by(|probe| (**probe).cmp(&**system))
                                        {
                                            systems_display.insert(index, system.to_string());
                                        }

                                        for component in components {
                                            if let Err(index) =
                                                components_display.binary_search_by(|probe| {
                                                    probe.storage_id.cmp(&component.storage_id)
                                                })
                                            {
                                                components_display.insert(index, component);
                                            }
                                        }
                                    }

                                    let components_display = components_display
                                        .into_iter()
                                        .map(|component| {
                                            component
                                                .name
                                                .trim_start_matches("shipyard::")
                                                .trim_start_matches("sparse_set::")
                                                .trim_start_matches("unique::")
                                                .trim_start_matches("all_storages::")
                                                .trim_start_matches("entities::")
                                                .to_string()
                                        })
                                        .collect();

                                    self.info_display = InfoDisplay {
                                        systems: systems_display,
                                        components: components_display,
                                    };

                                    self.info = Info {
                                        system_to_components,
                                        component_to_systems,
                                    };
                                }
                            });
                        });

                    if ui
                        .add_enabled(self.selected_workload.is_some(), egui::Button::new("Clear"))
                        .clicked()
                    {
                        self.selected_workload = None;
                    }
                });

                if self.selected_workload.is_some() {
                    ui.add_space(20.0);

                    ui.columns(2, |columns| {
                        columns[0].vertical_centered_justified(|systems| {
                            systems.heading("Systems");

                            self.info_display.systems.iter().for_each(|system| {
                                let system_color = if self.selected_system.as_ref() == Some(system)
                                {
                                    Some(systems.visuals().widgets.active.bg_fill)
                                } else {
                                    self.info_selection.systems.get(system).map(|mutability| {
                                        match mutability {
                                            Mutability::Shared => shared_color(systems),
                                            Mutability::Exclusive => exclusive_color(systems),
                                        }
                                    })
                                };

                                let mut system_button = egui::Button::new(system);
                                if let Some(color) = system_color {
                                    let mut stroke = systems.visuals().widgets.inactive.fg_stroke;
                                    stroke.color = egui::color::Color32::BLACK;
                                    system_button = system_button.fill(color).stroke(stroke);
                                }

                                if systems.add(system_button).clicked() {
                                    self.selected_system = Some(system.clone());
                                    self.selected_component = None;

                                    let components = self
                                        .info
                                        .system_to_components
                                        .get(system)
                                        .unwrap()
                                        .iter()
                                        .map(|component| {
                                            (
                                                component
                                                    .name
                                                    .trim_start_matches("shipyard::")
                                                    .trim_start_matches("sparse_set::")
                                                    .trim_start_matches("unique::")
                                                    .trim_start_matches("all_storages::")
                                                    .trim_start_matches("entities::")
                                                    .to_string(),
                                                component.mutability,
                                            )
                                        })
                                        .collect();

                                    self.info_selection = InfoSelection {
                                        systems: HashMap::new(),
                                        components,
                                    };
                                }
                            });
                        });

                        columns[1].vertical_centered_justified(|components| {
                            components.heading("Components");

                            self.info_display.components.iter().for_each(|component| {
                                let component_color = if self.selected_component.as_ref()
                                    == Some(component)
                                {
                                    Some(components.visuals().widgets.active.bg_fill)
                                } else {
                                    self.info_selection.components.get(component).map(
                                        |mutability| match mutability {
                                            Mutability::Shared => shared_color(components),
                                            Mutability::Exclusive => exclusive_color(components),
                                        },
                                    )
                                };

                                let mut component_button = egui::Button::new(component);
                                if let Some(color) = component_color {
                                    component_button = component_button.fill(color);
                                }

                                if components.add(component_button).clicked() {
                                    self.selected_system = None;
                                    self.selected_component = Some(component.clone());

                                    let systems = self
                                        .info
                                        .component_to_systems
                                        .get(component)
                                        .unwrap()
                                        .iter()
                                        .cloned()
                                        .collect();

                                    self.info_selection = InfoSelection {
                                        systems,
                                        components: HashMap::new(),
                                    };
                                }
                            });
                        });
                    });
                }
            }
        });

        // Collect dropped files:
        if !ctx.input().raw.dropped_files.is_empty() {
            let dropped_files = &ctx.input().raw.dropped_files;

            if let Some(dropped_file) = dropped_files.get(0) {
                let mut bytes: Cow<'_, [u8]> = Cow::Borrowed(&[]);
                if let Some(dropped_bytes) = &dropped_file.bytes {
                    bytes = (&**dropped_bytes).into();
                } else if let Some(path) = &dropped_file.path {
                    if let Ok(mut file) = std::fs::File::open(&path) {
                        let mut buffer = Vec::new();
                        file.read_to_end(&mut buffer).unwrap();
                        bytes = buffer.into();
                    }
                }

                if let Ok(workload_type_usage) =
                    serde_json::from_slice::<WorkloadsTypeUsage>(&bytes)
                {
                    self.workloads = Some(workload_type_usage);
                    self.selected_workload = None;
                }
            }
        }
    }
}

fn shared_color(ui: &mut egui::Ui) -> egui::color::Color32 {
    if ui.visuals().dark_mode {
        egui::color::Color32::DARK_BLUE
    } else {
        egui::color::Color32::LIGHT_BLUE
    }
}

fn exclusive_color(ui: &mut egui::Ui) -> egui::color::Color32 {
    if ui.visuals().dark_mode {
        egui::color::Color32::DARK_RED
    } else {
        egui::color::Color32::LIGHT_RED
    }
}
