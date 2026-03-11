use statum::{machine, state, transition};

#[state]
enum TaskState {
    Draft,
    InReview(ReviewData),
    Approved,
}

#[derive(Clone, Debug)]
struct ReviewData {
    reviewer: String,
}

#[machine]
struct Task<TaskState> {
    id: u64,
}

#[transition]
impl Task<Draft> {
    fn submit_for_review(self, reviewer: String) -> Task<InReview> {
        let data = ReviewData { reviewer };
        self.transition_with(data)
    }
}

#[transition]
impl Task<InReview> {
    fn approve(self) -> Task<Approved> {
        self.transition()
    }
}

pub fn run() {
    let task = Task::<Draft>::builder().id(42).build();
    let task = task.submit_for_review("sam".to_string());

    let _reviewer = task.state_data.reviewer.as_str();
    let _task = task.approve();
}
