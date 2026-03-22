#[transition]
impl FlowMachine<Start> {
    fn finish(self) -> FlowMachine<Done> {
        self.transition()
    }
}
