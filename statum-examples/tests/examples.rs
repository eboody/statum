use statum_examples::examples;

#[test]
fn example_01_setup() {
    examples::example_01_setup::run();
}

#[test]
fn example_02_machine_context() {
    examples::example_02_machine_context::run();
}

#[test]
fn example_03_derives() {
    examples::example_03_derives::run();
}

#[test]
fn example_04_transitions() {
    examples::example_04_transitions::run();
}

#[test]
fn example_05_split_transition() {
    examples::example_05_split_transition::run();
}

#[tokio::test]
async fn example_06_async_transitions() {
    examples::example_06_async_transitions::run().await;
}

#[test]
fn example_07_state_data() {
    examples::example_07_state_data::run();
}

#[test]
fn example_08_transition_with_data() {
    examples::example_08_transition_with_data::run();
}

#[tokio::test]
async fn example_09_persistent_data() {
    examples::example_09_persistent_data::run().await;
}

#[tokio::test]
async fn example_10_persistent_data_vecs() {
    examples::example_10_persistent_data_vecs::run().await;
}

#[test]
fn example_11_hierarchical_machines() {
    examples::example_11_hierarchical_machines::run();
}

#[test]
fn example_12_rollbacks() {
    examples::example_12_rollbacks::run();
}

#[test]
fn example_13_review_flow() {
    examples::example_13_review_flow::run();
}
