use shipyard::*;
use std::convert::TryInto;

use std::collections::HashMap;

struct Nothing;

struct ScriptSystem {
    name: String,
}

fn main() {
    // This stores our components by name
    let mut components: HashMap<String, CustomComponent> = HashMap::new();

    // Insert our custom components
    components.insert(
        "position".into(),
        CustomComponent {
            size: 32.try_into().unwrap(),
            id: 0,
        },
    );

    components.insert(
        "velocity".into(),
        CustomComponent {
            size: 16.try_into().unwrap(),
            id: 1,
        },
    );

    // Create a custom system
    let insert_pos_vel_sys = DynamicSystem {
        data: (),
        system_fn: |_, borrows| {},
        borrow_intents: vec![
            CustomComponentBorrowIntent {
                component: components.get("velocity").unwrap().clone(),
                mutation: Mutation::Shared,
            },
            CustomComponentBorrowIntent {
                component: components.get("position").unwrap().clone(),
                mutation: Mutation::Unique,
            },
        ],
    };

    let pos_vel_sys = DynamicSystem {
        data: (),
        system_fn: |_, borrows| {
            let velocities = borrows.get(0).unwrap();
            let positions = borrows.get(0).unwrap();
        },
        borrow_intents: vec![
            CustomComponentBorrowIntent {
                component: components.get("velocity").unwrap().clone(),
                mutation: Mutation::Shared,
            },
            CustomComponentBorrowIntent {
                component: components.get("position").unwrap().clone(),
                mutation: Mutation::Unique,
            },
        ],
    };

    let world = World::new();
    world.run(insert_pos_vel_sys);
    world.run(pos_vel_sys);
}
