#[transition]
impl GeneratedFlow<MacroTarget> {
    fn via_include(self) -> GeneratedFlow<Included> {
        self.transition()
    }
}
