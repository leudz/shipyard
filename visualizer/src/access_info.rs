use shipyard::borrow::Mutability;
use shipyard::scheduler::info::TypeInfo;
use std::collections::HashMap;
use yew::prelude::*;

pub(crate) struct AccessInfo {
    systems: Vec<String>,
    systems_mutability: Vec<Option<Mutability>>,
    /// First one is the full name, the second one is trimmed [trim_component_name]
    components: Vec<(String, String)>,
    components_mutability: Vec<Option<Mutability>>,
    selected_system: Option<String>,
    selected_component: Option<String>,
}

pub(crate) enum Msg {
    SetSelectedSystem(String),
    SetSelectedComponent(String),
}

#[derive(Properties, PartialEq)]
pub(crate) struct Props {
    pub(crate) system_to_components: HashMap<String, Vec<TypeInfo>>,
    pub(crate) component_to_systems: HashMap<String, Vec<(String, Mutability)>>,
}

impl Component for AccessInfo {
    type Message = Msg;
    type Properties = Props;

    fn create(ctx: &Context<AccessInfo>) -> AccessInfo {
        AccessInfo {
            systems: ctx.props().system_to_components.keys().cloned().collect(),
            systems_mutability: ctx
                .props()
                .system_to_components
                .keys()
                .map(|_| None)
                .collect(),
            components: ctx
                .props()
                .component_to_systems
                .keys()
                .map(|component| (component.clone(), trim_component_name(component)))
                .collect(),
            components_mutability: ctx
                .props()
                .component_to_systems
                .keys()
                .map(|_| None)
                .collect(),
            selected_system: None,
            selected_component: None,
        }
    }

    fn changed(&mut self, ctx: &Context<Self>, old_props: &Self::Properties) -> bool {
        if ctx.props() == old_props {
            return false;
        }

        self.systems.clear();
        self.systems_mutability.clear();
        self.components.clear();
        self.components_mutability.clear();
        self.selected_system = None;
        self.selected_component = None;

        self.systems
            .extend(ctx.props().system_to_components.keys().cloned());
        self.systems.sort_unstable();
        self.components.extend(
            ctx.props()
                .component_to_systems
                .keys()
                .map(|component| (component.clone(), trim_component_name(component))),
        );
        self.components
            .sort_unstable_by(|(_, name1), (_, name2)| name1.cmp(&name2));
        self.systems_mutability
            .extend((0..self.systems.len()).map(|_| None));
        self.components_mutability
            .extend((0..self.components.len()).map(|_| None));

        true
    }

    fn update(&mut self, ctx: &Context<AccessInfo>, msg: Msg) -> bool {
        match msg {
            Msg::SetSelectedSystem(system) => {
                if self.selected_system.as_ref() == Some(&system) {
                    return false;
                }

                let components = &ctx.props().system_to_components[&system];
                for (component, mutability) in
                    self.components.iter().zip(&mut self.components_mutability)
                {
                    if let Some(component) = components
                        .iter()
                        .find(|type_info| type_info.name == *component.0)
                    {
                        *mutability = Some(component.mutability);
                    } else {
                        *mutability = None;
                    }
                }

                self.selected_system = Some(system);

                self.selected_component = None;
                self.systems_mutability
                    .iter_mut()
                    .for_each(|mutability| *mutability = None);
            }
            Msg::SetSelectedComponent(component) => {
                if self.selected_component.as_ref() == Some(&component) {
                    return false;
                }

                let systems = &ctx.props().component_to_systems[&component];
                for (system, mutability) in self.systems.iter().zip(&mut self.systems_mutability) {
                    if let Some((_, sys_mutability)) =
                        systems.iter().find(|(name, _)| name == system)
                    {
                        *mutability = Some(*sys_mutability);
                    } else {
                        *mutability = None;
                    }
                }

                self.selected_component = Some(component);

                self.selected_system = None;
                self.components_mutability
                    .iter_mut()
                    .for_each(|mutability| *mutability = None);
            }
        }

        true
    }

    fn view(&self, ctx: &Context<AccessInfo>) -> Html {
        let len = self.systems.len().max(self.components.len());

        let link = ctx.link().clone();
        let rows: Html = self
            .systems
            .iter()
            .zip(&self.systems_mutability)
            .chain(std::iter::repeat((&String::new(), &None)))
            .zip(
                self.components
                    .iter()
                    .zip(&self.components_mutability)
                    .chain(std::iter::repeat((&(String::new(), String::new()), &None))),
            )
            .take(len)
            .map(
                move |((system, sys_mutability), (component, comp_mutability))| {
                    let mut sys_style = "border-radius: 10px;".to_string();
                    let mut comp_style = "border-radius: 10px;".to_string();

                    sys_style += match sys_mutability {
                        Some(Mutability::Exclusive) => "background-color: #FF8080;",
                        Some(Mutability::Shared) => "background-color: #ADD8E6;",
                        None if Some(system) == self.selected_system.as_ref() => {
                            "background-color: darkgrey;"
                        }
                        None if !system.is_empty() => "background-color: #e8e8e8;",
                        None => "",
                    };
                    comp_style += match comp_mutability {
                        Some(Mutability::Exclusive) => "background-color: #FF8080;",
                        Some(Mutability::Shared) => "background-color: #ADD8E6;",
                        None if Some(&component.0) == self.selected_component.as_ref() => {
                            "background-color: darkgrey;"
                        }
                        None if !component.0.is_empty() => "background-color: #e8e8e8;",
                        None => "",
                    };
                    if !system.is_empty() {
                        sys_style += "cursor: pointer;";
                    }
                    if !component.0.is_empty() {
                        comp_style += "cursor: pointer;";
                    }

                    let link_clone = link.clone();
                    let system_clone = system.clone();
                    let on_click_sys = move |_| {
                        // If there are more components than systems there are some padding systems
                        if !system_clone.is_empty() {
                            link_clone.send_message(Msg::SetSelectedSystem(system_clone.clone()))
                        }
                    };
                    let link_clone = link.clone();
                    let component_clone = component.0.clone();
                    let on_click_comp = move |_| {
                        // If there are more systems than components
                        // there are some padding components
                        if !component_clone.is_empty() {
                            link_clone
                                .send_message(Msg::SetSelectedComponent(component_clone.clone()))
                        }
                    };

                    html! {
                        <tr>
                            <td onclick={on_click_sys} style={sys_style}>
                                <span>{system}</span>
                            </td>

                            <td onclick={on_click_comp} style={comp_style}>
                                <span>{&component.1}</span>
                            </td>
                        </tr>
                    }
                },
            )
            .collect();

        html! {
            <div style="overflow-y: auto;">
                <table
                    style="
                        width: 100%;
                        table-layout: fixed;
                        text-align: center;
                        border-collapse: separate;
                        border-spacing: 10px 3px;
                    "
                >
                    if {rows != html! {}} {
                        <thead>
                            <tr>
                                <th>{"Systems"}</th>
                                <th>{"Components"}</th>
                            </tr>
                        </thead>
                    }

                    <tbody>
                        {rows}
                    </tbody>
                </table>
            </div>
        }
    }
}

fn trim_component_name(name: &str) -> String {
    name.trim_start_matches("shipyard::")
        .trim_start_matches("sparse_set::")
        .trim_start_matches("unique::")
        .trim_start_matches("all_storages::")
        .trim_start_matches("entities::")
        .to_string()
}
