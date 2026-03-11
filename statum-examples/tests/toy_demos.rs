use statum_examples::toy_demos;

#[test]
fn example_01_setup() {
    toy_demos::example_01_setup::run();
}

#[test]
fn example_02_machine_context() {
    toy_demos::example_02_machine_context::run();
}

#[test]
fn example_03_derives() {
    toy_demos::example_03_derives::run();
}

#[test]
fn example_04_transitions() {
    toy_demos::example_04_transitions::run();
}

#[test]
fn example_05_split_transition() {
    toy_demos::example_05_split_transition::run();
}

#[tokio::test]
async fn example_06_async_transitions() {
    toy_demos::example_06_async_transitions::run().await;
}

#[test]
fn example_07_state_data() {
    toy_demos::example_07_state_data::run();
}

#[test]
fn example_08_transition_with_data() {
    toy_demos::example_08_transition_with_data::run();
}

#[tokio::test]
async fn example_09_persistent_data() {
    toy_demos::example_09_persistent_data::run().await;
}

#[tokio::test]
async fn example_10_persistent_data_vecs() {
    toy_demos::example_10_persistent_data_vecs::run().await;
}

#[test]
fn example_11_hierarchical_machines() {
    toy_demos::example_11_hierarchical_machines::run();
}

#[test]
fn example_12_rollbacks() {
    toy_demos::example_12_rollbacks::run();
}

#[test]
fn example_13_review_flow() {
    toy_demos::example_13_review_flow::run();
}

#[test]
fn example_14_batch_machine_fields() {
    toy_demos::example_14_batch_machine_fields::run();
}

#[test]
fn example_15_transition_map() {
    toy_demos::example_15_transition_map::run();
}
