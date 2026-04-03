use std::marker::PhantomData;

struct Off;
struct On;

struct LightSwitch<State> {
    name: String,
    _state: PhantomData<State>,
}

impl<State> LightSwitch<State> {
    fn transition<Next>(self) -> LightSwitch<Next> {
        LightSwitch {
            name: self.name,
            _state: PhantomData,
        }
    }

    fn name(&self) -> &str {
        &self.name
    }
}

struct MissingName;
struct Ready;

trait NamePhase {
    type Storage;
}

impl NamePhase for MissingName {
    type Storage = ();
}

impl NamePhase for Ready {
    type Storage = String;
}

struct LightSwitchBuilder<NameState: NamePhase> {
    name: NameState::Storage,
    _name: PhantomData<NameState>,
}

impl LightSwitch<Off> {
    fn builder() -> LightSwitchBuilder<MissingName> {
        LightSwitchBuilder {
            name: (),
            _name: PhantomData,
        }
    }

    fn switch_on(self) -> LightSwitch<On> {
        self.transition()
    }
}

impl LightSwitch<On> {
    fn switch_off(self) -> LightSwitch<Off> {
        self.transition()
    }
}

impl LightSwitchBuilder<MissingName> {
    fn name(self, name: String) -> LightSwitchBuilder<Ready> {
        LightSwitchBuilder {
            name,
            _name: PhantomData,
        }
    }
}

impl LightSwitchBuilder<Ready> {
    fn build(self) -> LightSwitch<Off> {
        LightSwitch {
            name: self.name,
            _state: PhantomData,
        }
    }
}

fn main() {
    let light = LightSwitch::<Off>::builder()
        .name("desk lamp".to_owned())
        .build();

    println!("{} starts off", light.name());

    let light = light.switch_on();
    println!("{} is on", light.name());

    let _light = light.switch_off();
}
