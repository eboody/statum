use statum::{machine, state, validators};

#[state]
enum State {
    Draft,
    Published,
}

#[machine]
struct Machine<State> {
    tenant: String,
    priority: u8,
}

struct ArticleRow {
    tenant: String,
    priority: u8,
    status: &'static str,
}

#[validators(Machine)]
impl ArticleRow {
    fn is_draft(&self) -> Result<(), statum::Error> {
        if self.status == "draft" {
            Ok(())
        } else {
            Err(statum::Error::InvalidState)
        }
    }

    fn is_published(&self) -> Result<(), statum::Error> {
        if self.status == "published" {
            Ok(())
        } else {
            Err(statum::Error::InvalidState)
        }
    }
}

pub fn run() {
    let rows = vec![
        ArticleRow {
            tenant: "acme".to_owned(),
            priority: 1,
            status: "draft",
        },
        ArticleRow {
            tenant: "globex".to_owned(),
            priority: 3,
            status: "published",
        },
    ];

    let machines = rows
        .into_machines_by(|row| machine::Fields {
            tenant: row.tenant.clone(),
            priority: row.priority,
        })
        .build();

    let machines: Vec<machine::SomeState> = machines.into_iter().map(Result::unwrap).collect();

    match &machines[0] {
        machine::SomeState::Draft(machine) => {
            assert_eq!(machine.tenant.as_str(), "acme");
            assert_eq!(machine.priority, 1);
        }
        _ => panic!("expected draft article"),
    }

    match &machines[1] {
        machine::SomeState::Published(machine) => {
            assert_eq!(machine.tenant.as_str(), "globex");
            assert_eq!(machine.priority, 3);
        }
        _ => panic!("expected published article"),
    }
}
