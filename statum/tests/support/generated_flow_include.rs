#[transition]
impl GeneratedFlow<MacroTarget> {
    #[present(label = "Via Include", metadata = "include")]
    fn via_include(self) -> GeneratedFlow<Included> {
        self.transition()
    }
}
