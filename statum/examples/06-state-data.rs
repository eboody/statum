use statum::{machine, state, transition};

#[state]
enum State {
    //NOTE: we add a state data to the Draft state
    Draft(MyDraft),
    InReview,
    Published,
}

struct MyDraft {
    _title: String,
    _content: String,
}

#[machine]
struct Machine<State> {}



#[transition]
impl Machine<Draft> {
    pub fn into_in_review(self) -> Machine<InReview> {
        //NOTE: we can access the state's data with &self.state_data
        let my_draft_data_ref: &MyDraft = &self.state_data;

        println!(
            "This is us doing something with the reference to the draft data: {}",
            my_draft_data_ref._title
        );

        self.transition()
    }
}

fn main() {
    let my_draft = MyDraft {
        _title: "My first article".to_owned(),
        _content: "This is the content of my first article".to_owned(),
    };

    //NOTE: we build the machine with the state data
    let _machine = Machine::<Draft>::builder().state_data(my_draft).build();
}
