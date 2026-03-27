use cargo_statum_graph::HeuristicRelationSource;

#[test]
fn heuristic_relation_source_is_exported_from_the_crate_root() {
    let source = HeuristicRelationSource::Method {
        machine: 1,
        state: 2,
        method_name: "await_task".to_owned(),
    };

    assert_eq!(source.machine(), 1);
    assert_eq!(source.state(), Some(2));
    assert_eq!(source.transition(), None);
    assert_eq!(source.method_name(), Some("await_task"));
    assert_eq!(source.kind_label(), "method");
}
