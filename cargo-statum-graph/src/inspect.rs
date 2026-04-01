use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::ffi::OsString;
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::rc::Rc;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Tabs, Wrap};
use ratatui::{Frame, Terminal};
use statum_graph::{
    codebase::render as codebase_render, CodebaseDoc, CodebaseMachine,
    CodebaseMachineRelationGroup, CodebaseMachineRelationGroupSemantic, CodebaseRelation,
    CodebaseRelationBasis, CodebaseRelationCount, CodebaseRelationDetail, CodebaseRelationKind,
    CodebaseRelationSource, CodebaseState, CodebaseTransition, CodebaseValidatorEntry,
};

use crate::heuristics::{
    HeuristicDiagnostic, HeuristicEvidenceKind, HeuristicMachineRelationGroup, HeuristicOverlay,
    HeuristicRelation, HeuristicRelationCount, HeuristicRelationDetail, HeuristicRelationSource,
    HeuristicStatusKind,
};
use crate::suggestions::{
    CompositionSuggestion, CompositionSuggestionOverlay, CompositionSuggestionSeverity,
};

pub fn run(
    doc: CodebaseDoc,
    heuristic: HeuristicOverlay,
    suggestions: CompositionSuggestionOverlay,
    workspace_label: String,
) -> io::Result<()> {
    let mut terminal = setup_terminal()?;
    let mut app = InspectorApp::new(doc, heuristic, suggestions, workspace_label);

    let result = (|| -> io::Result<()> {
        loop {
            terminal.draw(|frame| app.render(frame))?;
            if app.should_quit {
                return Ok(());
            }

            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => app.handle_key(key),
                _ => {}
            }
        }
    })();

    restore_terminal(&mut terminal)?;
    result
}

type InspectorTerminal = Terminal<CrosstermBackend<io::Stdout>>;

fn setup_terminal() -> io::Result<InspectorTerminal> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    Terminal::new(CrosstermBackend::new(stdout))
}

fn restore_terminal(terminal: &mut InspectorTerminal) -> io::Result<()> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Focus {
    Workspace,
    MainView,
    Detail,
}

impl Focus {
    fn next(self) -> Self {
        match self {
            Self::Workspace => Self::MainView,
            Self::MainView => Self::Detail,
            Self::Detail => Self::Workspace,
        }
    }

    fn previous(self) -> Self {
        match self {
            Self::Workspace => Self::Detail,
            Self::MainView => Self::Workspace,
            Self::Detail => Self::MainView,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WorkspaceSection {
    Composition,
    Machines,
    Gaps,
}

impl WorkspaceSection {
    fn label(self) -> &'static str {
        match self {
            Self::Composition => "Workspace",
            Self::Machines => "Machine",
            Self::Gaps => "Gaps",
        }
    }

    fn next(self, available: &[Self]) -> Self {
        let Some(current_index) = available.iter().position(|section| *section == self) else {
            return *available.first().unwrap_or(&Self::Machines);
        };
        available
            .get(current_index + 1)
            .copied()
            .unwrap_or_else(|| available[0])
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MachineSection {
    Overview,
    States,
    Transitions,
    Validators,
    Relations,
    Paths,
    Diagnostics,
}

impl MachineSection {
    fn label(self) -> &'static str {
        match self {
            Self::Overview => "Overview",
            Self::States => "States",
            Self::Transitions => "Transitions",
            Self::Validators => "Validators",
            Self::Relations => "Relations",
            Self::Paths => "Paths",
            Self::Diagnostics => "Diagnostics",
        }
    }

    fn next(self, available: &[Self]) -> Self {
        let Some(current_index) = available.iter().position(|section| *section == self) else {
            return *available.first().unwrap_or(&Self::Overview);
        };
        available
            .get(current_index + 1)
            .copied()
            .unwrap_or_else(|| available[0])
    }

    fn previous(self, available: &[Self]) -> Self {
        let Some(current_index) = available.iter().position(|section| *section == self) else {
            return *available.first().unwrap_or(&Self::Overview);
        };
        current_index
            .checked_sub(1)
            .and_then(|index| available.get(index))
            .copied()
            .unwrap_or_else(|| *available.last().unwrap_or(&Self::Overview))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DetailTab {
    Summary,
    Docs,
    Diagram,
    Source,
    Explain,
}

impl DetailTab {
    const ORDER: [Self; 5] = [
        Self::Summary,
        Self::Docs,
        Self::Diagram,
        Self::Source,
        Self::Explain,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::Summary => "Summary",
            Self::Docs => "Docs",
            Self::Diagram => "Diagram",
            Self::Source => "Source",
            Self::Explain => "Explain",
        }
    }

    fn next(self) -> Self {
        let current_index = Self::ORDER
            .iter()
            .position(|tab| *tab == self)
            .expect("detail tab should exist");
        Self::ORDER
            .get(current_index + 1)
            .copied()
            .unwrap_or(Self::ORDER[0])
    }

    fn previous(self) -> Self {
        let current_index = Self::ORDER
            .iter()
            .position(|tab| *tab == self)
            .expect("detail tab should exist");
        current_index
            .checked_sub(1)
            .and_then(|index| Self::ORDER.get(index))
            .copied()
            .unwrap_or(*Self::ORDER.last().expect("detail order should exist"))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SearchScope {
    Primary,
    Docs,
    Relations,
    Paths,
    All,
}

impl SearchScope {
    fn label(self) -> &'static str {
        match self {
            Self::Primary => "primary",
            Self::Docs => "docs",
            Self::Relations => "relations",
            Self::Paths => "paths",
            Self::All => "all",
        }
    }

    fn next(self) -> Self {
        match self {
            Self::Primary => Self::Docs,
            Self::Docs => Self::Relations,
            Self::Relations => Self::Paths,
            Self::Paths => Self::All,
            Self::All => Self::Primary,
        }
    }

    fn includes_primary(self) -> bool {
        matches!(self, Self::Primary | Self::All)
    }

    fn includes_docs(self) -> bool {
        matches!(self, Self::Docs | Self::All)
    }

    fn includes_relations(self) -> bool {
        matches!(self, Self::Relations | Self::All)
    }

    fn includes_paths(self) -> bool {
        matches!(self, Self::Paths | Self::All)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RelationContext {
    Machine,
    State(usize),
    Transition(usize),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RelationDirection {
    Outbound,
    Inbound,
}

impl RelationDirection {
    fn label(self) -> &'static str {
        match self {
            Self::Outbound => "Outbound",
            Self::Inbound => "Inbound",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LaneMode {
    Exact,
    Heuristic,
    Mixed,
}

impl LaneMode {
    fn label(self) -> &'static str {
        match self {
            Self::Exact => "exact",
            Self::Heuristic => "heuristic",
            Self::Mixed => "mixed",
        }
    }

    fn shows_exact(self) -> bool {
        matches!(self, Self::Exact | Self::Mixed)
    }

    fn shows_heuristic(self) -> bool {
        matches!(self, Self::Heuristic | Self::Mixed)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RelationSubject {
    Machine { machine: usize },
    State { machine: usize, state: usize },
    Transition { machine: usize, transition: usize },
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SummaryDirection {
    Outbound,
    Inbound,
}

#[cfg(test)]
#[derive(Clone, Debug, Eq, PartialEq)]
struct ExactSummaryItem {
    direction: SummaryDirection,
    group: CodebaseMachineRelationGroup,
}

#[cfg(test)]
#[derive(Clone, Debug, Eq, PartialEq)]
struct HeuristicSummaryItem {
    direction: SummaryDirection,
    group: HeuristicMachineRelationGroup,
}

#[cfg(test)]
#[derive(Clone, Debug, Eq, PartialEq)]
enum SummaryItem {
    Exact(ExactSummaryItem),
    Heuristic(HeuristicSummaryItem),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PathKind {
    Composition,
    Exact,
    Heuristic,
}

impl PathKind {
    fn display_label(self) -> &'static str {
        match self {
            Self::Composition => "composition",
            Self::Exact => "exact",
            Self::Heuristic => "heuristic",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PathStep {
    from_machine: usize,
    to_machine: usize,
    kind: PathKind,
    label: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PathItem {
    kind: PathKind,
    target_machine: usize,
    steps: Vec<PathStep>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum InputMode {
    Normal,
    Search,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct HeuristicFilters {
    evidence_kinds: BTreeSet<HeuristicEvidenceKind>,
}

impl HeuristicFilters {
    fn toggle_evidence_kind(&mut self, evidence_kind: HeuristicEvidenceKind) {
        if !self.evidence_kinds.insert(evidence_kind) {
            self.evidence_kinds.remove(&evidence_kind);
        }
    }

    fn clear(&mut self) {
        self.evidence_kinds.clear();
    }

    fn has_active(&self) -> bool {
        !self.evidence_kinds.is_empty()
    }

    fn matches_relation(&self, relation: &HeuristicRelation) -> bool {
        self.evidence_kinds.is_empty() || self.evidence_kinds.contains(&relation.evidence_kind)
    }

    fn evidence_summary(&self) -> String {
        if self.evidence_kinds.is_empty() {
            "all".to_owned()
        } else {
            self.evidence_kinds
                .iter()
                .map(|evidence_kind| evidence_kind.display_label())
                .collect::<Vec<_>>()
                .join(", ")
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct ExactFilters {
    relation_kinds: BTreeSet<CodebaseRelationKind>,
    relation_bases: BTreeSet<CodebaseRelationBasis>,
}

impl ExactFilters {
    fn toggle_kind(&mut self, kind: CodebaseRelationKind) {
        if !self.relation_kinds.insert(kind) {
            self.relation_kinds.remove(&kind);
        }
    }

    fn toggle_basis(&mut self, basis: CodebaseRelationBasis) {
        if !self.relation_bases.insert(basis) {
            self.relation_bases.remove(&basis);
        }
    }

    fn clear(&mut self) {
        self.relation_kinds.clear();
        self.relation_bases.clear();
    }

    fn has_active(&self) -> bool {
        !self.relation_kinds.is_empty() || !self.relation_bases.is_empty()
    }

    fn matches_relation(&self, relation: &CodebaseRelation) -> bool {
        (self.relation_kinds.is_empty() || self.relation_kinds.contains(&relation.kind))
            && (self.relation_bases.is_empty() || self.relation_bases.contains(&relation.basis))
    }

    fn kind_summary(&self) -> String {
        if self.relation_kinds.is_empty() {
            "all".to_owned()
        } else {
            self.relation_kinds
                .iter()
                .map(|kind| kind.display_label())
                .collect::<Vec<_>>()
                .join(", ")
        }
    }

    fn basis_summary(&self) -> String {
        if self.relation_bases.is_empty() {
            "all".to_owned()
        } else {
            self.relation_bases
                .iter()
                .map(|basis| basis.display_label())
                .collect::<Vec<_>>()
                .join(", ")
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum MachineItem {
    State(usize),
    Transition(usize),
    Validator(usize),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RelationItem {
    Exact(usize),
    Heuristic(usize),
}

#[derive(Debug)]
enum RelationDetailSelection<'a> {
    Exact(CodebaseRelationDetail<'a>),
    Heuristic {
        detail: HeuristicRelationDetail<'a>,
        shadowed_by_exact: bool,
    },
}

#[derive(Debug, Default)]
struct InspectorSessionCache {
    visible_machine_indices: Option<Rc<[usize]>>,
    visible_composition_machine_indices: Option<Rc<[usize]>>,
    visible_gap_indices: Option<Rc<[usize]>>,
    filtered_machine_relation_groups: Option<Rc<[CodebaseMachineRelationGroup]>>,
    filtered_heuristic_machine_relation_groups: Option<Rc<[HeuristicMachineRelationGroup]>>,
    visible_machine_relation_groups: Option<Rc<[CodebaseMachineRelationGroup]>>,
    visible_heuristic_machine_relation_groups: Option<Rc<[HeuristicMachineRelationGroup]>>,
    machine_items: Option<Rc<[MachineItem]>>,
    #[cfg(test)]
    summary_items: Option<Rc<[SummaryItem]>>,
    relation_items: Option<Rc<[RelationItem]>>,
    path_items: Option<Rc<[PathItem]>>,
    disconnected_group_count: Option<usize>,
    diagram_preview: Option<DiagramPreviewCache>,
}

impl InspectorSessionCache {
    fn clear(&mut self) {
        *self = Self::default();
    }
}

#[derive(Clone, Debug)]
struct DiagramPreviewCache {
    key: String,
    text: Text<'static>,
}

#[derive(Clone, Debug)]
struct DiagramPlan {
    key: String,
    title: String,
    kind_label: &'static str,
    exact: bool,
    source: Text<'static>,
}

#[derive(Debug)]
struct InspectorApp {
    doc: CodebaseDoc,
    heuristic: HeuristicOverlay,
    suggestions: CompositionSuggestionOverlay,
    workspace_label: String,
    workspace_section: WorkspaceSection,
    selected_machine: usize,
    selected_gap: usize,
    input_mode: InputMode,
    search_query: String,
    search_scope: SearchScope,
    filters: ExactFilters,
    heuristic_filters: HeuristicFilters,
    lane_mode: LaneMode,
    focus: Focus,
    machine_section: MachineSection,
    detail_tab: DetailTab,
    relation_context: RelationContext,
    machine_item_index: usize,
    relation_direction: RelationDirection,
    relation_index: usize,
    path_index: usize,
    diagram_scroll_y: u16,
    show_help: bool,
    should_quit: bool,
    cache: RefCell<InspectorSessionCache>,
}

impl InspectorApp {
    fn new(
        doc: CodebaseDoc,
        heuristic: HeuristicOverlay,
        suggestions: CompositionSuggestionOverlay,
        workspace_label: String,
    ) -> Self {
        let has_composition = doc
            .machines()
            .iter()
            .any(|machine| machine.role.is_composition());
        let workspace_section = if has_composition {
            WorkspaceSection::Composition
        } else {
            WorkspaceSection::Machines
        };
        let mut app = Self {
            doc,
            heuristic,
            suggestions,
            workspace_label,
            workspace_section,
            selected_machine: 0,
            selected_gap: 0,
            input_mode: InputMode::Normal,
            search_query: String::new(),
            search_scope: SearchScope::Primary,
            filters: ExactFilters::default(),
            heuristic_filters: HeuristicFilters::default(),
            lane_mode: LaneMode::Exact,
            focus: Focus::Workspace,
            machine_section: MachineSection::Overview,
            detail_tab: DetailTab::Summary,
            relation_context: RelationContext::Machine,
            machine_item_index: 0,
            relation_direction: RelationDirection::Outbound,
            relation_index: 0,
            path_index: 0,
            diagram_scroll_y: 0,
            show_help: false,
            should_quit: false,
            cache: RefCell::new(InspectorSessionCache::default()),
        };
        app.clamp_indices();
        if matches!(
            app.workspace_section,
            WorkspaceSection::Machines | WorkspaceSection::Composition
        ) {
            app.machine_section = MachineSection::Overview;
        }
        app
    }

    fn focus_label(&self) -> &'static str {
        match self.focus {
            Focus::Workspace => "outline",
            Focus::MainView => "center",
            Focus::Detail => "detail",
        }
    }

    fn available_workspace_sections(&self) -> Vec<WorkspaceSection> {
        let mut sections = Vec::new();
        if self.has_composition_machines() {
            sections.push(WorkspaceSection::Composition);
        }
        sections.push(WorkspaceSection::Machines);
        if !self.suggestions.is_empty() {
            sections.push(WorkspaceSection::Gaps);
        }
        sections
    }

    fn has_composition_machines(&self) -> bool {
        self.doc
            .machines()
            .iter()
            .any(|machine| machine.role.is_composition())
    }

    fn visible_gap_indices(&self) -> Rc<[usize]> {
        if let Some(cached) = self.cache.borrow().visible_gap_indices.clone() {
            return cached;
        }

        let query = self.normalized_query();
        let computed: Rc<[usize]> = self
            .suggestions
            .suggestions()
            .iter()
            .filter(|suggestion| self.suggestion_matches_query(suggestion, query.as_deref()))
            .map(|suggestion| suggestion.index)
            .collect::<Vec<_>>()
            .into();
        self.cache.borrow_mut().visible_gap_indices = Some(computed.clone());
        computed
    }

    fn current_gap(&self) -> Option<&CompositionSuggestion> {
        let visible = self.visible_gap_indices();
        let gap_index = visible
            .iter()
            .find(|gap_index| **gap_index == self.selected_gap)
            .copied()
            .or_else(|| visible.first().copied())?;
        self.suggestions.suggestions().get(gap_index)
    }

    fn select_gap(&mut self, gap_index: usize) {
        self.selected_gap = gap_index;
        self.path_index = 0;
        self.diagram_scroll_y = 0;
        self.invalidate_cache();
    }

    fn select_machine(&mut self, machine_index: usize) {
        self.selected_machine = machine_index;
        self.machine_section = MachineSection::Overview;
        self.relation_context = RelationContext::Machine;
        self.machine_item_index = 0;
        self.relation_index = 0;
        self.path_index = 0;
        self.diagram_scroll_y = 0;
        self.invalidate_cache();
    }

    fn current_machine(&self) -> Option<&CodebaseMachine> {
        let visible_machines = self.visible_workspace_machine_indices();
        let machine_index = visible_machines
            .iter()
            .find(|machine_index| **machine_index == self.selected_machine)
            .copied()
            .or_else(|| visible_machines.first().copied())?;
        self.doc.machine(machine_index)
    }

    fn visible_workspace_machine_indices(&self) -> Rc<[usize]> {
        match self.workspace_section {
            WorkspaceSection::Composition => self.visible_composition_machine_indices(),
            WorkspaceSection::Machines => self.visible_machine_indices(),
            WorkspaceSection::Gaps => Rc::from(Vec::new()),
        }
    }

    fn visible_machine_indices(&self) -> Rc<[usize]> {
        if let Some(cached) = self.cache.borrow().visible_machine_indices.clone() {
            return cached;
        }

        let query = self.normalized_query();
        let computed: Rc<[usize]> = self
            .doc
            .machines()
            .iter()
            .filter(|machine| self.machine_matches_query(machine, query.as_deref()))
            .map(|machine| machine.index)
            .collect::<Vec<_>>()
            .into();
        self.cache.borrow_mut().visible_machine_indices = Some(computed.clone());
        computed
    }

    fn visible_composition_machine_indices(&self) -> Rc<[usize]> {
        if let Some(cached) = self
            .cache
            .borrow()
            .visible_composition_machine_indices
            .clone()
        {
            return cached;
        }

        let query = self.normalized_query();
        let computed: Rc<[usize]> = self
            .doc
            .machines()
            .iter()
            .filter(|machine| machine.role.is_composition())
            .filter(|machine| self.machine_matches_query(machine, query.as_deref()))
            .map(|machine| machine.index)
            .collect::<Vec<_>>()
            .into();
        self.cache.borrow_mut().visible_composition_machine_indices = Some(computed.clone());
        computed
    }

    fn machine_suggestions(&self, machine_index: usize) -> Vec<&CompositionSuggestion> {
        self.suggestions
            .machine_suggestions(machine_index)
            .collect()
    }

    fn composition_diagnostic_counts(&self) -> (usize, usize) {
        (
            self.suggestions.warning_count(),
            self.suggestions.suggestion_count(),
        )
    }

    fn invalidate_cache(&mut self) {
        self.cache.get_mut().clear();
    }

    fn available_machine_sections(&self) -> &'static [MachineSection] {
        match self.workspace_section {
            WorkspaceSection::Composition => &[
                MachineSection::Overview,
                MachineSection::States,
                MachineSection::Transitions,
                MachineSection::Validators,
                MachineSection::Paths,
                MachineSection::Diagnostics,
            ],
            WorkspaceSection::Machines => &[
                MachineSection::Overview,
                MachineSection::States,
                MachineSection::Transitions,
                MachineSection::Validators,
                MachineSection::Relations,
                MachineSection::Paths,
                MachineSection::Diagnostics,
            ],
            WorkspaceSection::Gaps => &[MachineSection::Overview],
        }
    }

    fn capture_relation_context(&mut self) {
        self.relation_context = match self.machine_section {
            MachineSection::States => match self.selected_machine_item() {
                Some(MachineItem::State(state)) => RelationContext::State(state),
                _ => RelationContext::Machine,
            },
            MachineSection::Transitions => match self.selected_machine_item() {
                Some(MachineItem::Transition(transition)) => {
                    RelationContext::Transition(transition)
                }
                _ => RelationContext::Machine,
            },
            _ => self.relation_context,
        };
    }

    fn set_lane_mode(&mut self, lane_mode: LaneMode) {
        self.lane_mode = lane_mode;
        self.relation_index = 0;
        self.path_index = 0;
        self.diagram_scroll_y = 0;
    }

    fn reset_diagram_scroll(&mut self) {
        self.diagram_scroll_y = 0;
    }

    fn is_workspace_home(&self) -> bool {
        self.workspace_section == WorkspaceSection::Composition
            && self.machine_section == MachineSection::Overview
    }
}

impl InspectorApp {
    fn handle_key(&mut self, key: KeyEvent) {
        if self.show_help {
            match key.code {
                KeyCode::Esc | KeyCode::Char('?') => {
                    self.show_help = false;
                    return;
                }
                _ => return,
            }
        }

        if self.input_mode == InputMode::Search {
            self.handle_search_key(key);
            self.invalidate_cache();
            self.clamp_indices();
            return;
        }

        match key.code {
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Esc => {
                if self.focus != Focus::Workspace {
                    self.focus = Focus::Workspace;
                } else if self.has_search_query() {
                    self.search_query.clear();
                }
            }
            KeyCode::Char('?') => self.show_help = true,
            KeyCode::Char('/') => self.input_mode = InputMode::Search,
            KeyCode::Char('1') if self.has_composition_machines() => {
                self.workspace_section = WorkspaceSection::Composition;
                self.machine_section = MachineSection::Overview;
                self.reset_diagram_scroll();
            }
            KeyCode::Char('2') => {
                self.workspace_section = WorkspaceSection::Machines;
                self.machine_section = MachineSection::Overview;
                self.reset_diagram_scroll();
            }
            KeyCode::Char('3') if !self.suggestions.is_empty() => {
                self.workspace_section = WorkspaceSection::Gaps;
                self.reset_diagram_scroll();
            }
            KeyCode::Char('e') => self.set_lane_mode(LaneMode::Exact),
            KeyCode::Char('m') => self.set_lane_mode(LaneMode::Mixed),
            KeyCode::Char('h') => self.set_lane_mode(LaneMode::Heuristic),
            KeyCode::Char('0') => {
                self.filters.clear();
                self.heuristic_filters.clear();
            }
            KeyCode::Char('p') => self.filters.toggle_kind(CodebaseRelationKind::StatePayload),
            KeyCode::Char('f') => self.filters.toggle_kind(CodebaseRelationKind::MachineField),
            KeyCode::Char('t') => self
                .filters
                .toggle_kind(CodebaseRelationKind::TransitionParam),
            KeyCode::Char('d') => self
                .filters
                .toggle_basis(CodebaseRelationBasis::DirectTypeSyntax),
            KeyCode::Char('n') => self
                .filters
                .toggle_basis(CodebaseRelationBasis::DeclaredReferenceType),
            KeyCode::Char('s') => {
                self.search_scope = self.search_scope.next();
                self.machine_item_index = 0;
                self.relation_index = 0;
                self.path_index = 0;
            }
            KeyCode::Char('g') => self
                .heuristic_filters
                .toggle_evidence_kind(HeuristicEvidenceKind::Signature),
            KeyCode::Char('b') => self
                .heuristic_filters
                .toggle_evidence_kind(HeuristicEvidenceKind::Body),
            KeyCode::Char('w') => {
                self.workspace_section = self
                    .workspace_section
                    .next(&self.available_workspace_sections());
                self.machine_section = MachineSection::Overview;
                self.relation_context = RelationContext::Machine;
                self.reset_diagram_scroll();
            }
            KeyCode::Tab => self.focus = self.focus.next(),
            KeyCode::BackTab => self.focus = self.focus.previous(),
            KeyCode::Left | KeyCode::Char('[') => self.move_left(),
            KeyCode::Right | KeyCode::Char(']') => self.move_right(),
            KeyCode::Char('i') if self.machine_section == MachineSection::Relations => {
                self.relation_direction = RelationDirection::Inbound;
                self.relation_index = 0;
            }
            KeyCode::Char('o') if self.machine_section == MachineSection::Relations => {
                self.relation_direction = RelationDirection::Outbound;
                self.relation_index = 0;
            }
            KeyCode::Up | KeyCode::Char('k') => self.move_up(),
            KeyCode::Down | KeyCode::Char('j') => self.move_down(),
            _ => {}
        }

        self.invalidate_cache();
        self.clamp_indices();
    }

    fn handle_search_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc | KeyCode::Enter => self.input_mode = InputMode::Normal,
            KeyCode::Backspace => {
                self.search_query.pop();
            }
            KeyCode::Char(ch)
                if !key
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
            {
                self.search_query.push(ch);
            }
            _ => {}
        }
    }

    fn move_left(&mut self) {
        match self.focus {
            Focus::Workspace => self.focus = self.focus.previous(),
            Focus::MainView => {
                self.capture_relation_context();
                self.machine_section = self
                    .machine_section
                    .previous(self.available_machine_sections());
                self.machine_item_index = 0;
                self.relation_index = 0;
                self.path_index = 0;
                self.reset_diagram_scroll();
            }
            Focus::Detail => {
                self.detail_tab = self.detail_tab.previous();
            }
        }
    }

    fn move_right(&mut self) {
        match self.focus {
            Focus::Workspace => self.focus = self.focus.next(),
            Focus::MainView => {
                self.capture_relation_context();
                self.machine_section = self.machine_section.next(self.available_machine_sections());
                self.machine_item_index = 0;
                self.relation_index = 0;
                self.path_index = 0;
                self.reset_diagram_scroll();
            }
            Focus::Detail => {
                self.detail_tab = self.detail_tab.next();
            }
        }
    }

    fn move_up(&mut self) {
        match self.focus {
            Focus::Workspace => match self.workspace_section {
                WorkspaceSection::Composition | WorkspaceSection::Machines => {
                    let visible = self.visible_workspace_machine_indices();
                    let Some(current_position) = visible
                        .iter()
                        .position(|machine_index| *machine_index == self.selected_machine)
                    else {
                        if let Some(&first) = visible.first() {
                            self.select_machine(first);
                        }
                        return;
                    };
                    if current_position > 0 {
                        self.select_machine(visible[current_position - 1]);
                    }
                }
                WorkspaceSection::Gaps => {
                    let visible = self.visible_gap_indices();
                    let Some(current_position) = visible
                        .iter()
                        .position(|gap_index| *gap_index == self.selected_gap)
                    else {
                        if let Some(&first) = visible.first() {
                            self.select_gap(first);
                        }
                        return;
                    };
                    if current_position > 0 {
                        self.select_gap(visible[current_position - 1]);
                    }
                }
            },
            Focus::MainView => match self.workspace_section {
                WorkspaceSection::Composition | WorkspaceSection::Machines => {
                    match self.machine_section {
                        MachineSection::States
                        | MachineSection::Transitions
                        | MachineSection::Validators => {
                            self.machine_item_index = self.machine_item_index.saturating_sub(1);
                        }
                        MachineSection::Relations => {
                            self.relation_index = self.relation_index.saturating_sub(1);
                        }
                        MachineSection::Paths => {
                            self.path_index = self.path_index.saturating_sub(1);
                        }
                        MachineSection::Overview => {
                            self.diagram_scroll_y = self.diagram_scroll_y.saturating_sub(1);
                        }
                        MachineSection::Diagnostics => {}
                    }
                }
                WorkspaceSection::Gaps => {}
            },
            Focus::Detail => {}
        }
    }

    fn move_down(&mut self) {
        match self.focus {
            Focus::Workspace => match self.workspace_section {
                WorkspaceSection::Composition | WorkspaceSection::Machines => {
                    let visible = self.visible_workspace_machine_indices();
                    let Some(current_position) = visible
                        .iter()
                        .position(|machine_index| *machine_index == self.selected_machine)
                    else {
                        if let Some(&first) = visible.first() {
                            self.select_machine(first);
                        }
                        return;
                    };
                    if let Some(&next) = visible.get(current_position + 1) {
                        self.select_machine(next);
                    }
                }
                WorkspaceSection::Gaps => {
                    let visible = self.visible_gap_indices();
                    let Some(current_position) = visible
                        .iter()
                        .position(|gap_index| *gap_index == self.selected_gap)
                    else {
                        if let Some(&first) = visible.first() {
                            self.select_gap(first);
                        }
                        return;
                    };
                    if let Some(&next) = visible.get(current_position + 1) {
                        self.select_gap(next);
                    }
                }
            },
            Focus::MainView => match self.workspace_section {
                WorkspaceSection::Composition | WorkspaceSection::Machines => {
                    match self.machine_section {
                        MachineSection::States
                        | MachineSection::Transitions
                        | MachineSection::Validators => {
                            self.machine_item_index = self.machine_item_index.saturating_add(1);
                        }
                        MachineSection::Relations => {
                            self.relation_index = self.relation_index.saturating_add(1);
                        }
                        MachineSection::Paths => {
                            self.path_index = self.path_index.saturating_add(1);
                        }
                        MachineSection::Overview => {
                            self.diagram_scroll_y = self.diagram_scroll_y.saturating_add(1);
                        }
                        MachineSection::Diagnostics => {}
                    }
                }
                WorkspaceSection::Gaps => {}
            },
            Focus::Detail => {}
        }
    }

    fn clamp_indices(&mut self) {
        self.invalidate_cache();
        match self.workspace_section {
            WorkspaceSection::Composition | WorkspaceSection::Machines => {
                let visible_machines = self.visible_workspace_machine_indices();
                if visible_machines.is_empty() {
                    self.selected_machine = 0;
                    self.machine_item_index = 0;
                    self.relation_index = 0;
                    self.path_index = 0;
                    return;
                }

                if !visible_machines.contains(&self.selected_machine) {
                    self.select_machine(visible_machines[0]);
                }
                if !self
                    .available_machine_sections()
                    .contains(&self.machine_section)
                {
                    self.machine_section = MachineSection::Overview;
                }
                match self.machine_section {
                    MachineSection::States
                    | MachineSection::Transitions
                    | MachineSection::Validators => {
                        self.machine_item_index = self
                            .machine_item_index
                            .min(self.machine_items().len().saturating_sub(1));
                    }
                    MachineSection::Relations => {
                        self.relation_index = self
                            .relation_index
                            .min(self.relation_items().len().saturating_sub(1));
                    }
                    MachineSection::Paths => {
                        self.path_index = self
                            .path_index
                            .min(self.path_items().len().saturating_sub(1));
                    }
                    MachineSection::Overview | MachineSection::Diagnostics => {}
                }
            }
            WorkspaceSection::Gaps => {
                let visible_gaps = self.visible_gap_indices();
                if visible_gaps.is_empty() {
                    self.selected_gap = 0;
                    self.path_index = 0;
                    return;
                }

                if !visible_gaps.contains(&self.selected_gap) {
                    self.select_gap(visible_gaps[0]);
                }
                self.path_index = self
                    .path_index
                    .min(self.path_items().len().saturating_sub(1));
            }
        }
    }

    fn machine_visible_summary_counts(&self, machine_index: usize) -> (usize, usize) {
        let exact = if self.lane_mode.shows_exact() {
            self.filtered_machine_relation_groups()
                .iter()
                .filter(|group| {
                    group.from_machine != group.to_machine
                        && (group.from_machine == machine_index
                            || group.to_machine == machine_index)
                })
                .count()
        } else {
            0
        };
        let heuristic = if self.lane_mode.shows_heuristic() {
            self.filtered_heuristic_machine_relation_groups()
                .iter()
                .filter(|group| {
                    group.from_machine != group.to_machine
                        && (group.from_machine == machine_index
                            || group.to_machine == machine_index)
                })
                .count()
        } else {
            0
        };
        (exact, heuristic)
    }

    fn machine_has_any_heuristic_summary(&self, machine_index: usize) -> bool {
        self.heuristic
            .machine_relation_groups()
            .iter()
            .any(|group| {
                group.from_machine != group.to_machine
                    && (group.from_machine == machine_index || group.to_machine == machine_index)
            })
    }

    fn machine_section_label(&self, machine: &CodebaseMachine, section: MachineSection) -> String {
        match section {
            MachineSection::Overview => match self.workspace_section {
                WorkspaceSection::Composition => "Workspace".to_owned(),
                WorkspaceSection::Machines => "Diagram".to_owned(),
                WorkspaceSection::Gaps => "Overview".to_owned(),
            },
            MachineSection::States => format!("States ({})", machine.states.len()),
            MachineSection::Transitions => {
                format!("Transitions ({})", machine.transitions.len())
            }
            MachineSection::Validators => {
                format!("Validators ({})", machine.validator_entries.len())
            }
            MachineSection::Relations => {
                let exact = self
                    .exact_relation_items(
                        RelationSubject::Machine {
                            machine: machine.index,
                        },
                        None,
                    )
                    .len();
                let heuristic = self
                    .heuristic_relation_items(
                        RelationSubject::Machine {
                            machine: machine.index,
                        },
                        None,
                    )
                    .len();
                match self.lane_mode {
                    LaneMode::Exact => format!("Relations ({exact})"),
                    LaneMode::Heuristic => format!("Relations ({heuristic})"),
                    LaneMode::Mixed => format!("Relations ({exact}e/{heuristic}h)"),
                }
            }
            MachineSection::Paths => format!("Paths ({})", self.path_items().len()),
            MachineSection::Diagnostics => {
                format!(
                    "Diagnostics ({})",
                    self.machine_suggestions(machine.index).len()
                )
            }
        }
    }

    fn machine_items(&self) -> Rc<[MachineItem]> {
        if let Some(cached) = self.cache.borrow().machine_items.clone() {
            return cached;
        }

        let Some(machine) = self.current_machine() else {
            return Rc::from(Vec::new());
        };
        let query = self.normalized_query();
        let computed: Rc<[MachineItem]> = match self.machine_section {
            MachineSection::States => machine
                .states
                .iter()
                .filter(|state| self.state_matches_query(state, query.as_deref()))
                .map(|state| MachineItem::State(state.index))
                .collect::<Vec<_>>()
                .into(),
            MachineSection::Transitions => machine
                .transitions
                .iter()
                .filter(|transition| self.transition_matches_query(transition, query.as_deref()))
                .map(|transition| MachineItem::Transition(transition.index))
                .collect::<Vec<_>>()
                .into(),
            MachineSection::Validators => machine
                .validator_entries
                .iter()
                .filter(|entry| self.validator_matches_query(entry, query.as_deref()))
                .map(|entry| MachineItem::Validator(entry.index))
                .collect::<Vec<_>>()
                .into(),
            MachineSection::Overview
            | MachineSection::Relations
            | MachineSection::Paths
            | MachineSection::Diagnostics => Rc::from(Vec::new()),
        };
        self.cache.borrow_mut().machine_items = Some(computed.clone());
        computed
    }

    #[cfg(test)]
    fn machine_item_label(&self, machine: &CodebaseMachine, item: &MachineItem) -> String {
        match item {
            MachineItem::State(state_index) => machine
                .state(*state_index)
                .map(render_state_label)
                .unwrap_or_else(|| "<missing state>".to_owned()),
            MachineItem::Transition(transition_index) => machine
                .transition(*transition_index)
                .map(|transition| render_transition_label(transition).to_owned())
                .unwrap_or_else(|| "<missing transition>".to_owned()),
            MachineItem::Validator(entry_index) => machine
                .validator_entry(*entry_index)
                .map(|entry| entry.display_label().into_owned())
                .unwrap_or_else(|| "<missing validator>".to_owned()),
        }
    }

    fn path_item_label(&self, item: &PathItem) -> String {
        let target = self
            .doc
            .machine(item.target_machine)
            .map(render_machine_label)
            .unwrap_or("<missing machine>");
        format!(
            "[{}] {} ({} hop{})",
            item.kind.display_label(),
            target,
            item.steps.len(),
            if item.steps.len() == 1 { "" } else { "s" }
        )
    }

    #[cfg(test)]
    fn summary_items(&self) -> Rc<[SummaryItem]> {
        if let Some(cached) = self.cache.borrow().summary_items.clone() {
            return cached;
        }

        let Some(machine) = self.current_machine() else {
            return Rc::from(Vec::new());
        };
        let mut items = Vec::new();

        if self.lane_mode.shows_exact() {
            items.extend(
                self.filtered_machine_relation_groups()
                    .iter()
                    .filter_map(|group| {
                        if group.from_machine == machine.index
                            && group.from_machine != group.to_machine
                        {
                            Some(SummaryItem::Exact(ExactSummaryItem {
                                direction: SummaryDirection::Outbound,
                                group: group.clone(),
                            }))
                        } else if group.to_machine == machine.index
                            && group.from_machine != group.to_machine
                        {
                            Some(SummaryItem::Exact(ExactSummaryItem {
                                direction: SummaryDirection::Inbound,
                                group: group.clone(),
                            }))
                        } else {
                            None
                        }
                    }),
            );
        }

        if self.lane_mode.shows_heuristic() {
            items.extend(
                self.filtered_heuristic_machine_relation_groups()
                    .iter()
                    .filter_map(|group| {
                        if group.from_machine == machine.index
                            && group.from_machine != group.to_machine
                        {
                            Some(SummaryItem::Heuristic(HeuristicSummaryItem {
                                direction: SummaryDirection::Outbound,
                                group: group.clone(),
                            }))
                        } else if group.to_machine == machine.index
                            && group.from_machine != group.to_machine
                        {
                            Some(SummaryItem::Heuristic(HeuristicSummaryItem {
                                direction: SummaryDirection::Inbound,
                                group: group.clone(),
                            }))
                        } else {
                            None
                        }
                    }),
            );
        }

        let computed: Rc<[SummaryItem]> = items.into();
        self.cache.borrow_mut().summary_items = Some(computed.clone());
        computed
    }

    fn selected_machine_item(&self) -> Option<MachineItem> {
        self.machine_items().get(self.machine_item_index).cloned()
    }

    fn relation_subject(&self) -> Option<RelationSubject> {
        let machine = self.current_machine()?;
        match self.machine_section {
            MachineSection::States => match self.selected_machine_item() {
                Some(MachineItem::State(state)) => Some(RelationSubject::State {
                    machine: machine.index,
                    state,
                }),
                _ => Some(RelationSubject::Machine {
                    machine: machine.index,
                }),
            },
            MachineSection::Transitions => match self.selected_machine_item() {
                Some(MachineItem::Transition(transition)) => Some(RelationSubject::Transition {
                    machine: machine.index,
                    transition,
                }),
                _ => Some(RelationSubject::Machine {
                    machine: machine.index,
                }),
            },
            MachineSection::Relations => match self.relation_context {
                RelationContext::Machine => Some(RelationSubject::Machine {
                    machine: machine.index,
                }),
                RelationContext::State(state) => Some(RelationSubject::State {
                    machine: machine.index,
                    state,
                }),
                RelationContext::Transition(transition) => Some(RelationSubject::Transition {
                    machine: machine.index,
                    transition,
                }),
            },
            MachineSection::Overview
            | MachineSection::Validators
            | MachineSection::Paths
            | MachineSection::Diagnostics => Some(RelationSubject::Machine {
                machine: machine.index,
            }),
        }
    }

    fn path_items(&self) -> Rc<[PathItem]> {
        if let Some(cached) = self.cache.borrow().path_items.clone() {
            return cached;
        }

        let query = self.normalized_query();
        let computed: Rc<[PathItem]> = match self.workspace_section {
            WorkspaceSection::Composition => self
                .current_machine()
                .map(|machine| self.path_items_from_source(machine.index, None, query.as_deref()))
                .unwrap_or_default()
                .into(),
            WorkspaceSection::Machines => self
                .current_machine()
                .map(|machine| self.path_items_from_source(machine.index, None, query.as_deref()))
                .unwrap_or_default()
                .into(),
            WorkspaceSection::Gaps => self
                .current_gap()
                .map(|gap| {
                    self.path_items_from_source(gap.source_machine, Some(gap.target_machine), None)
                })
                .unwrap_or_default()
                .into(),
        };
        self.cache.borrow_mut().path_items = Some(computed.clone());
        computed
    }

    fn selected_path_item(&self) -> Option<PathItem> {
        self.path_items().get(self.path_index).cloned()
    }

    fn path_items_from_source(
        &self,
        source_machine: usize,
        target_filter: Option<usize>,
        query: Option<&str>,
    ) -> Vec<PathItem> {
        let composition_edges = self.path_edges(true, false);
        let exact_edges = self.path_edges(false, false);
        let combined_edges = self.path_edges(false, true);
        let mut seen_targets = BTreeSet::new();
        let mut items = Vec::new();

        for (kind, edges) in [
            (PathKind::Composition, &composition_edges),
            (PathKind::Exact, &exact_edges),
        ] {
            for (target_machine, steps) in discover_paths(source_machine, edges) {
                if target_machine == source_machine
                    || seen_targets.contains(&target_machine)
                    || target_filter.is_some_and(|target| target != target_machine)
                {
                    continue;
                }
                let item = PathItem {
                    kind,
                    target_machine,
                    steps,
                };
                if self.path_item_matches_query(source_machine, &item, query) {
                    seen_targets.insert(target_machine);
                    items.push(item);
                }
            }
        }

        if self.lane_mode.shows_heuristic() {
            for (target_machine, steps) in discover_paths(source_machine, &combined_edges) {
                if target_machine == source_machine
                    || seen_targets.contains(&target_machine)
                    || target_filter.is_some_and(|target| target != target_machine)
                {
                    continue;
                }
                let item = PathItem {
                    kind: PathKind::Heuristic,
                    target_machine,
                    steps,
                };
                if self.path_item_matches_query(source_machine, &item, query) {
                    seen_targets.insert(target_machine);
                    items.push(item);
                }
            }
        }

        items.sort_by_key(|item| {
            (
                match item.kind {
                    PathKind::Composition => 0usize,
                    PathKind::Exact => 1,
                    PathKind::Heuristic => 2,
                },
                self.doc
                    .machine(item.target_machine)
                    .map(|machine| render_machine_label(machine).to_owned())
                    .unwrap_or_default(),
            )
        });
        items
    }

    fn path_edges(
        &self,
        composition_only: bool,
        include_heuristic: bool,
    ) -> BTreeMap<usize, Vec<PathStep>> {
        let mut adjacency = BTreeMap::<usize, Vec<PathStep>>::new();

        for group in self.filtered_machine_relation_groups().iter() {
            if group.from_machine == group.to_machine {
                continue;
            }

            let is_composition = group.semantic != CodebaseMachineRelationGroupSemantic::Exact;
            if composition_only && !is_composition {
                continue;
            }

            adjacency
                .entry(group.from_machine)
                .or_default()
                .push(PathStep {
                    from_machine: group.from_machine,
                    to_machine: group.to_machine,
                    kind: if is_composition {
                        PathKind::Composition
                    } else {
                        PathKind::Exact
                    },
                    label: group.display_label(),
                });
        }

        if include_heuristic {
            for group in self.filtered_heuristic_machine_relation_groups().iter() {
                if group.from_machine == group.to_machine {
                    continue;
                }

                adjacency
                    .entry(group.from_machine)
                    .or_default()
                    .push(PathStep {
                        from_machine: group.from_machine,
                        to_machine: group.to_machine,
                        kind: PathKind::Heuristic,
                        label: group.display_label(),
                    });
            }
        }

        for steps in adjacency.values_mut() {
            steps.sort_by_key(|step| (step.to_machine, step.label.clone()));
        }

        adjacency
    }

    fn path_item_matches_query(
        &self,
        source_machine: usize,
        item: &PathItem,
        query: Option<&str>,
    ) -> bool {
        if !self.search_scope.includes_paths() {
            return true;
        }

        let source = self
            .doc
            .machine(source_machine)
            .map(render_machine_label)
            .unwrap_or("<missing machine>");
        let target = self
            .doc
            .machine(item.target_machine)
            .map(render_machine_label)
            .unwrap_or("<missing machine>");
        let mut candidates = vec![
            item.kind.display_label().to_owned(),
            source.to_owned(),
            target.to_owned(),
            format!("{} hop", item.steps.len()),
        ];
        for step in &item.steps {
            candidates.push(step.kind.display_label().to_owned());
            candidates.push(step.label.clone());
            if let Some(machine) = self.doc.machine(step.to_machine) {
                candidates.push(render_machine_label(machine).to_owned());
            }
        }
        Self::query_matches_any(query, candidates)
    }

    fn relation_items(&self) -> Rc<[RelationItem]> {
        if let Some(cached) = self.cache.borrow().relation_items.clone() {
            return cached;
        }

        let query = self.normalized_query();
        let Some(subject) = self.relation_subject() else {
            return Rc::from(Vec::new());
        };

        let mut items = Vec::new();
        if self.lane_mode.shows_exact() {
            items.extend(
                self.exact_relation_items(subject, query.as_deref())
                    .into_iter()
                    .map(RelationItem::Exact),
            );
        }
        if self.lane_mode.shows_heuristic() {
            items.extend(
                self.heuristic_relation_items(subject, query.as_deref())
                    .into_iter()
                    .map(RelationItem::Heuristic),
            );
        }
        let computed: Rc<[RelationItem]> = items.into();
        self.cache.borrow_mut().relation_items = Some(computed.clone());
        computed
    }

    fn filtered_machine_relation_groups(&self) -> Rc<[CodebaseMachineRelationGroup]> {
        if let Some(cached) = self.cache.borrow().filtered_machine_relation_groups.clone() {
            return cached;
        }

        let mut filtered = Vec::new();
        for group in self.doc.machine_relation_groups() {
            let relation_indices = group
                .relation_indices
                .iter()
                .copied()
                .filter(|relation_index| {
                    self.doc
                        .relation(*relation_index)
                        .is_some_and(|relation| self.filters.matches_relation(relation))
                })
                .collect::<Vec<_>>();
            if relation_indices.is_empty() {
                continue;
            }

            let mut counts =
                BTreeMap::<(CodebaseRelationKind, CodebaseRelationBasis), usize>::new();
            let mut composition_owned_relations = 0usize;
            for relation_index in &relation_indices {
                let relation = self
                    .doc
                    .relation(*relation_index)
                    .expect("filtered relation index should resolve");
                *counts.entry((relation.kind, relation.basis)).or_default() += 1;
                if relation.is_composition_owned() {
                    composition_owned_relations += 1;
                }
            }

            filtered.push(CodebaseMachineRelationGroup {
                index: filtered.len(),
                from_machine: group.from_machine,
                to_machine: group.to_machine,
                semantic: classify_group_semantic(
                    composition_owned_relations,
                    relation_indices.len(),
                ),
                relation_indices,
                counts: counts
                    .into_iter()
                    .map(|((kind, basis), count)| CodebaseRelationCount { kind, basis, count })
                    .collect(),
            });
        }
        let computed: Rc<[CodebaseMachineRelationGroup]> = filtered.into();
        self.cache.borrow_mut().filtered_machine_relation_groups = Some(computed.clone());
        computed
    }

    fn filtered_heuristic_machine_relation_groups(&self) -> Rc<[HeuristicMachineRelationGroup]> {
        if let Some(cached) = self
            .cache
            .borrow()
            .filtered_heuristic_machine_relation_groups
            .clone()
        {
            return cached;
        }

        let mut filtered = Vec::new();
        for group in self.heuristic.machine_relation_groups() {
            let relation_indices = group
                .relation_indices
                .iter()
                .copied()
                .filter(|relation_index| {
                    self.heuristic
                        .relation(*relation_index)
                        .is_some_and(|relation| {
                            self.heuristic_filters.matches_relation(relation)
                                && !self.should_hide_shadowed_heuristic_relation(relation)
                        })
                })
                .collect::<Vec<_>>();
            if relation_indices.is_empty() {
                continue;
            }

            let mut counts = BTreeMap::<HeuristicEvidenceKind, usize>::new();
            for relation_index in &relation_indices {
                let relation = self
                    .heuristic
                    .relation(*relation_index)
                    .expect("filtered heuristic relation index should resolve");
                *counts.entry(relation.evidence_kind).or_default() += 1;
            }

            filtered.push(HeuristicMachineRelationGroup {
                index: filtered.len(),
                from_machine: group.from_machine,
                to_machine: group.to_machine,
                relation_indices,
                counts: counts
                    .into_iter()
                    .map(|(evidence_kind, count)| HeuristicRelationCount {
                        evidence_kind,
                        count,
                    })
                    .collect(),
            });
        }
        let computed: Rc<[HeuristicMachineRelationGroup]> = filtered.into();
        self.cache
            .borrow_mut()
            .filtered_heuristic_machine_relation_groups = Some(computed.clone());
        computed
    }

    fn should_hide_shadowed_heuristic_relation(&self, relation: &HeuristicRelation) -> bool {
        self.lane_mode == LaneMode::Mixed
            && self.heuristic_relation_has_visible_exact_cover(relation)
    }

    fn heuristic_relation_has_visible_exact_cover(&self, relation: &HeuristicRelation) -> bool {
        self.doc
            .relations()
            .iter()
            .filter(|exact| self.filters.matches_relation(exact))
            .any(|exact| exact_covers_heuristic_relation(exact, relation))
    }

    fn heuristic_relation_has_exact_cover(&self, relation: &HeuristicRelation) -> bool {
        self.doc
            .relations()
            .iter()
            .any(|exact| exact_covers_heuristic_relation(exact, relation))
    }

    fn visible_machine_relation_groups(&self) -> Rc<[CodebaseMachineRelationGroup]> {
        if let Some(cached) = self.cache.borrow().visible_machine_relation_groups.clone() {
            return cached;
        }

        let visible_machine_indices = self
            .visible_machine_indices()
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        let computed: Rc<[CodebaseMachineRelationGroup]> = self
            .filtered_machine_relation_groups()
            .iter()
            .filter(|group| {
                visible_machine_indices.contains(&group.from_machine)
                    && visible_machine_indices.contains(&group.to_machine)
            })
            .cloned()
            .collect::<Vec<_>>()
            .into();
        self.cache.borrow_mut().visible_machine_relation_groups = Some(computed.clone());
        computed
    }

    fn visible_heuristic_machine_relation_groups(&self) -> Rc<[HeuristicMachineRelationGroup]> {
        if let Some(cached) = self
            .cache
            .borrow()
            .visible_heuristic_machine_relation_groups
            .clone()
        {
            return cached;
        }

        let visible_machine_indices = self
            .visible_machine_indices()
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        let computed: Rc<[HeuristicMachineRelationGroup]> = self
            .filtered_heuristic_machine_relation_groups()
            .iter()
            .filter(|group| {
                visible_machine_indices.contains(&group.from_machine)
                    && visible_machine_indices.contains(&group.to_machine)
            })
            .cloned()
            .collect::<Vec<_>>()
            .into();
        self.cache
            .borrow_mut()
            .visible_heuristic_machine_relation_groups = Some(computed.clone());
        computed
    }

    fn selected_relation_detail(&self) -> Option<RelationDetailSelection<'_>> {
        match self.relation_items().get(self.relation_index).copied()? {
            RelationItem::Exact(index) => self
                .doc
                .relation_detail(index)
                .map(RelationDetailSelection::Exact),
            RelationItem::Heuristic(index) => {
                self.heuristic
                    .relation_detail(&self.doc, index)
                    .map(|detail| RelationDetailSelection::Heuristic {
                        detail,
                        shadowed_by_exact: self.heuristic_relation_has_exact_cover(detail.relation),
                    })
            }
        }
    }

    fn exact_relation_items(&self, subject: RelationSubject, query: Option<&str>) -> Vec<usize> {
        let base: Vec<&CodebaseRelation> = match (subject, self.relation_direction) {
            (RelationSubject::Machine { machine }, RelationDirection::Outbound) => {
                self.doc.outbound_relations_for_machine(machine).collect()
            }
            (RelationSubject::Machine { machine }, RelationDirection::Inbound) => {
                self.doc.inbound_relations_for_machine(machine).collect()
            }
            (RelationSubject::State { machine, state }, RelationDirection::Outbound) => self
                .doc
                .outbound_relations_for_state(machine, state)
                .collect(),
            (RelationSubject::State { machine, state }, RelationDirection::Inbound) => self
                .doc
                .inbound_relations_for_state(machine, state)
                .collect(),
            (
                RelationSubject::Transition {
                    machine,
                    transition,
                },
                RelationDirection::Outbound,
            ) => self
                .doc
                .outbound_relations_for_transition(machine, transition)
                .collect(),
            (
                RelationSubject::Transition {
                    machine,
                    transition,
                },
                RelationDirection::Inbound,
            ) => self
                .doc
                .inbound_relations_for_transition(machine, transition)
                .collect(),
        };

        base.into_iter()
            .filter(|relation| self.filters.matches_relation(relation))
            .filter_map(|relation| {
                self.doc
                    .relation_detail(relation.index)
                    .filter(|detail| self.relation_matches_query(detail, query))
                    .map(|_| relation.index)
            })
            .collect()
    }

    fn heuristic_relation_items(
        &self,
        subject: RelationSubject,
        query: Option<&str>,
    ) -> Vec<usize> {
        let base: Vec<&HeuristicRelation> = match (subject, self.relation_direction) {
            (RelationSubject::Machine { machine }, RelationDirection::Outbound) => self
                .heuristic
                .outbound_relations_for_machine(machine)
                .collect(),
            (RelationSubject::Machine { machine }, RelationDirection::Inbound) => self
                .heuristic
                .inbound_relations_for_machine(machine)
                .collect(),
            (RelationSubject::State { machine, state }, RelationDirection::Outbound) => self
                .heuristic
                .outbound_relations_for_state(machine, state)
                .collect(),
            (RelationSubject::State { .. }, RelationDirection::Inbound) => Vec::new(),
            (
                RelationSubject::Transition {
                    machine,
                    transition,
                },
                RelationDirection::Outbound,
            ) => self
                .heuristic
                .outbound_relations_for_transition(machine, transition)
                .collect(),
            (
                RelationSubject::Transition {
                    machine,
                    transition,
                },
                RelationDirection::Inbound,
            ) => self
                .heuristic
                .inbound_relations_for_transition(machine, transition)
                .collect(),
        };

        base.into_iter()
            .filter(|relation| self.heuristic_filters.matches_relation(relation))
            .filter(|relation| !self.should_hide_shadowed_heuristic_relation(relation))
            .filter_map(|relation| {
                self.heuristic
                    .relation_detail(&self.doc, relation.index)
                    .filter(|detail| self.heuristic_relation_matches_query(detail, query))
                    .map(|_| relation.index)
            })
            .collect()
    }

    fn disconnected_group_count(&self) -> usize {
        if let Some(cached) = self.cache.borrow().disconnected_group_count {
            return cached;
        }

        let visible_machine_indices = self.visible_machine_indices();
        if visible_machine_indices.is_empty() {
            return 0;
        }

        let mut adjacency = vec![Vec::new(); self.doc.machines().len()];
        if self.lane_mode.shows_exact() {
            for group in self.visible_machine_relation_groups().iter() {
                if group.from_machine == group.to_machine {
                    continue;
                }
                adjacency[group.from_machine].push(group.to_machine);
                adjacency[group.to_machine].push(group.from_machine);
            }
        }
        if self.lane_mode.shows_heuristic() {
            for group in self.visible_heuristic_machine_relation_groups().iter() {
                if group.from_machine == group.to_machine {
                    continue;
                }
                adjacency[group.from_machine].push(group.to_machine);
                adjacency[group.to_machine].push(group.from_machine);
            }
        }

        let mut seen = vec![false; self.doc.machines().len()];
        let mut groups = 0;
        for machine_index in visible_machine_indices.iter().copied() {
            if seen[machine_index] {
                continue;
            }
            groups += 1;
            let mut stack = vec![machine_index];
            seen[machine_index] = true;
            while let Some(current) = stack.pop() {
                for &next in &adjacency[current] {
                    if !seen[next] {
                        seen[next] = true;
                        stack.push(next);
                    }
                }
            }
        }

        self.cache.borrow_mut().disconnected_group_count = Some(groups);
        groups
    }

    fn render(&self, frame: &mut Frame) {
        let vertical = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Length(4)])
            .split(frame.area());
        let horizontal = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(28),
                Constraint::Min(68),
                Constraint::Length(42),
            ])
            .split(vertical[0]);

        self.render_workspace(frame, horizontal[0]);
        self.render_center_view(frame, horizontal[1]);
        self.render_detail(frame, horizontal[2]);
        self.render_status(frame, vertical[1]);
        if self.show_help {
            self.render_help_overlay(frame);
        }
    }

    fn render_workspace(&self, frame: &mut Frame, area: Rect) {
        let accent = workspace_section_accent(self.workspace_section);
        let block = titled_block(
            Line::from(vec![
                badge("OUTLINE", Color::Black, accent),
                Span::raw(" "),
                Span::styled(
                    format!("Workspace {}", self.workspace_label),
                    title_style(accent, self.focus == Focus::Workspace),
                ),
            ]),
            accent,
            self.focus == Focus::Workspace,
        );
        let inner = block.inner(area);
        frame.render_widget(block, area);

        let sections = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Length(10),
                Constraint::Min(0),
            ])
            .split(inner);

        let available_sections = self.available_workspace_sections();
        let tabs = Tabs::new(
            available_sections
                .iter()
                .map(|section| Line::from(section.label()))
                .collect::<Vec<_>>(),
        )
        .select(
            available_sections
                .iter()
                .position(|section| *section == self.workspace_section)
                .unwrap_or(0),
        )
        .highlight_style(Style::default().fg(accent).add_modifier(Modifier::BOLD));
        frame.render_widget(tabs, sections[0]);

        let visible_machine_indices = self.visible_machine_indices();
        let visible_machine_count = visible_machine_indices.len();
        let total_machine_count = self.doc.machines().len();
        let visible_composition_indices = self.visible_composition_machine_indices();
        let visible_composition_count = visible_composition_indices.len();
        let total_composition_count = self
            .doc
            .machines()
            .iter()
            .filter(|machine| machine.role.is_composition())
            .count();
        let visible_gap_indices = self.visible_gap_indices();
        let visible_gap_count = visible_gap_indices.len();
        let total_gap_count = self.suggestions.suggestions().len();
        let visible_exact_summary_edges = self
            .visible_machine_relation_groups()
            .iter()
            .filter(|group| group.from_machine != group.to_machine)
            .count();
        let visible_heuristic_summary_edges = self
            .visible_heuristic_machine_relation_groups()
            .iter()
            .filter(|group| group.from_machine != group.to_machine)
            .count();
        let search_status = if self.has_search_query() {
            format!("/{}", self.search_query.trim())
        } else {
            "<none>".to_owned()
        };

        match self.workspace_section {
            WorkspaceSection::Composition => self.render_workspace_summary_cards(
                frame,
                sections[1],
                [
                    (
                        "flows:",
                        format!("{visible_composition_count}/{total_composition_count}"),
                        "composition".to_owned(),
                        accent,
                    ),
                    (
                        "machines:",
                        format!("{visible_machine_count}/{total_machine_count}"),
                        "reachable".to_owned(),
                        accent,
                    ),
                    (
                        "exact summary edges:",
                        visible_exact_summary_edges.to_string(),
                        format!("heuristic {visible_heuristic_summary_edges}"),
                        lane_accent(LaneMode::Exact),
                    ),
                    (
                        "search:",
                        search_status,
                        format!("lane {}", self.lane_mode.label()),
                        lane_accent(self.lane_mode),
                    ),
                ],
            ),
            WorkspaceSection::Machines => self.render_workspace_summary_cards(
                frame,
                sections[1],
                [
                    (
                        "machines:",
                        format!("{visible_machine_count}/{total_machine_count}"),
                        "inventory".to_owned(),
                        accent,
                    ),
                    (
                        "groups:",
                        self.disconnected_group_count().to_string(),
                        "connected sets".to_owned(),
                        accent,
                    ),
                    (
                        "exact summary edges:",
                        visible_exact_summary_edges.to_string(),
                        format!("heuristic {visible_heuristic_summary_edges}"),
                        lane_accent(LaneMode::Exact),
                    ),
                    (
                        "search:",
                        search_status,
                        format!("lane {}", self.lane_mode.label()),
                        lane_accent(self.lane_mode),
                    ),
                ],
            ),
            WorkspaceSection::Gaps => self.render_workspace_summary_cards(
                frame,
                sections[1],
                [
                    (
                        "gaps:",
                        format!("{visible_gap_count}/{total_gap_count}"),
                        "open diagnostics".to_owned(),
                        accent,
                    ),
                    (
                        "warnings:",
                        self.suggestions.warning_count().to_string(),
                        "exact typed".to_owned(),
                        severity_accent(CompositionSuggestionSeverity::Warning),
                    ),
                    (
                        "suggestions:",
                        self.suggestions.suggestion_count().to_string(),
                        "heuristic".to_owned(),
                        severity_accent(CompositionSuggestionSeverity::Suggestion),
                    ),
                    (
                        "search:",
                        search_status,
                        format!("lane {}", self.lane_mode.label()),
                        lane_accent(self.lane_mode),
                    ),
                ],
            ),
        }

        let (items, selected) = match self.workspace_section {
            WorkspaceSection::Composition => (
                visible_composition_indices
                    .iter()
                    .filter_map(|machine_index| self.doc.machine(*machine_index))
                    .map(|machine| self.workspace_machine_list_item(machine))
                    .collect::<Vec<_>>(),
                visible_composition_indices
                    .iter()
                    .position(|machine_index| *machine_index == self.selected_machine),
            ),
            WorkspaceSection::Machines => (
                visible_machine_indices
                    .iter()
                    .filter_map(|machine_index| self.doc.machine(*machine_index))
                    .map(|machine| self.workspace_machine_list_item(machine))
                    .collect::<Vec<_>>(),
                visible_machine_indices
                    .iter()
                    .position(|machine_index| *machine_index == self.selected_machine),
            ),
            WorkspaceSection::Gaps => (
                visible_gap_indices
                    .iter()
                    .filter_map(|gap_index| self.suggestions.suggestions().get(*gap_index))
                    .map(|suggestion| self.workspace_gap_list_item(suggestion))
                    .collect::<Vec<_>>(),
                visible_gap_indices
                    .iter()
                    .position(|gap_index| *gap_index == self.selected_gap),
            ),
        };
        let mut state = ListState::default().with_selected(selected);
        let list = if items.is_empty() {
            List::new(vec![ListItem::new("<no matches>")])
        } else {
            List::new(items)
        }
        .highlight_style(selected_list_style(accent))
        .highlight_symbol(">> ");
        frame.render_stateful_widget(list, sections[2], &mut state);
    }

    fn render_center_view(&self, frame: &mut Frame, area: Rect) {
        let accent = workspace_section_accent(self.workspace_section);
        let block = titled_block(
            Line::from(vec![
                badge(
                    self.workspace_section.label().to_ascii_uppercase(),
                    Color::Black,
                    accent,
                ),
                Span::raw(" "),
                Span::styled(
                    self.center_title(),
                    title_style(accent, self.focus == Focus::MainView),
                ),
            ]),
            accent,
            self.focus == Focus::MainView,
        );
        let inner = block.inner(area);
        frame.render_widget(block, area);

        if self.workspace_section == WorkspaceSection::Gaps {
            self.render_gap_center_content(frame, inner);
            return;
        }

        let sections = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(0)])
            .split(inner);

        let available_sections = self.available_machine_sections();
        let tabs = Tabs::new(
            available_sections
                .iter()
                .map(|section| {
                    self.current_machine()
                        .map(|machine| Line::from(self.machine_section_label(machine, *section)))
                        .unwrap_or_else(|| Line::from(section.label()))
                })
                .collect::<Vec<_>>(),
        )
        .select(
            available_sections
                .iter()
                .position(|section| *section == self.machine_section)
                .unwrap_or(0),
        )
        .highlight_style(Style::default().fg(accent).add_modifier(Modifier::BOLD));
        frame.render_widget(tabs, sections[0]);

        match self.machine_section {
            MachineSection::Overview => self.render_overview_content(frame, sections[1]),
            MachineSection::States | MachineSection::Transitions | MachineSection::Validators => {
                self.render_machine_items_content(frame, sections[1]);
            }
            MachineSection::Relations => self.render_relations_content(frame, sections[1]),
            MachineSection::Paths => self.render_paths_content(frame, sections[1]),
            MachineSection::Diagnostics => self.render_diagnostics_content(frame, sections[1]),
        }
    }

    fn center_title(&self) -> String {
        match self.workspace_section {
            WorkspaceSection::Composition | WorkspaceSection::Machines
                if self.machine_section == MachineSection::Overview =>
            {
                self.center_diagram_plan().title
            }
            WorkspaceSection::Composition => self
                .current_machine()
                .map(|machine| format!("Workspace {}", render_machine_label(machine)))
                .unwrap_or_else(|| "Workspace <no matches>".to_owned()),
            WorkspaceSection::Machines => self
                .current_machine()
                .map(|machine| format!("Machine {}", render_machine_label(machine)))
                .unwrap_or_else(|| "Machine <no matches>".to_owned()),
            WorkspaceSection::Gaps => self
                .current_gap()
                .map(|gap| format!("Gap {}", gap.summary_label(&self.doc)))
                .unwrap_or_else(|| "Gap <no matches>".to_owned()),
        }
    }

    fn render_workspace_summary_cards(
        &self,
        frame: &mut Frame,
        area: Rect,
        cards: [(&str, String, String, Color); 4],
    ) {
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Length(3)])
            .split(area);
        for (row, chunk) in rows.into_iter().zip(cards.as_slice().chunks(2)) {
            let cols = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                .split(*row);
            for (col, card) in cols.into_iter().zip(chunk.iter()) {
                render_metric_card(frame, *col, card.0, card.1.clone(), card.2.clone(), card.3);
            }
        }
    }

    fn workspace_machine_list_item(&self, machine: &CodebaseMachine) -> ListItem<'static> {
        let accent = workspace_section_accent(self.workspace_section);
        let (exact, heuristic) = self.machine_visible_summary_counts(machine.index);
        let paths = self.path_items_from_source(machine.index, None, None).len();
        let warnings = self
            .machine_suggestions(machine.index)
            .into_iter()
            .filter(|suggestion| suggestion.severity == CompositionSuggestionSeverity::Warning)
            .count();
        let mut spans = vec![
            badge(
                machine.role.display_label().to_ascii_uppercase(),
                Color::Black,
                accent,
            ),
            Span::raw(" "),
            Span::styled(
                render_machine_label(machine).to_owned(),
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
            ghost_badge(format!("{exact}e"), lane_accent(LaneMode::Exact)),
            Span::raw(" "),
            ghost_badge(format!("{heuristic}h"), lane_accent(LaneMode::Heuristic)),
            Span::raw(" "),
            ghost_badge(format!("{paths}p"), lane_accent(LaneMode::Mixed)),
        ];
        if warnings > 0 {
            spans.push(Span::raw(" "));
            spans.push(ghost_badge(
                format!("{warnings} warn"),
                severity_accent(CompositionSuggestionSeverity::Warning),
            ));
        }
        let mut lines = vec![
            Line::from(spans),
            subdued_line(
                format!(
                    "{} state{}  {} transition{}  {} validator{}",
                    machine.states.len(),
                    plural_suffix(machine.states.len()),
                    machine.transitions.len(),
                    plural_suffix(machine.transitions.len()),
                    machine.validator_entries.len(),
                    plural_suffix(machine.validator_entries.len())
                ),
                accent,
            ),
        ];
        push_match_reason_line(&mut lines, self.machine_search_reason(machine));
        ListItem::new(Text::from(lines))
    }

    fn workspace_gap_list_item(&self, suggestion: &CompositionSuggestion) -> ListItem<'static> {
        let source = suggestion
            .source_machine(&self.doc)
            .map(render_machine_label)
            .unwrap_or("<missing machine>");
        let target = suggestion
            .target_machine(&self.doc)
            .map(render_machine_label)
            .unwrap_or("<missing machine>");
        let mut lines = vec![
            Line::from(vec![
                badge(
                    suggestion.severity.display_label().to_ascii_uppercase(),
                    Color::Black,
                    severity_accent(suggestion.severity),
                ),
                Span::raw(" "),
                Span::styled(
                    suggestion.summary_label(&self.doc),
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(" "),
                ghost_badge(
                    suggestion.counts_label(),
                    workspace_section_accent(WorkspaceSection::Gaps),
                ),
            ]),
            subdued_line(format!("{source} -> {target}"), muted_color()),
        ];
        push_match_reason_line(&mut lines, self.suggestion_search_reason(suggestion));
        ListItem::new(Text::from(lines))
    }

    fn render_overview_content(&self, frame: &mut Frame, area: Rect) {
        let plan = self.center_diagram_plan();
        self.render_diagram_viewport(frame, area, &plan);
    }

    fn render_diagnostics_content(&self, frame: &mut Frame, area: Rect) {
        let accent = workspace_section_accent(self.workspace_section);
        let split = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(66), Constraint::Percentage(34)])
            .split(area);
        let left = Block::default()
            .title(Line::from(vec![
                badge("SIGNALS", Color::Black, accent),
                Span::raw(" "),
                Span::styled("Composition Diagnostics", title_style(accent, true)),
            ]))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(accent));
        let left_inner = left.inner(split[0]);
        frame.render_widget(left, split[0]);
        frame.render_widget(
            Paragraph::new(self.center_diagnostics_text()).wrap(Wrap { trim: false }),
            left_inner,
        );

        let right_accent = detail_tab_accent(DetailTab::Explain);
        let right = Block::default()
            .title(Line::from(vec![
                badge("STATE", Color::Black, right_accent),
                Span::raw(" "),
                Span::styled("Overlay Status", title_style(right_accent, true)),
            ]))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(right_accent));
        let right_inner = right.inner(split[1]);
        frame.render_widget(right, split[1]);
        frame.render_widget(
            Paragraph::new(Text::from(vec![
                Line::from(format!(
                    "heuristics: {}",
                    self.heuristic.status().display_label()
                )),
                Line::from(format!(
                    "diagnostics: {}",
                    self.heuristic.diagnostics().len()
                )),
                Line::from(format!("warnings: {}", self.suggestions.warning_count())),
                Line::from(format!(
                    "suggestions: {}",
                    self.suggestions.suggestion_count()
                )),
            ]))
            .wrap(Wrap { trim: false }),
            right_inner,
        );
    }

    fn machine_item_list_item(
        &self,
        machine: &CodebaseMachine,
        item: &MachineItem,
    ) -> ListItem<'static> {
        let accent = workspace_section_accent(self.workspace_section);
        let mut lines = match item {
            MachineItem::State(state_index) => {
                let state = machine
                    .state(*state_index)
                    .expect("state list item should resolve");
                let mut spans = vec![
                    ghost_badge("state", accent),
                    Span::raw(" "),
                    Span::styled(
                        render_state_label(state),
                        Style::default()
                            .fg(Color::White)
                            .add_modifier(Modifier::BOLD),
                    ),
                ];
                if state.has_data {
                    spans.push(Span::raw(" "));
                    spans.push(ghost_badge("data", detail_tab_accent(DetailTab::Docs)));
                }
                if state.direct_construction_available {
                    spans.push(Span::raw(" "));
                    spans.push(ghost_badge("build", lane_accent(LaneMode::Exact)));
                }
                if state.is_graph_root {
                    spans.push(Span::raw(" "));
                    spans.push(ghost_badge("root", lane_accent(LaneMode::Mixed)));
                }
                vec![
                    Line::from(spans),
                    subdued_line(
                        first_text_excerpt(
                            state.description,
                            state.docs,
                            &format!("rust state {}", state.rust_name),
                        ),
                        accent,
                    ),
                ]
            }
            MachineItem::Transition(transition_index) => {
                let transition = machine
                    .transition(*transition_index)
                    .expect("transition list item should resolve");
                vec![
                    Line::from(vec![
                        ghost_badge("transition", accent),
                        Span::raw(" "),
                        Span::styled(
                            render_transition_label(transition).to_owned(),
                            Style::default()
                                .fg(Color::White)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::raw(" "),
                        ghost_badge(
                            format!("{} target", transition.to.len()),
                            lane_accent(LaneMode::Exact),
                        ),
                    ]),
                    subdued_line(
                        format!(
                            "from {} -> {}",
                            machine
                                .state(transition.from)
                                .map(render_state_label)
                                .unwrap_or_else(|| format!("state {}", transition.from)),
                            transition_target_summary(machine, transition)
                        ),
                        accent,
                    ),
                ]
            }
            MachineItem::Validator(entry_index) => {
                let entry = machine
                    .validator_entry(*entry_index)
                    .expect("validator list item should resolve");
                vec![
                    Line::from(vec![
                        ghost_badge("validator", accent),
                        Span::raw(" "),
                        Span::styled(
                            entry.display_label().into_owned(),
                            Style::default()
                                .fg(Color::White)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::raw(" "),
                        ghost_badge(
                            format!("{} state", entry.target_states.len()),
                            detail_tab_accent(DetailTab::Source),
                        ),
                    ]),
                    subdued_line(
                        format!(
                            "{} -> {}",
                            entry.source_type_display,
                            validator_target_summary(machine, entry)
                        ),
                        accent,
                    ),
                ]
            }
        };
        push_match_reason_line(&mut lines, self.machine_item_search_reason(machine, item));
        ListItem::new(Text::from(lines))
    }

    fn relation_list_item(&self, relation: RelationItem) -> ListItem<'static> {
        let mut lines = match relation {
            RelationItem::Exact(index) => self
                .doc
                .relation_detail(index)
                .map(|detail| {
                    let mut spans = vec![
                        badge("EXACT", Color::Black, lane_accent(LaneMode::Exact)),
                        Span::raw(" "),
                    ];
                    if detail.relation.is_composition_owned() {
                        spans.push(ghost_badge(
                            "composition",
                            workspace_section_accent(WorkspaceSection::Composition),
                        ));
                        spans.push(Span::raw(" "));
                    }
                    spans.push(Span::styled(
                        render_machine_label(detail.target_machine).to_owned(),
                        Style::default()
                            .fg(Color::White)
                            .add_modifier(Modifier::BOLD),
                    ));
                    spans.push(Span::raw(" "));
                    spans.push(ghost_badge(
                        detail.relation.kind.display_label(),
                        lane_accent(LaneMode::Exact),
                    ));
                    spans.push(Span::raw(" "));
                    spans.push(Span::raw(render_state_label(detail.target_state)));
                    vec![
                        Line::from(spans),
                        subdued_line(
                            format!(
                                "{}  |  {}  |  {}",
                                relation_origin_label(&detail),
                                exact_relation_source_label(detail.relation.source),
                                detail.relation.basis.display_label()
                            ),
                            lane_accent(LaneMode::Exact),
                        ),
                    ]
                })
                .unwrap_or_else(|| vec![Line::from("[exact] <missing relation>")]),
            RelationItem::Heuristic(index) => self
                .heuristic
                .relation_detail(&self.doc, index)
                .map(|detail| {
                    vec![
                        Line::from(vec![
                            badge("HEUR", Color::Black, lane_accent(LaneMode::Heuristic)),
                            Span::raw(" "),
                            ghost_badge(
                                detail.relation.evidence_kind.display_label(),
                                lane_accent(LaneMode::Heuristic),
                            ),
                            Span::raw(" "),
                            Span::styled(
                                render_machine_label(detail.target_machine).to_owned(),
                                Style::default()
                                    .fg(Color::White)
                                    .add_modifier(Modifier::BOLD),
                            ),
                            Span::raw(" "),
                            Span::raw(detail.relation.matched_path_text.clone()),
                        ]),
                        subdued_line(
                            format!(
                                "{}  |  {}:{}",
                                render_heuristic_source_label(&detail),
                                compact_file_label(&detail.relation.file_path),
                                detail.relation.line_number
                            ),
                            lane_accent(LaneMode::Heuristic),
                        ),
                    ]
                })
                .unwrap_or_else(|| vec![Line::from("[heur] <missing relation>")]),
        };
        push_match_reason_line(&mut lines, self.relation_search_reason(relation));
        ListItem::new(Text::from(lines))
    }

    fn path_list_item(&self, item: &PathItem) -> ListItem<'static> {
        let accent = match item.kind {
            PathKind::Composition => workspace_section_accent(WorkspaceSection::Composition),
            PathKind::Exact => lane_accent(LaneMode::Exact),
            PathKind::Heuristic => lane_accent(LaneMode::Heuristic),
        };
        let target = self
            .doc
            .machine(item.target_machine)
            .map(render_machine_label)
            .unwrap_or("<missing machine>");
        let mut lines = vec![
            Line::from(vec![
                badge(
                    item.kind.display_label().to_ascii_uppercase(),
                    Color::Black,
                    accent,
                ),
                Span::raw(" "),
                Span::styled(
                    target.to_owned(),
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(" "),
                ghost_badge(format!("{} hop", item.steps.len()), accent),
            ]),
            subdued_line(path_step_preview(item, &self.doc), accent),
        ];
        push_match_reason_line(&mut lines, self.path_search_reason(item));
        ListItem::new(Text::from(lines))
    }

    fn current_selection_label(&self) -> String {
        if self.is_workspace_home() {
            return format!("Workspace {}", self.workspace_label);
        }
        match self.workspace_section {
            WorkspaceSection::Composition | WorkspaceSection::Machines => {
                let Some(machine) = self.current_machine() else {
                    return "<no matches>".to_owned();
                };
                match self.machine_section {
                    MachineSection::Overview | MachineSection::Diagnostics => {
                        render_machine_label(machine).to_owned()
                    }
                    MachineSection::States => match self.selected_machine_item() {
                        Some(MachineItem::State(state_index)) => machine
                            .state(state_index)
                            .map(render_state_label)
                            .unwrap_or_else(|| render_machine_label(machine).to_owned()),
                        _ => render_machine_label(machine).to_owned(),
                    },
                    MachineSection::Transitions => match self.selected_machine_item() {
                        Some(MachineItem::Transition(transition_index)) => machine
                            .transition(transition_index)
                            .map(|transition| render_transition_label(transition).to_owned())
                            .unwrap_or_else(|| render_machine_label(machine).to_owned()),
                        _ => render_machine_label(machine).to_owned(),
                    },
                    MachineSection::Validators => match self.selected_machine_item() {
                        Some(MachineItem::Validator(entry_index)) => machine
                            .validator_entry(entry_index)
                            .map(|entry| entry.display_label().into_owned())
                            .unwrap_or_else(|| render_machine_label(machine).to_owned()),
                        _ => render_machine_label(machine).to_owned(),
                    },
                    MachineSection::Relations => self
                        .selected_relation_detail()
                        .map(|selection| match selection {
                            RelationDetailSelection::Exact(detail) => {
                                render_relation_label(&detail)
                            }
                            RelationDetailSelection::Heuristic { detail, .. } => {
                                render_heuristic_relation_label(&detail)
                            }
                        })
                        .unwrap_or_else(|| render_machine_label(machine).to_owned()),
                    MachineSection::Paths => self
                        .selected_path_item()
                        .map(|path| self.path_item_label(&path))
                        .unwrap_or_else(|| render_machine_label(machine).to_owned()),
                }
            }
            WorkspaceSection::Gaps => self
                .current_gap()
                .map(|gap| gap.summary_label(&self.doc))
                .unwrap_or_else(|| "<no matches>".to_owned()),
        }
    }

    fn detail_header_text(&self) -> Text<'static> {
        let section_accent = workspace_section_accent(self.workspace_section);
        let tab_accent = detail_tab_accent(self.detail_tab);
        let mut lines = vec![Line::from(vec![
            badge(
                self.workspace_section.label().to_ascii_uppercase(),
                Color::Black,
                section_accent,
            ),
            Span::raw(" "),
            badge(
                self.detail_tab.label().to_ascii_uppercase(),
                Color::Black,
                tab_accent,
            ),
            Span::raw(" "),
            ghost_badge(self.lane_mode.label(), lane_accent(self.lane_mode)),
        ])];
        lines.push(Line::from(Span::styled(
            self.current_selection_label(),
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )));
        if self.has_search_query() {
            lines.push(Line::from(format!(
                "scope {}  query /{}",
                self.search_scope.label(),
                self.search_query.trim()
            )));
        }
        Text::from(lines)
    }

    fn render_detail_body(&self, frame: &mut Frame, area: Rect) {
        let accent = detail_tab_accent(self.detail_tab);
        let (badge_label, title, notes_title, preferred_head_lines) = match self.detail_tab {
            DetailTab::Summary => ("SNAPSHOT", "Snapshot", "Breakdown", 6usize),
            DetailTab::Docs => ("HIGHLIGHTS", "Highlights", "Source-Local Docs", 5usize),
            DetailTab::Diagram => ("TERMAID", "Diagram Preview", "Availability", 10usize),
            DetailTab::Source => ("SURFACE", "Observed Surface", "Locations", 5usize),
            DetailTab::Explain => ("READING", "Interpretation", "Why It Matters", 4usize),
        };
        if self.detail_tab == DetailTab::Diagram {
            render_text_card(
                frame,
                area,
                badge_label,
                title,
                accent,
                self.current_diagram_preview_text(area.width.saturating_sub(2)),
            );
            return;
        }
        let (head, tail) = split_text_for_cards(self.detail_text_for_tab(), preferred_head_lines);
        if text_is_empty(&tail) || area.height < 8 {
            render_text_card(frame, area, badge_label, title, accent, head);
            return;
        }

        let head_height = (head.lines.len() as u16 + 2)
            .min(area.height.saturating_sub(3))
            .max(4);
        let sections = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(head_height), Constraint::Min(0)])
            .split(area);
        render_text_card(frame, sections[0], badge_label, title, accent, head);
        render_text_card(
            frame,
            sections[1],
            "NOTES",
            notes_title,
            muted_color(),
            tail,
        );
    }

    fn render_machine_items_content(&self, frame: &mut Frame, area: Rect) {
        let items = self.machine_items();
        let visible_items = self
            .current_machine()
            .map(|machine| {
                items
                    .iter()
                    .map(|item| self.machine_item_list_item(machine, item))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let empty = items.is_empty();
        let mut state =
            ListState::default().with_selected((!empty).then_some(self.machine_item_index));
        let list = if empty {
            List::new(vec![ListItem::new(self.empty_list_label())])
        } else {
            List::new(visible_items)
        }
        .highlight_style(selected_list_style(workspace_section_accent(
            self.workspace_section,
        )))
        .highlight_symbol(">> ");
        frame.render_stateful_widget(list, area, &mut state);
    }

    fn render_relations_content(&self, frame: &mut Frame, area: Rect) {
        let sections = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Min(0),
            ])
            .split(area);
        let accent = lane_accent(self.lane_mode);
        let tabs = Tabs::new(
            [RelationDirection::Outbound, RelationDirection::Inbound]
                .into_iter()
                .map(|direction| Line::from(direction.label()))
                .collect::<Vec<_>>(),
        )
        .select(match self.relation_direction {
            RelationDirection::Outbound => 0,
            RelationDirection::Inbound => 1,
        })
        .highlight_style(Style::default().fg(accent).add_modifier(Modifier::BOLD));
        frame.render_widget(tabs, sections[0]);
        frame.render_widget(
            Paragraph::new(Text::from(Line::from(vec![
                badge(
                    self.lane_mode.label().to_ascii_uppercase(),
                    Color::Black,
                    accent,
                ),
                Span::raw(" "),
                ghost_badge(self.relation_direction.label(), accent),
                Span::raw(" "),
                Span::styled(
                    self.current_selection_label(),
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
            ]))),
            sections[1],
        );

        let relation_labels = self
            .relation_items()
            .iter()
            .copied()
            .map(|relation| self.relation_list_item(relation))
            .collect::<Vec<_>>();
        let empty = relation_labels.is_empty();
        let mut state = ListState::default().with_selected((!empty).then_some(self.relation_index));
        let list = if empty {
            List::new(vec![ListItem::new(self.empty_list_label())])
        } else {
            List::new(relation_labels)
        }
        .highlight_style(selected_list_style(accent))
        .highlight_symbol(">> ");
        frame.render_stateful_widget(list, sections[2], &mut state);
    }

    fn render_paths_content(&self, frame: &mut Frame, area: Rect) {
        let accent = lane_accent(self.lane_mode);
        let items = self
            .path_items()
            .iter()
            .map(|item| self.path_list_item(item))
            .collect::<Vec<_>>();
        let empty = items.is_empty();
        let mut state = ListState::default().with_selected((!empty).then_some(self.path_index));
        let list = if empty {
            List::new(vec![ListItem::new(self.empty_list_label())])
        } else {
            List::new(items)
        }
        .highlight_style(selected_list_style(accent))
        .highlight_symbol(">> ");
        frame.render_stateful_widget(list, area, &mut state);
    }

    fn render_gap_center_content(&self, frame: &mut Frame, area: Rect) {
        let sections = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(8), Constraint::Length(10)])
            .split(area);
        let accent = workspace_section_accent(WorkspaceSection::Gaps);
        let gap_block = Block::default()
            .title(Line::from(vec![
                badge("GAP", Color::Black, accent),
                Span::raw(" "),
                Span::styled("Diagnostic Card", title_style(accent, true)),
            ]))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(accent));
        let gap_inner = gap_block.inner(sections[0]);
        frame.render_widget(gap_block, sections[0]);
        frame.render_widget(
            Paragraph::new(self.gap_card_text())
                .wrap(Wrap { trim: false })
                .style(Style::default().fg(Color::White)),
            gap_inner,
        );

        let path_items = self
            .path_items()
            .iter()
            .take(4)
            .map(|item| self.path_list_item(item))
            .collect::<Vec<_>>();
        let title = Line::from(vec![
            badge(
                self.lane_mode.label().to_ascii_uppercase(),
                Color::Black,
                lane_accent(self.lane_mode),
            ),
            Span::raw(" "),
            Span::styled("Best Paths", title_style(accent, true)),
        ]);
        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(accent));
        let inner = block.inner(sections[1]);
        frame.render_widget(block, sections[1]);
        frame.render_widget(
            if path_items.is_empty() {
                List::new(vec![ListItem::new(self.empty_list_label())])
            } else {
                List::new(path_items)
            }
            .highlight_style(selected_list_style(lane_accent(self.lane_mode))),
            inner,
        );
    }

    fn render_detail(&self, frame: &mut Frame, area: Rect) {
        let accent = detail_tab_accent(self.detail_tab);
        let block = titled_block(
            Line::from(vec![
                badge(
                    self.detail_tab.label().to_ascii_uppercase(),
                    Color::Black,
                    accent,
                ),
                Span::raw(" "),
                Span::styled(
                    "Detail Pane",
                    title_style(accent, self.focus == Focus::Detail),
                ),
            ]),
            accent,
            self.focus == Focus::Detail,
        );
        let inner = block.inner(area);
        frame.render_widget(block, area);
        let sections = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Min(0),
            ])
            .split(inner);
        let tabs = Tabs::new(
            DetailTab::ORDER
                .iter()
                .map(|tab| Line::from(tab.label()))
                .collect::<Vec<_>>(),
        )
        .select(
            DetailTab::ORDER
                .iter()
                .position(|tab| *tab == self.detail_tab)
                .unwrap_or(0),
        )
        .highlight_style(Style::default().fg(accent).add_modifier(Modifier::BOLD));
        frame.render_widget(tabs, sections[0]);
        frame.render_widget(
            Paragraph::new(self.detail_header_text()).wrap(Wrap { trim: false }),
            sections[1],
        );
        self.render_detail_body(frame, sections[2]);
    }

    fn render_status(&self, frame: &mut Frame, area: Rect) {
        let mut lines = vec![Line::from(vec![
            badge(
                self.focus_label().to_ascii_uppercase(),
                Color::Black,
                workspace_section_accent(self.workspace_section),
            ),
            Span::raw(" "),
            ghost_badge(
                self.workspace_section.label(),
                workspace_section_accent(self.workspace_section),
            ),
            Span::raw(" "),
            ghost_badge(self.lane_mode.label(), lane_accent(self.lane_mode)),
            Span::raw(" "),
            ghost_badge(self.detail_tab.label(), detail_tab_accent(self.detail_tab)),
            Span::raw(" "),
            ghost_badge(
                format!("scope {}", self.search_scope.label()),
                muted_color(),
            ),
        ])];

        let mut chips = Vec::new();
        if self.has_search_query() {
            chips.push(ghost_badge(
                format!(
                    "search /{}",
                    compact_inline_text(self.search_query.trim(), 30)
                ),
                detail_tab_accent(DetailTab::Source),
            ));
        }
        if self.filters.has_active() {
            chips.push(ghost_badge(
                format!("exact {}", self.filters.kind_summary()),
                lane_accent(LaneMode::Exact),
            ));
            if self.filters.basis_summary() != "all" {
                chips.push(ghost_badge(
                    format!("basis {}", self.filters.basis_summary()),
                    lane_accent(LaneMode::Exact),
                ));
            }
        }
        if self.heuristic_filters.has_active() {
            chips.push(ghost_badge(
                format!("heur {}", self.heuristic_filters.evidence_summary()),
                lane_accent(LaneMode::Heuristic),
            ));
        }
        if matches!(
            self.workspace_section,
            WorkspaceSection::Composition | WorkspaceSection::Machines | WorkspaceSection::Gaps
        ) {
            let (warnings, suggestions) = self.composition_diagnostic_counts();
            chips.push(ghost_badge(
                format!("diag {warnings}w/{suggestions}s"),
                workspace_section_accent(WorkspaceSection::Gaps),
            ));
        }
        if self.lane_mode.shows_heuristic()
            || self.heuristic.status() != HeuristicStatusKind::Available
        {
            chips.push(ghost_badge(
                format!("heur {}", self.heuristic.status().display_label()),
                lane_accent(LaneMode::Heuristic),
            ));
        }
        if chips.is_empty() {
            chips.push(ghost_badge("ready", muted_color()));
        }
        lines.push(join_spans(chips));

        let key_help = if self.input_mode == InputMode::Search {
            "enter apply  esc clear  backspace delete  s scope"
        } else {
            "tab focus  [ ] tabs  j/k move or scroll  / search  ? help  q quit"
        };
        lines.push(Line::from(Span::styled(
            key_help,
            Style::default().fg(mutated_color_fallback(lane_accent(self.lane_mode))),
        )));
        let status = Paragraph::new(Text::from(lines)).wrap(Wrap { trim: false });
        frame.render_widget(status, area);
    }

    fn render_help_overlay(&self, frame: &mut Frame) {
        let area = centered_rect(72, 60, frame.area());
        let block = Block::default()
            .title(Line::from(vec![
                badge("HELP", Color::Black, detail_tab_accent(DetailTab::Explain)),
                Span::raw(" "),
                Span::styled(
                    "Inspector Keys",
                    title_style(detail_tab_accent(DetailTab::Explain), true),
                ),
            ]))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(detail_tab_accent(DetailTab::Explain)));
        let inner = block.inner(area);
        frame.render_widget(block, area);
        frame.render_widget(
            Paragraph::new(Text::from(vec![
                Line::from("Views: `1` Workspace, `2` Machine, `3` Gaps"),
                Line::from("Lanes: `e` exact, `m` mixed, `h` heuristic"),
                Line::from("Focus: `tab` / `shift-tab`"),
                Line::from("Tabs: `[` previous, `]` next"),
                Line::from("Lists: arrows or `j` / `k`"),
                Line::from(
                    "Diagrams: `j` / `k` scroll when the center is on `Workspace` or `Diagram`",
                ),
                Line::from("Search: `/` enter, `s` scope, `esc` or `enter` finish"),
                Line::from("Filters: `p` payload, `f` field, `t` param"),
                Line::from("Filters: `d` direct, `n` declared ref, `g` signature, `b` body"),
                Line::from("Relations: `o` outbound, `i` inbound"),
                Line::from("Other: `0` clear filters, `q` quit, `?` close help"),
            ]))
            .wrap(Wrap { trim: false }),
            inner,
        );
    }

    fn center_diagnostics_text(&self) -> Text<'static> {
        match self.workspace_section {
            WorkspaceSection::Composition | WorkspaceSection::Machines => self
                .current_machine()
                .map(|machine| {
                    let suggestions = self.machine_suggestions(machine.index);
                    if suggestions.is_empty() {
                        Text::from(vec![
                            Line::from("No machine-local composition diagnostics."),
                            Line::from(format!(
                                "heuristics: {} ({})",
                                self.heuristic.status().display_label(),
                                self.heuristic.diagnostics().len()
                            )),
                        ])
                    } else {
                        let mut lines = vec![Line::from(format!(
                            "{} diagnostic{}",
                            suggestions.len(),
                            if suggestions.len() == 1 { "" } else { "s" }
                        ))];
                        for suggestion in suggestions {
                            lines.push(Line::from(""));
                            lines.push(Line::from(format!(
                                "{}: {}",
                                suggestion.severity.display_label(),
                                suggestion.summary_label(&self.doc)
                            )));
                            lines.push(Line::from(format!("why: {}", suggestion.why_text())));
                            lines.push(Line::from(format!("help: {}", suggestion.help_text())));
                        }
                        Text::from(lines)
                    }
                })
                .unwrap_or_else(|| Text::from("<no matches>")),
            WorkspaceSection::Gaps => self.gap_card_text(),
        }
    }

    fn detail_text_for_tab(&self) -> Text<'static> {
        match self.detail_tab {
            DetailTab::Summary => self.current_summary_text(),
            DetailTab::Docs => self.current_docs_text(),
            DetailTab::Diagram => self.current_diagram_text(),
            DetailTab::Source => self.current_source_text(),
            DetailTab::Explain => self.current_explain_text(),
        }
    }

    fn current_summary_text(&self) -> Text<'static> {
        if self.is_workspace_home() {
            return self.composition_workspace_detail_text();
        }
        match self.workspace_section {
            WorkspaceSection::Composition | WorkspaceSection::Machines => {
                let Some(machine) = self.current_machine() else {
                    return Text::from("<no matches>");
                };
                match self.machine_section {
                    MachineSection::Overview | MachineSection::Diagnostics => {
                        self.machine_workspace_detail_text(machine)
                    }
                    MachineSection::States => match self.selected_machine_item() {
                        Some(MachineItem::State(state_index)) => machine
                            .state(state_index)
                            .map(state_detail_text)
                            .unwrap_or_else(|| self.machine_workspace_detail_text(machine)),
                        _ => self.machine_workspace_detail_text(machine),
                    },
                    MachineSection::Transitions => match self.selected_machine_item() {
                        Some(MachineItem::Transition(transition_index)) => machine
                            .transition(transition_index)
                            .map(transition_detail_text)
                            .unwrap_or_else(|| self.machine_workspace_detail_text(machine)),
                        _ => self.machine_workspace_detail_text(machine),
                    },
                    MachineSection::Validators => match self.selected_machine_item() {
                        Some(MachineItem::Validator(entry_index)) => machine
                            .validator_entry(entry_index)
                            .map(validator_detail_text)
                            .unwrap_or_else(|| self.machine_workspace_detail_text(machine)),
                        _ => self.machine_workspace_detail_text(machine),
                    },
                    MachineSection::Relations => self
                        .selected_relation_detail()
                        .map(relation_detail_selection_text)
                        .unwrap_or_else(|| self.empty_relation_text()),
                    MachineSection::Paths => self
                        .selected_path_item()
                        .map(|path| self.path_detail_text(&path))
                        .unwrap_or_else(|| self.empty_path_text()),
                }
            }
            WorkspaceSection::Gaps => self.gap_card_text(),
        }
    }

    fn current_docs_text(&self) -> Text<'static> {
        if self.is_workspace_home() {
            return workspace_docs_text();
        }
        match self.workspace_section {
            WorkspaceSection::Composition | WorkspaceSection::Machines => {
                let Some(machine) = self.current_machine() else {
                    return Text::from("<no matches>");
                };
                match self.machine_section {
                    MachineSection::Overview | MachineSection::Diagnostics => {
                        docs_text(machine.description, machine.docs)
                    }
                    MachineSection::States => match self.selected_machine_item() {
                        Some(MachineItem::State(state_index)) => machine
                            .state(state_index)
                            .map(|state| docs_text(state.description, state.docs))
                            .unwrap_or_else(|| docs_text(machine.description, machine.docs)),
                        _ => docs_text(machine.description, machine.docs),
                    },
                    MachineSection::Transitions => match self.selected_machine_item() {
                        Some(MachineItem::Transition(transition_index)) => machine
                            .transition(transition_index)
                            .map(|transition| docs_text(transition.description, transition.docs))
                            .unwrap_or_else(|| docs_text(machine.description, machine.docs)),
                        _ => docs_text(machine.description, machine.docs),
                    },
                    MachineSection::Validators => match self.selected_machine_item() {
                        Some(MachineItem::Validator(entry_index)) => machine
                            .validator_entry(entry_index)
                            .map(|entry| docs_text(None, entry.docs))
                            .unwrap_or_else(|| docs_text(machine.description, machine.docs)),
                        _ => docs_text(machine.description, machine.docs),
                    },
                    MachineSection::Relations => self
                        .selected_relation_detail()
                        .map(relation_docs_text)
                        .unwrap_or_else(|| Text::from("No docs for the current relation.")),
                    MachineSection::Paths => self
                        .selected_path_item()
                        .map(|path| path_docs_text(&path, &self.doc))
                        .unwrap_or_else(|| Text::from("No docs for the current path.")),
                }
            }
            WorkspaceSection::Gaps => self
                .current_gap()
                .map(|gap| gap_docs_text(gap, &self.doc))
                .unwrap_or_else(|| Text::from("<no matches>")),
        }
    }

    fn current_source_text(&self) -> Text<'static> {
        if self.is_workspace_home() {
            return workspace_source_text(&self.workspace_label, &self.doc);
        }
        match self.workspace_section {
            WorkspaceSection::Composition | WorkspaceSection::Machines => {
                let Some(machine) = self.current_machine() else {
                    return Text::from("<no matches>");
                };
                match self.machine_section {
                    MachineSection::Overview | MachineSection::Diagnostics => {
                        machine_source_text(machine)
                    }
                    MachineSection::States => match self.selected_machine_item() {
                        Some(MachineItem::State(state_index)) => machine
                            .state(state_index)
                            .map(|state| state_source_text(machine, state))
                            .unwrap_or_else(|| machine_source_text(machine)),
                        _ => machine_source_text(machine),
                    },
                    MachineSection::Transitions => match self.selected_machine_item() {
                        Some(MachineItem::Transition(transition_index)) => machine
                            .transition(transition_index)
                            .map(|transition| transition_source_text(machine, transition))
                            .unwrap_or_else(|| machine_source_text(machine)),
                        _ => machine_source_text(machine),
                    },
                    MachineSection::Validators => match self.selected_machine_item() {
                        Some(MachineItem::Validator(entry_index)) => machine
                            .validator_entry(entry_index)
                            .map(validator_source_text)
                            .unwrap_or_else(|| machine_source_text(machine)),
                        _ => machine_source_text(machine),
                    },
                    MachineSection::Relations => self
                        .selected_relation_detail()
                        .map(relation_source_text)
                        .unwrap_or_else(|| self.empty_relation_text()),
                    MachineSection::Paths => self
                        .selected_path_item()
                        .map(|path| path_source_text(&path, &self.doc))
                        .unwrap_or_else(|| self.empty_path_text()),
                }
            }
            WorkspaceSection::Gaps => self
                .current_gap()
                .map(|gap| gap_source_text(gap, &self.doc))
                .unwrap_or_else(|| Text::from("<no matches>")),
        }
    }

    fn workspace_diagram_plan(&self) -> DiagramPlan {
        DiagramPlan {
            key: "workspace-flow".to_owned(),
            title: format!("Workspace {}", self.workspace_label),
            kind_label: "flowchart",
            exact: true,
            source: workspace_diagram_text(&self.doc),
        }
    }

    fn machine_diagram_plan(&self, machine: &CodebaseMachine) -> DiagramPlan {
        DiagramPlan {
            key: format!("machine:{}", machine.rust_type_path),
            title: format!("Machine {}", render_machine_label(machine)),
            kind_label: "stateDiagram-v2",
            exact: true,
            source: machine_diagram_text(&self.doc, machine),
        }
    }

    fn unavailable_diagram_plan(
        &self,
        key: impl Into<String>,
        title: impl Into<String>,
        message: Text<'static>,
    ) -> DiagramPlan {
        DiagramPlan {
            key: key.into(),
            title: title.into(),
            kind_label: "unavailable",
            exact: false,
            source: message,
        }
    }

    fn center_diagram_plan(&self) -> DiagramPlan {
        match self.workspace_section {
            WorkspaceSection::Composition => self.workspace_diagram_plan(),
            WorkspaceSection::Machines => self
                .current_machine()
                .map(|machine| self.machine_diagram_plan(machine))
                .unwrap_or_else(|| {
                    self.unavailable_diagram_plan(
                        "machine:none",
                        "Machine <no matches>",
                        Text::from("<no matches>"),
                    )
                }),
            WorkspaceSection::Gaps => self.unavailable_diagram_plan(
                "gaps:none",
                "Gap Diagnostics",
                Text::from(
                    "Gap diagnostics do not have an exact Mermaid export yet.\nSelect a machine or exact relation to inspect a diagram.",
                ),
            ),
        }
    }

    fn detail_diagram_plan(&self) -> DiagramPlan {
        match self.workspace_section {
            WorkspaceSection::Composition if self.machine_section == MachineSection::Overview => {
                self.workspace_diagram_plan()
            }
            WorkspaceSection::Composition | WorkspaceSection::Machines => {
                let Some(machine) = self.current_machine() else {
                    return self.unavailable_diagram_plan(
                        "machine:none",
                        "Machine <no matches>",
                        Text::from("<no matches>"),
                    );
                };
                match self.machine_section {
                    MachineSection::Overview
                    | MachineSection::States
                    | MachineSection::Transitions
                    | MachineSection::Validators
                    | MachineSection::Diagnostics => self.machine_diagram_plan(machine),
                    MachineSection::Relations => match self.selected_relation_detail() {
                        Some(RelationDetailSelection::Exact(detail)) => DiagramPlan {
                            key: format!("relation:{}", detail.relation.index),
                            title: render_relation_label(&detail),
                            kind_label: "sequenceDiagram",
                            exact: true,
                            source: relation_diagram_text(
                                &self.doc,
                                RelationDetailSelection::Exact(detail),
                            ),
                        },
                        Some(RelationDetailSelection::Heuristic { detail, .. }) => {
                            self.unavailable_diagram_plan(
                                format!("heuristic-relation:{}", detail.relation.index),
                                render_heuristic_relation_label(&detail),
                                relation_diagram_text(
                                    &self.doc,
                                    RelationDetailSelection::Heuristic {
                                        detail,
                                        shadowed_by_exact: false,
                                    },
                                ),
                            )
                        }
                        None => self.unavailable_diagram_plan(
                            "relation:none",
                            "Handoff <no relation>",
                            Text::from(
                                "No exact relation selected.\nSelect an exact relation to view its Mermaid sequence diagram.",
                            ),
                        ),
                    },
                    MachineSection::Paths => self
                        .selected_path_item()
                        .map(|path| {
                            self.unavailable_diagram_plan(
                                format!("path:{}:{}", path.kind.display_label(), path.target_machine),
                                self.path_item_label(&path),
                                path_diagram_text(path),
                            )
                        })
                        .unwrap_or_else(|| {
                            self.unavailable_diagram_plan(
                                "path:none",
                                "Path <no selection>",
                                Text::from(
                                    "No path selected.\nExact composition-path Mermaid export is not implemented yet.",
                                ),
                            )
                        }),
                }
            }
            WorkspaceSection::Gaps => self.unavailable_diagram_plan(
                "gaps:none",
                "Gap Diagnostics",
                Text::from(
                    "Gap diagnostics do not have an exact Mermaid export yet.\nSelect a machine or exact relation to inspect a diagram.",
                ),
            ),
        }
    }

    fn current_diagram_text(&self) -> Text<'static> {
        self.detail_diagram_plan().source
    }

    fn diagram_plan_preview_text(&self, plan: &DiagramPlan, width: u16) -> Text<'static> {
        let raw = plan.source.clone();
        let source = text_plain_string(&raw);
        let width = width.max(24);
        let key = format!("{width}\n{}\n{source}", plan.key);
        if let Some(cached) = self.cache.borrow().diagram_preview.as_ref() {
            if cached.key == key {
                return cached.text.clone();
            }
        }

        let text = if mermaid_diagram_source(&source).is_some() {
            match render_termaid_preview(&source, width, &self.workspace_label) {
                Ok(rendered) => Text::from(rendered),
                Err(reason) => Text::from(format!(
                    "termaid preview unavailable; showing Mermaid source.\n{reason}\n\n{source}"
                )),
            }
        } else {
            raw
        };
        self.cache.borrow_mut().diagram_preview = Some(DiagramPreviewCache {
            key,
            text: text.clone(),
        });
        text
    }

    fn current_diagram_preview_text(&self, width: u16) -> Text<'static> {
        let plan = self.detail_diagram_plan();
        self.diagram_plan_preview_text(&plan, width)
    }

    fn render_diagram_viewport(&self, frame: &mut Frame, area: Rect, plan: &DiagramPlan) {
        let accent = if plan.exact {
            lane_accent(LaneMode::Exact)
        } else {
            muted_color()
        };
        let sections = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(2), Constraint::Min(0)])
            .split(area);
        frame.render_widget(
            Paragraph::new(Text::from(Line::from(vec![
                badge(
                    if plan.exact { "EXACT" } else { "INFO" },
                    Color::Black,
                    accent,
                ),
                Span::raw(" "),
                ghost_badge(plan.kind_label, accent),
                Span::raw(" "),
                Span::styled(
                    plan.title.clone(),
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
            ]))),
            sections[0],
        );
        frame.render_widget(
            Paragraph::new(self.diagram_plan_preview_text(plan, sections[1].width))
                .scroll((self.diagram_scroll_y, 0))
                .style(Style::default().fg(Color::White)),
            sections[1],
        );
    }

    fn current_explain_text(&self) -> Text<'static> {
        if self.is_workspace_home() {
            return workspace_explain_text(&self.doc);
        }
        match self.workspace_section {
            WorkspaceSection::Composition | WorkspaceSection::Machines => {
                let Some(machine) = self.current_machine() else {
                    return Text::from("<no matches>");
                };
                match self.machine_section {
                    MachineSection::Overview | MachineSection::Diagnostics => {
                        machine_explain_text(machine, self)
                    }
                    MachineSection::States => match self.selected_machine_item() {
                        Some(MachineItem::State(state_index)) => machine
                            .state(state_index)
                            .map(|state| state_explain_text(machine, state))
                            .unwrap_or_else(|| machine_explain_text(machine, self)),
                        _ => machine_explain_text(machine, self),
                    },
                    MachineSection::Transitions => match self.selected_machine_item() {
                        Some(MachineItem::Transition(transition_index)) => machine
                            .transition(transition_index)
                            .map(|transition| transition_explain_text(machine, transition))
                            .unwrap_or_else(|| machine_explain_text(machine, self)),
                        _ => machine_explain_text(machine, self),
                    },
                    MachineSection::Validators => match self.selected_machine_item() {
                        Some(MachineItem::Validator(entry_index)) => machine
                            .validator_entry(entry_index)
                            .map(validator_explain_text)
                            .unwrap_or_else(|| machine_explain_text(machine, self)),
                        _ => machine_explain_text(machine, self),
                    },
                    MachineSection::Relations => self
                        .selected_relation_detail()
                        .map(relation_explain_text)
                        .unwrap_or_else(|| self.empty_relation_text()),
                    MachineSection::Paths => self
                        .selected_path_item()
                        .map(|path| path_explain_text(&path, &self.doc))
                        .unwrap_or_else(|| self.empty_path_text()),
                }
            }
            WorkspaceSection::Gaps => self
                .current_gap()
                .map(|gap| gap_explain_text(gap, &self.doc))
                .unwrap_or_else(|| Text::from("<no matches>")),
        }
    }

    #[cfg(test)]
    fn detail_text(&self) -> Text<'static> {
        match self.focus {
            Focus::Workspace => match self.workspace_section {
                WorkspaceSection::Composition => self.composition_workspace_detail_text(),
                WorkspaceSection::Machines => self
                    .current_machine()
                    .map(|machine| self.machine_workspace_detail_text(machine))
                    .unwrap_or_else(|| Text::from("<no matches>")),
                WorkspaceSection::Gaps => self.gaps_workspace_detail_text(),
            },
            Focus::MainView => match self.workspace_section {
                WorkspaceSection::Composition => self.machine_detail_selection_text(),
                WorkspaceSection::Machines => self.machine_detail_selection_text(),
                WorkspaceSection::Gaps => self.gap_card_text(),
            },
            Focus::Detail => match self.workspace_section {
                WorkspaceSection::Composition | WorkspaceSection::Gaps => self
                    .selected_path_item()
                    .map(|path| self.path_detail_text(&path))
                    .unwrap_or_else(|| self.empty_path_text()),
                WorkspaceSection::Machines => self
                    .selected_relation_detail()
                    .map(relation_detail_selection_text)
                    .unwrap_or_else(|| self.empty_relation_text()),
            },
        }
    }

    #[cfg(test)]
    fn machine_detail_selection_text(&self) -> Text<'static> {
        let Some(machine) = self.current_machine() else {
            return Text::from("<no matches>");
        };
        match self.machine_section {
            MachineSection::States => match self.selected_machine_item() {
                Some(MachineItem::State(state_index)) => machine
                    .state(state_index)
                    .map(state_detail_text)
                    .unwrap_or_else(|| self.machine_workspace_detail_text(machine)),
                _ => self.machine_workspace_detail_text(machine),
            },
            MachineSection::Transitions => match self.selected_machine_item() {
                Some(MachineItem::Transition(transition_index)) => machine
                    .transition(transition_index)
                    .map(transition_detail_text)
                    .unwrap_or_else(|| self.machine_workspace_detail_text(machine)),
                _ => self.machine_workspace_detail_text(machine),
            },
            MachineSection::Validators => match self.selected_machine_item() {
                Some(MachineItem::Validator(entry_index)) => machine
                    .validator_entry(entry_index)
                    .map(validator_detail_text)
                    .unwrap_or_else(|| self.machine_workspace_detail_text(machine)),
                _ => self.machine_workspace_detail_text(machine),
            },
            MachineSection::Overview
            | MachineSection::Relations
            | MachineSection::Paths
            | MachineSection::Diagnostics => self.machine_workspace_detail_text(machine),
        }
    }

    fn machine_workspace_detail_text(&self, machine: &CodebaseMachine) -> Text<'static> {
        machine_detail_text(machine, &self.doc, &self.machine_suggestions(machine.index))
    }

    fn composition_workspace_detail_text(&self) -> Text<'static> {
        let visible = self.visible_composition_machine_indices();
        let exact_edges = self
            .visible_machine_relation_groups()
            .iter()
            .filter(|group| group.from_machine != group.to_machine)
            .count();
        let heuristic_edges = self
            .visible_heuristic_machine_relation_groups()
            .iter()
            .filter(|group| group.from_machine != group.to_machine)
            .count();
        Text::from(vec![
            Line::from(format!(
                "composition flows: {}/{}",
                visible.len(),
                self.doc
                    .machines()
                    .iter()
                    .filter(|machine| machine.role.is_composition())
                    .count()
            )),
            Line::from("Workspace view is the top-level exact flow surface."),
            Line::from(format!("visible exact machine edges: {exact_edges}")),
            Line::from(format!("visible heuristic machine edges: {heuristic_edges}")),
            Line::from(
                "Use the workspace diagram as the main flow surface, then drill into machines for exact state diagrams.",
            ),
            Line::from(
                "Use the path pane to follow composition-first routes, then exact graph routes, then heuristic fallback.",
            ),
        ])
    }

    #[cfg(test)]
    fn gaps_workspace_detail_text(&self) -> Text<'static> {
        Text::from(vec![
            Line::from(format!(
                "composition diagnostics: {} warning, {} suggestion",
                self.suggestions.warning_count(),
                self.suggestions.suggestion_count()
            )),
            Line::from("Warnings are exact typed orchestration smells on protocol machines."),
            Line::from(
                "Suggestions are weaker heuristic candidates that still need exact modeling.",
            ),
            Line::from(format!(
                "heuristics: {} ({})",
                self.heuristic.status().display_label(),
                self.heuristic.diagnostics().len()
            )),
        ])
    }

    fn gap_card_text(&self) -> Text<'static> {
        let Some(gap) = self.current_gap() else {
            return Text::from("<no matches>");
        };
        gap_detail_text(gap, &self.doc)
    }

    fn has_search_query(&self) -> bool {
        !self.search_query.trim().is_empty()
    }

    fn normalized_query(&self) -> Option<String> {
        let trimmed = self.search_query.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_ascii_lowercase())
    }

    fn empty_list_label(&self) -> &'static str {
        if self.has_search_query()
            || self.filters.has_active()
            || self.heuristic_filters.has_active()
        {
            "<no matches>"
        } else {
            "<none>"
        }
    }

    fn empty_relation_text(&self) -> Text<'static> {
        if self.relation_items().is_empty()
            && self.lane_mode.shows_heuristic()
            && self.heuristic.status() == HeuristicStatusKind::Unavailable
        {
            return heuristic_status_text(self.heuristic.status(), self.heuristic.diagnostics());
        }

        let Some(subject) = self.relation_subject() else {
            return Text::from(self.empty_list_label());
        };
        let machine_index = match subject {
            RelationSubject::Machine { machine }
            | RelationSubject::State { machine, .. }
            | RelationSubject::Transition { machine, .. } => machine,
        };
        let Some(machine) = self.doc.machine(machine_index) else {
            return Text::from(self.empty_list_label());
        };
        let (exact_summary_count, heuristic_summary_count) =
            self.machine_visible_summary_counts(machine_index);
        let has_any_heuristic_summary = self.machine_has_any_heuristic_summary(machine_index);
        let lane_label = match self.lane_mode {
            LaneMode::Exact => "exact",
            LaneMode::Heuristic => "heuristic",
            LaneMode::Mixed => "visible",
        };

        let mut lines = match subject {
            RelationSubject::State { state, .. } => {
                let state = machine.state(state).expect("state subject should exist");
                vec![
                    Line::from(format!(
                        "No {lane_label} relations for state {}.",
                        render_state_label(state)
                    )),
                    Line::from(
                        "State selections only show relations attached directly to that state.",
                    ),
                ]
            }
            RelationSubject::Transition { transition, .. } => {
                let transition = machine
                    .transition(transition)
                    .expect("transition subject should exist");
                vec![
                    Line::from(format!(
                        "No {lane_label} relations for transition {}.",
                        render_transition_label(transition)
                    )),
                    Line::from("Transition selections only show relations attached directly to that transition."),
                ]
            }
            RelationSubject::Machine { .. } => {
                return Text::from(self.empty_list_label());
            }
        };

        if exact_summary_count > 0 || heuristic_summary_count > 0 {
            lines.push(Line::from(format!(
                "Try Overview to inspect machine-level edges ({} exact, {} heuristic visible).",
                exact_summary_count, heuristic_summary_count
            )));
        }
        if matches!(subject, RelationSubject::State { .. }) && !machine.transitions.is_empty() {
            lines.push(Line::from(
                "Try Transitions to inspect transition-parameter and attested-route edges.",
            ));
        }
        if !self.lane_mode.shows_heuristic() && has_any_heuristic_summary {
            lines.push(Line::from(
                "Switch to heuristic (`h`) or mixed (`m`) mode to inspect weaker source-scanned couplings.",
            ));
        }

        Text::from(lines)
    }

    fn empty_path_text(&self) -> Text<'static> {
        match self.workspace_section {
            WorkspaceSection::Composition => self
                .current_machine()
                .map(|machine| {
                    let mut lines = vec![
                        Line::from(format!(
                            "No visible paths from {}.",
                            render_machine_label(machine)
                        )),
                        Line::from(
                            "Path explorer prefers composition-owned edges, then exact graph edges, then heuristic fallback.",
                        ),
                    ];
                    if !self.lane_mode.shows_heuristic()
                        && self.machine_has_any_heuristic_summary(machine.index)
                    {
                        lines.push(Line::from(
                            "Switch to heuristic (`h`) or mixed (`m`) mode to include weaker fallback paths.",
                        ));
                    }
                    Text::from(lines)
                })
                .unwrap_or_else(|| Text::from(self.empty_list_label())),
            WorkspaceSection::Gaps => self
                .current_gap()
                .map(|gap| {
                    Text::from(vec![
                        Line::from(format!(
                            "No visible path from {} to {}.",
                            gap.source_machine(&self.doc)
                                .map(render_machine_label)
                                .unwrap_or("<missing machine>"),
                            gap.target_machine(&self.doc)
                                .map(render_machine_label)
                                .unwrap_or("<missing machine>")
                        )),
                        Line::from(
                            "This gap still needs composition modeling or richer exact handoff surfaces.",
                        ),
                    ])
                })
                .unwrap_or_else(|| Text::from(self.empty_list_label())),
            WorkspaceSection::Machines => Text::from(self.empty_list_label()),
        }
    }

    fn path_detail_text(&self, path: &PathItem) -> Text<'static> {
        path_detail_text(path, &self.doc)
    }

    fn machine_search_reason(&self, machine: &CodebaseMachine) -> Option<String> {
        let query = self.normalized_query();
        let query = query.as_deref();

        if self.search_scope.includes_primary() {
            if let Some(reason) = first_match_reason(
                query,
                [
                    ("name", render_machine_label(machine).to_owned()),
                    ("path", machine.rust_type_path.to_owned()),
                    (
                        "summary",
                        machine.description.unwrap_or_default().to_owned(),
                    ),
                ],
            ) {
                return Some(reason);
            }
        }
        if self.search_scope.includes_docs() {
            if let Some(reason) = first_match_reason(
                query,
                [("docs", machine.docs.unwrap_or_default().to_owned())],
            ) {
                return Some(reason);
            }
        }
        if self.search_scope.includes_primary() || self.search_scope.includes_docs() {
            for state in &machine.states {
                if let Some(reason) = self.state_match_reason(state, query) {
                    return Some(format!("state {reason}"));
                }
            }
            for transition in &machine.transitions {
                if let Some(reason) = self.transition_match_reason(transition, query) {
                    return Some(format!("transition {reason}"));
                }
            }
            for entry in &machine.validator_entries {
                if let Some(reason) = self.validator_match_reason(entry, query) {
                    return Some(format!("validator {reason}"));
                }
            }
            for suggestion in self.machine_suggestions(machine.index) {
                if self.suggestion_matches_query(suggestion, query) {
                    if let Some(reason) = self.suggestion_search_reason(suggestion) {
                        return Some(format!("diagnostic {reason}"));
                    }
                }
            }
        }
        if self.search_scope.includes_relations() && self.lane_mode.shows_exact() {
            for relation in self
                .doc
                .outbound_relations_for_machine(machine.index)
                .chain(self.doc.inbound_relations_for_machine(machine.index))
                .filter(|relation| self.filters.matches_relation(relation))
            {
                if let Some(detail) = self.doc.relation_detail(relation.index) {
                    if let Some(reason) = self.exact_relation_match_reason(&detail) {
                        return Some(reason);
                    }
                }
            }
        }
        if self.search_scope.includes_relations() && self.lane_mode.shows_heuristic() {
            for relation in self
                .heuristic
                .outbound_relations_for_machine(machine.index)
                .chain(self.heuristic.inbound_relations_for_machine(machine.index))
                .filter(|relation| self.heuristic_filters.matches_relation(relation))
            {
                if let Some(detail) = self.heuristic.relation_detail(&self.doc, relation.index) {
                    if let Some(reason) = self.heuristic_relation_match_reason(&detail) {
                        return Some(reason);
                    }
                }
            }
        }
        if self.search_scope.includes_paths() {
            if let Some(path) = self
                .path_items_from_source(machine.index, None, query)
                .first()
            {
                return self.path_search_reason(path);
            }
        }

        None
    }

    fn suggestion_search_reason(&self, suggestion: &CompositionSuggestion) -> Option<String> {
        let query = self.normalized_query();
        let query = query.as_deref();
        let source = suggestion
            .source_machine(&self.doc)
            .map(render_machine_label)
            .unwrap_or("<missing machine>");
        let target = suggestion
            .target_machine(&self.doc)
            .map(render_machine_label)
            .unwrap_or("<missing machine>");

        if self.search_scope.includes_primary() {
            if let Some(reason) = first_match_reason(
                query,
                [
                    ("diagnostic", suggestion.summary_label(&self.doc)),
                    ("severity", suggestion.severity.display_label().to_owned()),
                    ("kind", suggestion.kind.display_label().to_owned()),
                ],
            ) {
                return Some(reason);
            }
        }
        if self.search_scope.includes_docs() {
            if let Some(reason) = first_match_reason(
                query,
                [
                    ("why", suggestion.why_text().to_owned()),
                    ("help", suggestion.help_text().to_owned()),
                ],
            ) {
                return Some(reason);
            }
        }
        if self.search_scope.includes_relations() {
            if let Some(reason) = first_match_reason(
                query,
                [
                    ("evidence", suggestion.counts_label()),
                    ("source", source.to_owned()),
                    ("target", target.to_owned()),
                ],
            ) {
                return Some(reason);
            }
        }
        if self.search_scope.includes_paths() {
            if let Some(reason) =
                first_match_reason(query, [("route", format!("{source} -> {target}"))])
            {
                return Some(reason);
            }
        }

        None
    }

    fn machine_item_search_reason(
        &self,
        machine: &CodebaseMachine,
        item: &MachineItem,
    ) -> Option<String> {
        let query = self.normalized_query();
        let query = query.as_deref();
        match item {
            MachineItem::State(index) => machine
                .state(*index)
                .and_then(|state| self.state_match_reason(state, query)),
            MachineItem::Transition(index) => machine
                .transition(*index)
                .and_then(|transition| self.transition_match_reason(transition, query)),
            MachineItem::Validator(index) => machine
                .validator_entry(*index)
                .and_then(|entry| self.validator_match_reason(entry, query)),
        }
    }

    fn relation_search_reason(&self, relation: RelationItem) -> Option<String> {
        match relation {
            RelationItem::Exact(index) => self
                .doc
                .relation_detail(index)
                .and_then(|detail| self.exact_relation_match_reason(&detail)),
            RelationItem::Heuristic(index) => self
                .heuristic
                .relation_detail(&self.doc, index)
                .and_then(|detail| self.heuristic_relation_match_reason(&detail)),
        }
    }

    fn path_search_reason(&self, item: &PathItem) -> Option<String> {
        let query = self.normalized_query();
        let query = query.as_deref();
        if !self.search_scope.includes_paths() {
            return None;
        }

        let source = item
            .steps
            .first()
            .and_then(|step| self.doc.machine(step.from_machine))
            .map(render_machine_label)
            .unwrap_or("<missing machine>");
        let target = self
            .doc
            .machine(item.target_machine)
            .map(render_machine_label)
            .unwrap_or("<missing machine>");
        let mut candidates = vec![
            ("route", path_step_preview(item, &self.doc)),
            ("kind", item.kind.display_label().to_owned()),
            ("from", source.to_owned()),
            ("to", target.to_owned()),
            ("hops", format!("{} hop", item.steps.len())),
        ];
        for step in &item.steps {
            candidates.push(("step", step.label.clone()));
        }
        first_match_reason(query, candidates)
    }

    fn state_match_reason(&self, state: &CodebaseState, query: Option<&str>) -> Option<String> {
        if self.search_scope.includes_primary() {
            if let Some(reason) = first_match_reason(
                query,
                [
                    ("name", render_state_label(state)),
                    ("rust", state.rust_name.to_owned()),
                    ("summary", state.description.unwrap_or_default().to_owned()),
                ],
            ) {
                return Some(reason);
            }
        }
        if self.search_scope.includes_docs() {
            if let Some(reason) =
                first_match_reason(query, [("docs", state.docs.unwrap_or_default().to_owned())])
            {
                return Some(reason);
            }
        }
        None
    }

    fn transition_match_reason(
        &self,
        transition: &CodebaseTransition,
        query: Option<&str>,
    ) -> Option<String> {
        if self.search_scope.includes_primary() {
            if let Some(reason) = first_match_reason(
                query,
                [
                    ("name", render_transition_label(transition).to_owned()),
                    ("method", transition.method_name.to_owned()),
                    (
                        "summary",
                        transition.description.unwrap_or_default().to_owned(),
                    ),
                ],
            ) {
                return Some(reason);
            }
        }
        if self.search_scope.includes_docs() {
            if let Some(reason) = first_match_reason(
                query,
                [("docs", transition.docs.unwrap_or_default().to_owned())],
            ) {
                return Some(reason);
            }
        }
        None
    }

    fn validator_match_reason(
        &self,
        entry: &CodebaseValidatorEntry,
        query: Option<&str>,
    ) -> Option<String> {
        if self.search_scope.includes_primary() {
            if let Some(reason) = first_match_reason(
                query,
                [
                    ("validator", entry.display_label().into_owned()),
                    ("module", entry.source_module_path.to_owned()),
                    ("type", entry.source_type_display.to_owned()),
                ],
            ) {
                return Some(reason);
            }
        }
        if self.search_scope.includes_docs() {
            if let Some(reason) =
                first_match_reason(query, [("docs", entry.docs.unwrap_or_default().to_owned())])
            {
                return Some(reason);
            }
        }
        None
    }

    fn exact_relation_match_reason(&self, detail: &CodebaseRelationDetail<'_>) -> Option<String> {
        let query = self.normalized_query();
        let query = query.as_deref();
        if !self.search_scope.includes_relations() {
            return None;
        }

        let mut candidates = vec![
            ("kind", detail.relation.kind.display_label().to_owned()),
            ("basis", detail.relation.basis.display_label().to_owned()),
            (
                "semantic",
                detail.relation.semantic.display_label().to_owned(),
            ),
            ("origin", relation_origin_label(detail)),
            (
                "source",
                exact_relation_source_label(detail.relation.source),
            ),
            (
                "target",
                render_machine_label(detail.target_machine).to_owned(),
            ),
            ("state", render_state_label(detail.target_state)),
        ];
        if let Some(reference_type) = detail.relation.declared_reference_type {
            candidates.push(("ref", reference_type.to_owned()));
        }
        if let Some(attested_via) = detail.relation.attested_via.as_ref() {
            candidates.push((
                "route",
                format!(
                    "{}::{}",
                    attested_via.via_module_path, attested_via.route_name
                ),
            ));
        }
        first_match_reason(query, candidates)
    }

    fn heuristic_relation_match_reason(
        &self,
        detail: &HeuristicRelationDetail<'_>,
    ) -> Option<String> {
        let query = self.normalized_query();
        let query = query.as_deref();
        if !self.search_scope.includes_relations() {
            return None;
        }

        first_match_reason(
            query,
            [
                (
                    "evidence",
                    detail.relation.evidence_kind.display_label().to_owned(),
                ),
                ("path", detail.relation.matched_path_text.clone()),
                ("source", render_heuristic_source_label(detail)),
                (
                    "target",
                    render_machine_label(detail.target_machine).to_owned(),
                ),
                (
                    "file",
                    format!(
                        "{}:{}",
                        compact_file_label(&detail.relation.file_path),
                        detail.relation.line_number
                    ),
                ),
                (
                    "snippet",
                    detail.relation.snippet.clone().unwrap_or_default(),
                ),
            ],
        )
    }

    fn query_matches_any<I, S>(query: Option<&str>, candidates: I) -> bool
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let Some(query) = query else {
            return true;
        };
        candidates
            .into_iter()
            .any(|candidate| candidate.as_ref().to_ascii_lowercase().contains(query))
    }

    fn machine_matches_query(&self, machine: &CodebaseMachine, query: Option<&str>) -> bool {
        let primary_match = self.search_scope.includes_primary()
            && Self::query_matches_any(
                query,
                [
                    machine.module_path.to_owned(),
                    machine.rust_type_path.to_owned(),
                    render_machine_label(machine).to_owned(),
                    machine.description.unwrap_or_default().to_owned(),
                ],
            );
        let docs_match = self.search_scope.includes_docs()
            && Self::query_matches_any(query, [machine.docs.unwrap_or_default().to_owned()]);
        if primary_match || docs_match {
            return true;
        }

        ((self.search_scope.includes_primary() || self.search_scope.includes_docs())
            && (machine
                .states
                .iter()
                .any(|state| self.state_matches_query(state, query))
                || machine
                    .transitions
                    .iter()
                    .any(|transition| self.transition_matches_query(transition, query))
                || machine
                    .validator_entries
                    .iter()
                    .any(|entry| self.validator_matches_query(entry, query))))
            || (self.search_scope.includes_relations()
                && self.lane_mode.shows_exact()
                && self.machine_exact_relations_match_query(machine.index, query))
            || (self.search_scope.includes_relations()
                && self.lane_mode.shows_heuristic()
                && self.machine_heuristic_relations_match_query(machine.index, query))
            || (self.search_scope.includes_paths()
                && !self
                    .path_items_from_source(machine.index, None, query)
                    .is_empty())
            || ((self.search_scope.includes_primary() || self.search_scope.includes_docs())
                && self.machine_suggestions_match_query(machine.index, query))
    }

    fn suggestion_matches_query(
        &self,
        suggestion: &CompositionSuggestion,
        query: Option<&str>,
    ) -> bool {
        let source = suggestion
            .source_machine(&self.doc)
            .map(render_machine_label)
            .unwrap_or("<missing machine>");
        let target = suggestion
            .target_machine(&self.doc)
            .map(render_machine_label)
            .unwrap_or("<missing machine>");
        (self.search_scope.includes_primary()
            && Self::query_matches_any(
                query,
                [
                    suggestion.severity.display_label().to_owned(),
                    suggestion.kind.display_label().to_owned(),
                    suggestion.summary_label(&self.doc),
                    source.to_owned(),
                    target.to_owned(),
                ],
            ))
            || (self.search_scope.includes_docs()
                && Self::query_matches_any(
                    query,
                    [
                        suggestion.help_text().to_owned(),
                        suggestion.why_text().to_owned(),
                    ],
                ))
            || (self.search_scope.includes_relations()
                && Self::query_matches_any(
                    query,
                    [
                        suggestion.counts_label(),
                        source.to_owned(),
                        target.to_owned(),
                    ],
                ))
            || (self.search_scope.includes_paths()
                && Self::query_matches_any(query, [source.to_owned(), target.to_owned()]))
    }

    fn machine_suggestions_match_query(&self, machine_index: usize, query: Option<&str>) -> bool {
        self.machine_suggestions(machine_index)
            .into_iter()
            .any(|suggestion| {
                let source = suggestion
                    .source_machine(&self.doc)
                    .map(|machine| render_machine_label(machine).to_owned())
                    .unwrap_or_default();
                let target = suggestion
                    .target_machine(&self.doc)
                    .map(|machine| render_machine_label(machine).to_owned())
                    .unwrap_or_default();
                Self::query_matches_any(
                    query,
                    [
                        suggestion.severity.display_label().to_owned(),
                        suggestion.kind.display_label().to_owned(),
                        suggestion.counts_label(),
                        suggestion.help_text().to_owned(),
                        suggestion.why_text().to_owned(),
                        source,
                        target,
                    ],
                )
            })
    }

    fn machine_exact_relations_match_query(
        &self,
        machine_index: usize,
        query: Option<&str>,
    ) -> bool {
        self.doc
            .outbound_relations_for_machine(machine_index)
            .chain(self.doc.inbound_relations_for_machine(machine_index))
            .filter(|relation| self.filters.matches_relation(relation))
            .any(|relation| {
                self.doc
                    .relation_detail(relation.index)
                    .is_some_and(|detail| self.relation_matches_query(&detail, query))
            })
    }

    fn machine_heuristic_relations_match_query(
        &self,
        machine_index: usize,
        query: Option<&str>,
    ) -> bool {
        self.heuristic
            .outbound_relations_for_machine(machine_index)
            .chain(self.heuristic.inbound_relations_for_machine(machine_index))
            .filter(|relation| self.heuristic_filters.matches_relation(relation))
            .any(|relation| {
                self.heuristic
                    .relation_detail(&self.doc, relation.index)
                    .is_some_and(|detail| self.heuristic_relation_matches_query(&detail, query))
            })
    }

    fn state_matches_query(&self, state: &CodebaseState, query: Option<&str>) -> bool {
        match self.search_scope {
            SearchScope::Primary => Self::query_matches_any(
                query,
                [
                    state.rust_name.to_owned(),
                    render_state_label(state),
                    state.description.unwrap_or_default().to_owned(),
                ],
            ),
            SearchScope::Docs => {
                Self::query_matches_any(query, [state.docs.unwrap_or_default().to_owned()])
            }
            SearchScope::Relations | SearchScope::Paths => true,
            SearchScope::All => Self::query_matches_any(
                query,
                [
                    state.rust_name.to_owned(),
                    render_state_label(state),
                    state.description.unwrap_or_default().to_owned(),
                    state.docs.unwrap_or_default().to_owned(),
                ],
            ),
        }
    }

    fn transition_matches_query(
        &self,
        transition: &CodebaseTransition,
        query: Option<&str>,
    ) -> bool {
        match self.search_scope {
            SearchScope::Primary => Self::query_matches_any(
                query,
                [
                    transition.method_name.to_owned(),
                    render_transition_label(transition).to_owned(),
                    transition.description.unwrap_or_default().to_owned(),
                ],
            ),
            SearchScope::Docs => {
                Self::query_matches_any(query, [transition.docs.unwrap_or_default().to_owned()])
            }
            SearchScope::Relations | SearchScope::Paths => true,
            SearchScope::All => Self::query_matches_any(
                query,
                [
                    transition.method_name.to_owned(),
                    render_transition_label(transition).to_owned(),
                    transition.description.unwrap_or_default().to_owned(),
                    transition.docs.unwrap_or_default().to_owned(),
                ],
            ),
        }
    }

    fn validator_matches_query(&self, entry: &CodebaseValidatorEntry, query: Option<&str>) -> bool {
        match self.search_scope {
            SearchScope::Primary => Self::query_matches_any(
                query,
                [
                    entry.display_label().into_owned(),
                    entry.source_module_path.to_owned(),
                    entry.source_type_display.to_owned(),
                ],
            ),
            SearchScope::Docs => {
                Self::query_matches_any(query, [entry.docs.unwrap_or_default().to_owned()])
            }
            SearchScope::Relations | SearchScope::Paths => true,
            SearchScope::All => Self::query_matches_any(
                query,
                [
                    entry.display_label().into_owned(),
                    entry.source_module_path.to_owned(),
                    entry.source_type_display.to_owned(),
                    entry.docs.unwrap_or_default().to_owned(),
                ],
            ),
        }
    }

    fn relation_matches_query(
        &self,
        detail: &CodebaseRelationDetail<'_>,
        query: Option<&str>,
    ) -> bool {
        if !self.search_scope.includes_relations() {
            return true;
        }

        let source_specific = match detail.relation.source {
            CodebaseRelationSource::StatePayload { field_name, .. } => {
                field_name.unwrap_or("state_data").to_owned()
            }
            CodebaseRelationSource::MachineField { field_name, .. } => {
                field_name.unwrap_or("<unnamed>").to_owned()
            }
            CodebaseRelationSource::TransitionParam {
                param_name,
                param_index,
                ..
            } => format!("{} {param_index}", param_name.unwrap_or("<unnamed>")),
        };

        let mut candidates = vec![
            detail.relation.kind.display_label().to_owned(),
            detail.relation.basis.display_label().to_owned(),
            detail.relation.semantic.display_label().to_owned(),
            source_specific,
            render_machine_label(detail.source_machine).to_owned(),
            detail.source_machine.rust_type_path.to_owned(),
            detail
                .source_machine
                .description
                .unwrap_or_default()
                .to_owned(),
            detail.source_machine.docs.unwrap_or_default().to_owned(),
            render_machine_label(detail.target_machine).to_owned(),
            detail.target_machine.rust_type_path.to_owned(),
            detail
                .target_machine
                .description
                .unwrap_or_default()
                .to_owned(),
            detail.target_machine.docs.unwrap_or_default().to_owned(),
            render_state_label(detail.target_state),
            detail.target_state.rust_name.to_owned(),
            detail
                .target_state
                .description
                .unwrap_or_default()
                .to_owned(),
            detail.target_state.docs.unwrap_or_default().to_owned(),
            detail
                .relation
                .declared_reference_type
                .unwrap_or_default()
                .to_owned(),
        ];
        if let Some(attested_via) = detail.relation.attested_via.as_ref() {
            candidates.push(attested_via.via_module_path.to_owned());
            candidates.push(attested_via.route_name.to_owned());
        }

        if let Some(state) = detail.source_state {
            candidates.push(render_state_label(state));
            candidates.push(state.rust_name.to_owned());
            candidates.push(state.description.unwrap_or_default().to_owned());
            candidates.push(state.docs.unwrap_or_default().to_owned());
        }

        if let Some(transition) = detail.source_transition {
            candidates.push(render_transition_label(transition).to_owned());
            candidates.push(transition.method_name.to_owned());
            candidates.push(transition.description.unwrap_or_default().to_owned());
            candidates.push(transition.docs.unwrap_or_default().to_owned());
        }
        if let Some(machine) = detail.attested_via_machine {
            candidates.push(render_machine_label(machine).to_owned());
            candidates.push(machine.rust_type_path.to_owned());
        }
        if let Some(state) = detail.attested_via_state {
            candidates.push(render_state_label(state));
            candidates.push(state.rust_name.to_owned());
        }
        if let Some(transition) = detail.attested_via_transition {
            candidates.push(render_transition_label(transition).to_owned());
            candidates.push(transition.method_name.to_owned());
        }
        for producer in &detail.attested_via_producers {
            candidates.push(render_machine_label(producer.machine).to_owned());
            candidates.push(producer.machine.rust_type_path.to_owned());
            candidates.push(render_state_label(producer.state));
            candidates.push(producer.state.rust_name.to_owned());
            candidates.push(render_transition_label(producer.transition).to_owned());
            candidates.push(producer.transition.method_name.to_owned());
            candidates.push(
                producer
                    .transition
                    .description
                    .unwrap_or_default()
                    .to_owned(),
            );
            candidates.push(producer.transition.docs.unwrap_or_default().to_owned());
        }

        Self::query_matches_any(query, candidates)
    }

    fn heuristic_relation_matches_query(
        &self,
        detail: &HeuristicRelationDetail<'_>,
        query: Option<&str>,
    ) -> bool {
        if !self.search_scope.includes_relations() {
            return true;
        }

        let mut candidates = vec![
            detail.relation.evidence_kind.display_label().to_owned(),
            detail.relation.source.kind_label().to_owned(),
            detail.relation.matched_path_text.clone(),
            detail.relation.file_path.display().to_string(),
            detail.relation.line_number.to_string(),
            detail.relation.snippet.clone().unwrap_or_default(),
            render_machine_label(detail.source_machine).to_owned(),
            detail.source_machine.rust_type_path.to_owned(),
            detail
                .source_machine
                .description
                .unwrap_or_default()
                .to_owned(),
            detail.source_machine.docs.unwrap_or_default().to_owned(),
            render_heuristic_source_label(detail),
            render_machine_label(detail.target_machine).to_owned(),
            detail.target_machine.rust_type_path.to_owned(),
            detail
                .target_machine
                .description
                .unwrap_or_default()
                .to_owned(),
            detail.target_machine.docs.unwrap_or_default().to_owned(),
        ];

        if let Some(state) = detail.source_state {
            candidates.push(render_state_label(state));
            candidates.push(state.rust_name.to_owned());
            candidates.push(state.description.unwrap_or_default().to_owned());
            candidates.push(state.docs.unwrap_or_default().to_owned());
        }
        if let Some(transition) = detail.source_transition {
            candidates.push(render_transition_label(transition).to_owned());
            candidates.push(transition.method_name.to_owned());
            candidates.push(transition.description.unwrap_or_default().to_owned());
            candidates.push(transition.docs.unwrap_or_default().to_owned());
        }
        if let Some(method_name) = detail.relation.source.method_name() {
            candidates.push(method_name.to_owned());
        }

        Self::query_matches_any(query, candidates)
    }
}

fn workspace_section_accent(section: WorkspaceSection) -> Color {
    match section {
        WorkspaceSection::Composition => Color::Rgb(214, 176, 67),
        WorkspaceSection::Machines => Color::Rgb(95, 154, 214),
        WorkspaceSection::Gaps => Color::Rgb(226, 104, 81),
    }
}

fn lane_accent(lane_mode: LaneMode) -> Color {
    match lane_mode {
        LaneMode::Exact => Color::Rgb(88, 193, 132),
        LaneMode::Heuristic => Color::Rgb(191, 120, 255),
        LaneMode::Mixed => Color::Rgb(232, 156, 63),
    }
}

fn detail_tab_accent(tab: DetailTab) -> Color {
    match tab {
        DetailTab::Summary => Color::Rgb(95, 154, 214),
        DetailTab::Docs => Color::Rgb(88, 193, 132),
        DetailTab::Diagram => Color::Rgb(72, 178, 164),
        DetailTab::Source => Color::Rgb(232, 156, 63),
        DetailTab::Explain => Color::Rgb(191, 120, 255),
    }
}

fn workspace_diagram_text(doc: &CodebaseDoc) -> Text<'static> {
    Text::from(codebase_render::mermaid(doc))
}

fn machine_diagram_text(doc: &CodebaseDoc, machine: &CodebaseMachine) -> Text<'static> {
    match codebase_render::mermaid_machine_state(doc, machine.index) {
        Ok(diagram) => Text::from(diagram),
        Err(error) => Text::from(format!("Machine Mermaid export failed closed.\n{error}")),
    }
}

fn relation_diagram_text(
    doc: &CodebaseDoc,
    selection: RelationDetailSelection<'_>,
) -> Text<'static> {
    match selection {
        RelationDetailSelection::Exact(detail) => {
            match codebase_render::mermaid_relation_sequence(doc, detail.relation.index) {
                Ok(diagram) => Text::from(diagram),
                Err(error) => Text::from(format!(
                    "Relation Mermaid export failed closed.\n{error}"
                )),
            }
        }
        RelationDetailSelection::Heuristic { .. } => Text::from(
            "Heuristic relations do not have an exact Mermaid export.\nSwitch to an exact relation to view a sequence diagram.",
        ),
    }
}

fn path_diagram_text(path: PathItem) -> Text<'static> {
    match path.kind {
        PathKind::Composition | PathKind::Exact => Text::from(
            "Exact composition-path Mermaid export is not implemented yet.\nUse the machine or relation diagram tabs for current exact Mermaid output.",
        ),
        PathKind::Heuristic => Text::from(
            "Heuristic paths do not have an exact Mermaid export.\nSwitch to an exact machine or relation selection to view Mermaid output.",
        ),
    }
}

fn text_plain_string(text: &Text<'_>) -> String {
    text.lines
        .iter()
        .map(|line| {
            line.spans
                .iter()
                .map(|span| span.content.as_ref())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn mermaid_diagram_source(source: &str) -> Option<&str> {
    source
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty() && !line.starts_with("%%"))
        .filter(|line| {
            line.starts_with("graph ") || line == &"stateDiagram-v2" || line == &"sequenceDiagram"
        })
}

fn render_termaid_preview(
    source: &str,
    width: u16,
    workspace_label: &str,
) -> Result<String, String> {
    let candidates = termaid_candidates(workspace_label);
    render_termaid_preview_with_candidates(source, width, &candidates)
}

fn render_termaid_preview_with_candidates(
    source: &str,
    width: u16,
    candidates: &[OsString],
) -> Result<String, String> {
    let mut last_error = None;
    for candidate in candidates {
        match run_termaid_candidate(candidate, source, width) {
            Ok(output) => return Ok(output),
            Err(TermaidRenderError::NotFound) => continue,
            Err(TermaidRenderError::Failed(message)) => {
                last_error = Some(format!("{}: {message}", candidate.to_string_lossy()));
                break;
            }
        }
    }

    Err(last_error.unwrap_or_else(|| {
        "termaid binary not found; set STATUM_TERMAID_BIN or put `termaid` on PATH".to_owned()
    }))
}

fn termaid_candidates(workspace_label: &str) -> Vec<OsString> {
    if let Some(path) = env::var_os("STATUM_TERMAID_BIN") {
        return vec![path];
    }

    let mut candidates = Vec::new();
    if let Some(sibling) = sibling_termaid_binary(workspace_label) {
        candidates.push(sibling.into_os_string());
    }
    candidates.push(OsString::from("termaid"));
    candidates
}

fn sibling_termaid_binary(workspace_label: &str) -> Option<PathBuf> {
    let manifest_path = Path::new(workspace_label);
    let workspace_dir = if manifest_path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name == "Cargo.toml")
    {
        manifest_path.parent()?
    } else {
        manifest_path
    };
    let parent = workspace_dir.parent()?;
    let candidate = parent
        .join("termaid")
        .join("target")
        .join("release")
        .join("termaid");
    candidate.is_file().then_some(candidate)
}

fn run_termaid_candidate(
    candidate: &OsString,
    source: &str,
    width: u16,
) -> Result<String, TermaidRenderError> {
    let mut command = Command::new(candidate);
    command
        .arg("--width")
        .arg(width.to_string())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Err(TermaidRenderError::NotFound);
        }
        Err(error) => {
            return Err(TermaidRenderError::Failed(error.to_string()));
        }
    };

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(source.as_bytes())
            .map_err(|error| TermaidRenderError::Failed(error.to_string()))?;
    }

    let output = child
        .wait_with_output()
        .map_err(|error| TermaidRenderError::Failed(error.to_string()))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        let detail = if stderr.is_empty() { stdout } else { stderr };
        Err(TermaidRenderError::Failed(if detail.is_empty() {
            format!("exit status {}", output.status)
        } else {
            detail
        }))
    }
}

enum TermaidRenderError {
    NotFound,
    Failed(String),
}

fn severity_accent(severity: CompositionSuggestionSeverity) -> Color {
    match severity {
        CompositionSuggestionSeverity::Warning => Color::Rgb(226, 104, 81),
        CompositionSuggestionSeverity::Suggestion => Color::Rgb(214, 176, 67),
    }
}

fn muted_color() -> Color {
    Color::Rgb(129, 140, 155)
}

fn badge(label: impl Into<String>, fg: Color, bg: Color) -> Span<'static> {
    Span::styled(
        format!(" {} ", label.into()),
        Style::default().fg(fg).bg(bg).add_modifier(Modifier::BOLD),
    )
}

fn ghost_badge(label: impl Into<String>, color: Color) -> Span<'static> {
    Span::styled(
        format!("[{}]", label.into()),
        Style::default().fg(color).add_modifier(Modifier::BOLD),
    )
}

fn styled_label(label: impl Into<String>, color: Color) -> Span<'static> {
    Span::styled(
        label.into(),
        Style::default().fg(color).add_modifier(Modifier::BOLD),
    )
}

fn selected_list_style(accent: Color) -> Style {
    Style::default()
        .fg(Color::Black)
        .bg(accent)
        .add_modifier(Modifier::BOLD)
}

fn title_style(accent: Color, focused: bool) -> Style {
    let base = if focused { accent } else { muted_color() };
    Style::default().fg(base).add_modifier(Modifier::BOLD)
}

fn titled_block(title: impl Into<Line<'static>>, accent: Color, focused: bool) -> Block<'static> {
    let border_style = if focused {
        Style::default().fg(accent)
    } else {
        Style::default().fg(mutated_color_fallback(accent))
    };
    Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(border_style)
}

fn mutated_color_fallback(accent: Color) -> Color {
    match accent {
        Color::Rgb(_, _, _) => muted_color(),
        other => other,
    }
}

fn render_metric_card(
    frame: &mut Frame,
    area: Rect,
    label: &str,
    value: impl Into<String>,
    note: impl Into<String>,
    accent: Color,
) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(accent));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let note = note.into();
    let mut lines = vec![
        Line::from(styled_label(label.to_owned(), accent)),
        Line::from(Span::styled(
            value.into(),
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )),
    ];
    if !note.is_empty() {
        lines.push(Line::from(Span::styled(
            note,
            Style::default().fg(mutated_color_fallback(accent)),
        )));
    }
    frame.render_widget(
        Paragraph::new(Text::from(lines)).wrap(Wrap { trim: false }),
        inner,
    );
}

fn join_spans(mut spans: Vec<Span<'static>>) -> Line<'static> {
    let mut joined = Vec::new();
    for (index, span) in spans.drain(..).enumerate() {
        if index > 0 {
            joined.push(Span::raw(" "));
        }
        joined.push(span);
    }
    Line::from(joined)
}

fn push_match_reason_line(lines: &mut Vec<Line<'static>>, reason: Option<String>) {
    if let Some(reason) = reason {
        lines.push(Line::from(vec![
            ghost_badge("match", detail_tab_accent(DetailTab::Source)),
            Span::raw(" "),
            Span::styled(
                reason,
                Style::default().fg(mutated_color_fallback(detail_tab_accent(DetailTab::Source))),
            ),
        ]));
    }
}

fn compact_inline_text(text: &str, limit: usize) -> String {
    let compact = normalize_inline_text(text);
    let char_count = compact.chars().count();
    if char_count <= limit {
        return compact;
    }
    compact
        .chars()
        .take(limit.saturating_sub(3))
        .collect::<String>()
        + "..."
}

fn match_excerpt(text: &str, query: &str, limit: usize) -> String {
    let compact = normalize_inline_text(text);
    let total_chars = compact.chars().count();
    if total_chars <= limit {
        return compact;
    }

    let lower = compact.to_ascii_lowercase();
    let Some(byte_index) = lower.find(query) else {
        return compact_inline_text(&compact, limit);
    };

    let match_char_index = compact[..byte_index].chars().count();
    let window_start = match_char_index.saturating_sub(limit / 3);
    let window_end = (window_start + limit).min(total_chars);
    let mut excerpt = compact
        .chars()
        .skip(window_start)
        .take(window_end - window_start)
        .collect::<String>();
    if window_start > 0 {
        excerpt = format!("...{excerpt}");
    }
    if window_end < total_chars {
        excerpt.push_str("...");
    }
    excerpt
}

fn first_match_reason<I>(query: Option<&str>, candidates: I) -> Option<String>
where
    I: IntoIterator<Item = (&'static str, String)>,
{
    let query = query?;
    for (label, candidate) in candidates {
        let compact = normalize_inline_text(&candidate);
        if compact.to_ascii_lowercase().contains(query) {
            return Some(format!("{label}: {}", match_excerpt(&compact, query, 44)));
        }
    }
    None
}

fn normalize_inline_text(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn line_is_blank(line: &Line<'_>) -> bool {
    line.spans
        .iter()
        .all(|span| span.content.as_ref().trim().is_empty())
}

fn trim_blank_lines(lines: &mut Vec<Line<'static>>) {
    while lines.first().is_some_and(line_is_blank) {
        lines.remove(0);
    }
    while lines.last().is_some_and(line_is_blank) {
        lines.pop();
    }
}

fn split_text_for_cards(
    text: Text<'static>,
    preferred_head_lines: usize,
) -> (Text<'static>, Text<'static>) {
    let mut lines = text.lines;
    if lines.is_empty() {
        return (
            Text::from(Vec::<Line<'static>>::new()),
            Text::from(Vec::<Line<'static>>::new()),
        );
    }

    let split_index = lines
        .iter()
        .enumerate()
        .skip(1)
        .find(|(_, line)| line_is_blank(line))
        .map(|(index, _)| index)
        .unwrap_or_else(|| preferred_head_lines.min(lines.len()));

    let mut tail = lines.split_off(split_index);
    trim_blank_lines(&mut lines);
    trim_blank_lines(&mut tail);
    (Text::from(lines), Text::from(tail))
}

fn text_is_empty(text: &Text<'_>) -> bool {
    text.lines.is_empty()
        || text.lines.iter().all(|line| {
            line.spans
                .iter()
                .all(|span| span.content.as_ref().trim().is_empty())
        })
}

fn render_text_card(
    frame: &mut Frame,
    area: Rect,
    badge_label: &str,
    title: &str,
    accent: Color,
    text: Text<'static>,
) {
    let block = Block::default()
        .title(Line::from(vec![
            badge(badge_label, Color::Black, accent),
            Span::raw(" "),
            Span::styled(title.to_owned(), title_style(accent, true)),
        ]))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(accent));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    frame.render_widget(
        Paragraph::new(text)
            .wrap(Wrap { trim: false })
            .style(Style::default().fg(Color::White)),
        inner,
    );
}

fn subdued_line(text: impl Into<String>, accent: Color) -> Line<'static> {
    Line::from(Span::styled(
        text.into(),
        Style::default().fg(mutated_color_fallback(accent)),
    ))
}

fn plural_suffix(count: usize) -> &'static str {
    if count == 1 {
        ""
    } else {
        "s"
    }
}

fn first_text_excerpt(
    description: Option<&'static str>,
    docs: Option<&'static str>,
    fallback: &str,
) -> String {
    description
        .into_iter()
        .chain(docs)
        .flat_map(str::lines)
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or(fallback)
        .to_owned()
}

fn transition_target_summary(machine: &CodebaseMachine, transition: &CodebaseTransition) -> String {
    let labels = transition
        .to
        .iter()
        .take(3)
        .map(|state_index| {
            machine
                .state(*state_index)
                .map(render_state_label)
                .unwrap_or_else(|| format!("state {state_index}"))
        })
        .collect::<Vec<_>>();
    if labels.is_empty() {
        "<none>".to_owned()
    } else if transition.to.len() > labels.len() {
        format!(
            "{} +{}",
            labels.join(", "),
            transition.to.len() - labels.len()
        )
    } else {
        labels.join(", ")
    }
}

fn validator_target_summary(machine: &CodebaseMachine, entry: &CodebaseValidatorEntry) -> String {
    let labels = entry
        .target_states
        .iter()
        .take(3)
        .map(|state_index| {
            machine
                .state(*state_index)
                .map(render_state_label)
                .unwrap_or_else(|| format!("state {state_index}"))
        })
        .collect::<Vec<_>>();
    if labels.is_empty() {
        "<none>".to_owned()
    } else if entry.target_states.len() > labels.len() {
        format!(
            "{} +{}",
            labels.join(", "),
            entry.target_states.len() - labels.len()
        )
    } else {
        labels.join(", ")
    }
}

fn relation_origin_label(detail: &CodebaseRelationDetail<'_>) -> String {
    if let Some(transition) = detail.source_transition {
        format!("transition {}", render_transition_label(transition))
    } else if let Some(state) = detail.source_state {
        format!("state {}", render_state_label(state))
    } else {
        format!("machine {}", render_machine_label(detail.source_machine))
    }
}

fn compact_file_label(path: &std::path::Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(str::to_owned)
        .unwrap_or_else(|| path.display().to_string())
}

fn path_step_preview(item: &PathItem, doc: &CodebaseDoc) -> String {
    let Some(first_step) = item.steps.first() else {
        return "no visible steps".to_owned();
    };
    let mut machine_labels = vec![doc
        .machine(first_step.from_machine)
        .map(render_machine_label)
        .unwrap_or("<missing machine>")
        .to_owned()];
    for step in item.steps.iter().take(3) {
        machine_labels.push(
            doc.machine(step.to_machine)
                .map(render_machine_label)
                .unwrap_or("<missing machine>")
                .to_owned(),
        );
    }
    let mut route = machine_labels.join(" -> ");
    if item.steps.len() > 3 {
        route.push_str(" -> ...");
    }
    if let Some(first_label) = item.steps.first().map(|step| step.label.as_str()) {
        format!("{route}  |  {first_label}")
    } else {
        route
    }
}

fn render_machine_label(machine: &CodebaseMachine) -> &str {
    machine.label.unwrap_or(machine.rust_type_path)
}

fn render_state_label(state: &CodebaseState) -> String {
    let base = match state.label {
        Some(label) => label.to_owned(),
        None if state.has_data => format!("{} (data)", state.rust_name),
        None => state.rust_name.to_owned(),
    };
    if state.direct_construction_available {
        format!("{base} [build]")
    } else {
        base
    }
}

fn render_transition_label(transition: &CodebaseTransition) -> &str {
    transition.label.unwrap_or(transition.method_name)
}

fn render_heuristic_source_label(detail: &HeuristicRelationDetail<'_>) -> String {
    match &detail.relation.source {
        HeuristicRelationSource::State { .. } => detail
            .source_state
            .map(render_state_label)
            .unwrap_or_else(|| "<missing state>".to_owned()),
        HeuristicRelationSource::Transition { .. } => detail
            .source_transition
            .map(|transition| render_transition_label(transition).to_owned())
            .unwrap_or_else(|| "<missing transition>".to_owned()),
        HeuristicRelationSource::Method { method_name, .. } => {
            if let Some(state) = detail.source_state {
                format!("{}::{}", render_state_label(state), method_name)
            } else {
                method_name.clone()
            }
        }
    }
}

fn exact_covers_heuristic_relation(
    exact: &CodebaseRelation,
    heuristic: &HeuristicRelation,
) -> bool {
    if exact.source_machine() != heuristic.source.machine()
        || exact.target_machine != heuristic.target_machine
    {
        return false;
    }

    match heuristic.source.transition() {
        Some(transition) => exact.source_transition() == Some(transition),
        None => match heuristic.source.state() {
            Some(state) => exact.source_state() == Some(state),
            None => true,
        },
    }
}

fn render_relation_label(detail: &CodebaseRelationDetail<'_>) -> String {
    let prefix = if detail.relation.is_composition_owned() {
        "[exact][composition]"
    } else {
        "[exact]"
    };
    format!(
        "{prefix} {} ({}) -> {} :: {}",
        detail.relation.kind.display_label(),
        detail.relation.basis.display_label(),
        render_machine_label(detail.target_machine),
        render_state_label(detail.target_state)
    )
}

fn render_heuristic_relation_label(detail: &HeuristicRelationDetail<'_>) -> String {
    format!(
        "[heur] {} -> {} ({})",
        render_machine_label(detail.target_machine),
        render_heuristic_source_label(detail),
        detail.relation.evidence_kind.display_label()
    )
}

fn machine_detail_text(
    machine: &CodebaseMachine,
    doc: &CodebaseDoc,
    suggestions: &[&CompositionSuggestion],
) -> Text<'static> {
    let mut lines = vec![
        Line::from(format!("machine: {}", render_machine_label(machine))),
        Line::from(format!("path: {}", machine.rust_type_path)),
        Line::from(format!("role: {}", machine.role.display_label())),
        Line::from(format!("states: {}", machine.states.len())),
        Line::from(format!("transitions: {}", machine.transitions.len())),
        Line::from(format!("validators: {}", machine.validator_entries.len())),
        Line::from(format!(
            "outbound exact relations: {}",
            doc.outbound_relations_for_machine(machine.index).count()
        )),
        Line::from(format!(
            "inbound exact relations: {}",
            doc.inbound_relations_for_machine(machine.index).count()
        )),
    ];
    if !suggestions.is_empty() {
        append_composition_suggestions(&mut lines, suggestions, doc);
    }
    append_description_and_docs(&mut lines, machine.description, machine.docs);
    Text::from(lines)
}

fn state_detail_text(state: &CodebaseState) -> Text<'static> {
    let mut lines = vec![
        Line::from(format!("state: {}", render_state_label(state))),
        Line::from(format!("rust name: {}", state.rust_name)),
        Line::from(format!("has data: {}", yes_no(state.has_data))),
        Line::from(format!(
            "direct construction: {}",
            yes_no(state.direct_construction_available)
        )),
        Line::from(format!("graph root: {}", yes_no(state.is_graph_root))),
    ];
    append_description_and_docs(&mut lines, state.description, state.docs);
    Text::from(lines)
}

fn transition_detail_text(transition: &CodebaseTransition) -> Text<'static> {
    let mut lines = vec![
        Line::from(format!(
            "transition: {}",
            render_transition_label(transition)
        )),
        Line::from(format!("method: {}", transition.method_name)),
        Line::from(format!("from state index: {}", transition.from)),
        Line::from(format!("target count: {}", transition.to.len())),
        Line::from(format!("targets: {:?}", transition.to)),
    ];
    append_description_and_docs(&mut lines, transition.description, transition.docs);
    Text::from(lines)
}

fn validator_detail_text(entry: &CodebaseValidatorEntry) -> Text<'static> {
    let mut lines = vec![
        Line::from(format!("validator: {}", entry.display_label())),
        Line::from(format!("module: {}", entry.source_module_path)),
        Line::from(format!("target states: {:?}", entry.target_states)),
    ];
    append_description_and_docs(&mut lines, None, entry.docs);
    Text::from(lines)
}

#[cfg(test)]
fn summary_detail_text(summary: &SummaryItem, doc: &CodebaseDoc) -> Text<'static> {
    let (lane_label, direction, group_from, group_to, label, relation_count) = match summary {
        SummaryItem::Exact(summary) => (
            "exact",
            summary.direction,
            summary.group.from_machine,
            summary.group.to_machine,
            summary.group.display_label(),
            summary.group.relation_indices.len(),
        ),
        SummaryItem::Heuristic(summary) => (
            "heuristic",
            summary.direction,
            summary.group.from_machine,
            summary.group.to_machine,
            summary.group.display_label(),
            summary.group.relation_indices.len(),
        ),
    };
    let (source_machine, target_machine) = match direction {
        SummaryDirection::Outbound => (group_from, group_to),
        SummaryDirection::Inbound => (group_from, group_to),
    };
    let source_machine = doc
        .machine(source_machine)
        .expect("summary source machine should exist");
    let target_machine = doc
        .machine(target_machine)
        .expect("summary target machine should exist");
    let direction_label = match direction {
        SummaryDirection::Outbound => "outbound",
        SummaryDirection::Inbound => "inbound",
    };

    let card_label = match summary {
        SummaryItem::Exact(summary)
            if summary.group.semantic != CodebaseMachineRelationGroupSemantic::Exact =>
        {
            "composition relationship card"
        }
        SummaryItem::Exact(_) => "exact relationship card",
        SummaryItem::Heuristic(_) => "heuristic relationship card",
    };

    let mut lines = vec![
        Line::from(card_label),
        Line::from(format!("{direction_label} {lane_label} summary edge")),
        Line::from(format!("from: {}", render_machine_label(source_machine))),
        Line::from(format!("to: {}", render_machine_label(target_machine))),
        Line::from(format!("semantic: {}", summary_group_semantic(summary))),
        Line::from(format!("label: {label}")),
        Line::from(format!("relation count: {relation_count}")),
    ];

    append_named_description_and_docs(
        &mut lines,
        "source machine",
        source_machine.description,
        source_machine.docs,
    );
    append_named_description_and_docs(
        &mut lines,
        "target machine",
        target_machine.description,
        target_machine.docs,
    );

    Text::from(lines)
}

fn gap_detail_text(gap: &CompositionSuggestion, doc: &CodebaseDoc) -> Text<'static> {
    let source_machine = gap.source_machine(doc);
    let target_machine = gap.target_machine(doc);
    let mut lines = vec![
        Line::from(format!("severity: {}", gap.severity.display_label())),
        Line::from(format!("kind: {}", gap.kind.display_label())),
        Line::from(format!(
            "source machine: {}",
            source_machine
                .map(render_machine_label)
                .unwrap_or("<missing machine>")
        )),
        Line::from(format!(
            "target machine: {}",
            target_machine
                .map(render_machine_label)
                .unwrap_or("<missing machine>")
        )),
        Line::from(format!("why: {}", gap.why_text())),
        Line::from(format!("evidence: {}", gap.counts_label())),
        Line::from(format!("help: {}", gap.help_text())),
    ];

    if let Some(source_machine) = source_machine {
        append_named_description_and_docs(
            &mut lines,
            "source machine",
            source_machine.description,
            source_machine.docs,
        );
    }
    if let Some(target_machine) = target_machine {
        append_named_description_and_docs(
            &mut lines,
            "target machine",
            target_machine.description,
            target_machine.docs,
        );
    }

    Text::from(lines)
}

fn path_detail_text(path: &PathItem, doc: &CodebaseDoc) -> Text<'static> {
    let source_machine = path
        .steps
        .first()
        .and_then(|step| doc.machine(step.from_machine));
    let target_machine = doc.machine(path.target_machine);
    let mut lines = vec![
        Line::from(format!("path kind: {}", path.kind.display_label())),
        Line::from(format!("hop count: {}", path.steps.len())),
        Line::from(format!(
            "from: {}",
            source_machine
                .map(render_machine_label)
                .unwrap_or("<missing machine>")
        )),
        Line::from(format!(
            "to: {}",
            target_machine
                .map(render_machine_label)
                .unwrap_or("<missing machine>")
        )),
    ];

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Steps".to_owned(),
        Style::default().add_modifier(Modifier::BOLD),
    )));
    for (index, step) in path.steps.iter().enumerate() {
        let from = doc
            .machine(step.from_machine)
            .map(render_machine_label)
            .unwrap_or("<missing machine>");
        let to = doc
            .machine(step.to_machine)
            .map(render_machine_label)
            .unwrap_or("<missing machine>");
        lines.push(Line::from(format!(
            "{}. [{}] {} -> {} : {}",
            index + 1,
            step.kind.display_label(),
            from,
            to,
            step.label
        )));
    }

    if let Some(source_machine) = source_machine {
        append_named_description_and_docs(
            &mut lines,
            "source machine",
            source_machine.description,
            source_machine.docs,
        );
    }
    if let Some(target_machine) = target_machine {
        append_named_description_and_docs(
            &mut lines,
            "target machine",
            target_machine.description,
            target_machine.docs,
        );
    }

    Text::from(lines)
}

fn relation_detail_selection_text(detail: RelationDetailSelection<'_>) -> Text<'static> {
    match detail {
        RelationDetailSelection::Exact(detail) => relation_detail_text(detail),
        RelationDetailSelection::Heuristic {
            detail,
            shadowed_by_exact,
        } => heuristic_relation_detail_text(detail, shadowed_by_exact),
    }
}

fn relation_detail_text(detail: CodebaseRelationDetail<'_>) -> Text<'static> {
    let source = match detail.relation.source {
        CodebaseRelationSource::StatePayload { field_name, .. } => format!(
            "state payload{}",
            field_name
                .map(|name| format!(" field `{name}`"))
                .unwrap_or_default()
        ),
        CodebaseRelationSource::MachineField { field_name, .. } => {
            format!("machine field {}", field_name.unwrap_or("<unnamed>"))
        }
        CodebaseRelationSource::TransitionParam {
            param_name,
            param_index,
            ..
        } => format!(
            "transition param {} ({})",
            param_name.unwrap_or("<unnamed>"),
            param_index
        ),
    };

    let mut lines = vec![
        Line::from(if detail.relation.is_composition_owned() {
            "composition relationship card"
        } else {
            "exact relationship card"
        }),
        Line::from(format!("kind: {}", detail.relation.kind.display_label())),
        Line::from(format!("basis: {}", detail.relation.basis.display_label())),
        Line::from(format!(
            "semantic: {}",
            detail.relation.semantic.display_label()
        )),
        Line::from(format!(
            "source machine: {}",
            render_machine_label(detail.source_machine)
        )),
        Line::from(format!(
            "source machine role: {}",
            detail.source_machine.role.display_label()
        )),
        Line::from(format!("source: {source}")),
    ];

    if let Some(state) = detail.source_state {
        lines.push(Line::from(format!(
            "source state: {}",
            render_state_label(state)
        )));
    }
    if let Some(transition) = detail.source_transition {
        lines.push(Line::from(format!(
            "source transition: {}",
            render_transition_label(transition)
        )));
    }
    lines.push(Line::from(format!(
        "target machine: {}",
        render_machine_label(detail.target_machine)
    )));
    lines.push(Line::from(format!(
        "target machine role: {}",
        detail.target_machine.role.display_label()
    )));
    lines.push(Line::from(format!(
        "target state: {}",
        render_state_label(detail.target_state)
    )));
    if let Some(reference_type) = detail.relation.declared_reference_type {
        lines.push(Line::from(format!("declared ref type: {reference_type}")));
    }
    if let Some(attested_via) = detail.relation.attested_via.as_ref() {
        lines.push(Line::from(format!(
            "attested via: {}::{}",
            attested_via.via_module_path, attested_via.route_name
        )));
    }
    if detail.attested_via_producers.len() == 1 {
        if let Some(machine) = detail.attested_via_machine {
            lines.push(Line::from(format!(
                "producer machine: {}",
                render_machine_label(machine)
            )));
        }
        if let Some(state) = detail.attested_via_state {
            lines.push(Line::from(format!(
                "producer state: {}",
                render_state_label(state)
            )));
        }
        if let Some(transition) = detail.attested_via_transition {
            lines.push(Line::from(format!(
                "producer transition: {}",
                render_transition_label(transition)
            )));
        }
    } else if !detail.attested_via_producers.is_empty() {
        lines.push(Line::from(format!(
            "producer transitions: {}",
            detail.attested_via_producers.len()
        )));
        for producer in &detail.attested_via_producers {
            lines.push(Line::from(format!(
                "  - {} / {} / {}",
                render_machine_label(producer.machine),
                render_state_label(producer.state),
                render_transition_label(producer.transition)
            )));
        }
    }

    append_named_description_and_docs(
        &mut lines,
        "source machine",
        detail.source_machine.description,
        detail.source_machine.docs,
    );
    if let Some(state) = detail.source_state {
        append_named_description_and_docs(
            &mut lines,
            "source state",
            state.description,
            state.docs,
        );
    }
    if let Some(transition) = detail.source_transition {
        append_named_description_and_docs(
            &mut lines,
            "source transition",
            transition.description,
            transition.docs,
        );
    }
    append_named_description_and_docs(
        &mut lines,
        "target machine",
        detail.target_machine.description,
        detail.target_machine.docs,
    );
    append_named_description_and_docs(
        &mut lines,
        "target state",
        detail.target_state.description,
        detail.target_state.docs,
    );
    if detail.attested_via_producers.len() == 1 {
        if let Some(machine) = detail.attested_via_machine {
            append_named_description_and_docs(
                &mut lines,
                "producer machine",
                machine.description,
                machine.docs,
            );
        }
        if let Some(state) = detail.attested_via_state {
            append_named_description_and_docs(
                &mut lines,
                "producer state",
                state.description,
                state.docs,
            );
        }
        if let Some(transition) = detail.attested_via_transition {
            append_named_description_and_docs(
                &mut lines,
                "producer transition",
                transition.description,
                transition.docs,
            );
        }
    } else {
        for (index, producer) in detail.attested_via_producers.iter().enumerate() {
            let prefix = format!("producer {}", index + 1);
            append_named_description_and_docs(
                &mut lines,
                &format!("{prefix} machine"),
                producer.machine.description,
                producer.machine.docs,
            );
            append_named_description_and_docs(
                &mut lines,
                &format!("{prefix} state"),
                producer.state.description,
                producer.state.docs,
            );
            append_named_description_and_docs(
                &mut lines,
                &format!("{prefix} transition"),
                producer.transition.description,
                producer.transition.docs,
            );
        }
    }

    Text::from(lines)
}

fn classify_group_semantic(
    composition_owned_relations: usize,
    total_relations: usize,
) -> CodebaseMachineRelationGroupSemantic {
    if composition_owned_relations == 0 {
        CodebaseMachineRelationGroupSemantic::Exact
    } else if composition_owned_relations == total_relations {
        CodebaseMachineRelationGroupSemantic::CompositionDirectChild
    } else {
        CodebaseMachineRelationGroupSemantic::Mixed
    }
}

#[cfg(test)]
fn summary_group_semantic(summary: &SummaryItem) -> &'static str {
    match summary {
        SummaryItem::Exact(summary) => summary.group.semantic.display_label(),
        SummaryItem::Heuristic(_) => "heuristic",
    }
}

fn heuristic_relation_detail_text(
    detail: HeuristicRelationDetail<'_>,
    shadowed_by_exact: bool,
) -> Text<'static> {
    let mut lines = vec![
        Line::from("heuristic relation"),
        Line::from("basis: source-scanned affinity"),
        Line::from(format!(
            "evidence: {}",
            detail.relation.evidence_kind.display_label()
        )),
        Line::from(format!(
            "source kind: {}",
            detail.relation.source.kind_label()
        )),
        Line::from(format!(
            "source machine: {}",
            render_machine_label(detail.source_machine)
        )),
        Line::from(format!(
            "source item: {}",
            render_heuristic_source_label(&detail)
        )),
        Line::from(format!(
            "target machine: {}",
            render_machine_label(detail.target_machine)
        )),
        Line::from(format!(
            "matched path: {}",
            detail.relation.matched_path_text
        )),
        Line::from(format!(
            "location: {}:{}",
            detail.relation.file_path.display(),
            detail.relation.line_number
        )),
    ];
    if shadowed_by_exact {
        lines.push(Line::from(
            "exact lane already covers this relationship and will hide it in mixed mode.",
        ));
    }
    if let Some(snippet) = detail.relation.snippet.as_deref() {
        lines.push(Line::from(format!("snippet: {snippet}")));
    }

    append_named_description_and_docs(
        &mut lines,
        "source machine",
        detail.source_machine.description,
        detail.source_machine.docs,
    );
    if let Some(state) = detail.source_state {
        append_named_description_and_docs(
            &mut lines,
            "source state",
            state.description,
            state.docs,
        );
    }
    if let Some(transition) = detail.source_transition {
        append_named_description_and_docs(
            &mut lines,
            "source transition",
            transition.description,
            transition.docs,
        );
    }
    append_named_description_and_docs(
        &mut lines,
        "target machine",
        detail.target_machine.description,
        detail.target_machine.docs,
    );

    Text::from(lines)
}

fn heuristic_status_text(
    status: HeuristicStatusKind,
    diagnostics: &[HeuristicDiagnostic],
) -> Text<'static> {
    let mut lines = vec![Line::from(format!("heuristics {}", status.display_label()))];
    for diagnostic in diagnostics.iter().take(4) {
        lines.push(Line::from(""));
        lines.push(Line::from(diagnostic.display_label()));
    }
    Text::from(lines)
}

fn discover_paths(
    source_machine: usize,
    adjacency: &BTreeMap<usize, Vec<PathStep>>,
) -> BTreeMap<usize, Vec<PathStep>> {
    use std::collections::VecDeque;

    let mut queue = VecDeque::from([source_machine]);
    let mut seen = BTreeSet::from([source_machine]);
    let mut previous = BTreeMap::<usize, PathStep>::new();

    while let Some(current) = queue.pop_front() {
        for step in adjacency.get(&current).into_iter().flatten() {
            if seen.insert(step.to_machine) {
                previous.insert(step.to_machine, step.clone());
                queue.push_back(step.to_machine);
            }
        }
    }

    let mut paths = BTreeMap::new();
    for &target_machine in previous.keys() {
        let mut steps = Vec::new();
        let mut current = target_machine;
        while let Some(step) = previous.get(&current) {
            steps.push(step.clone());
            current = step.from_machine;
        }
        steps.reverse();
        paths.insert(target_machine, steps);
    }

    paths
}

fn append_composition_suggestions(
    lines: &mut Vec<Line<'static>>,
    suggestions: &[&CompositionSuggestion],
    doc: &CodebaseDoc,
) {
    let (warnings, suggestions_count) =
        suggestions
            .iter()
            .fold((0usize, 0usize), |counts, item| match item.severity {
                CompositionSuggestionSeverity::Warning => (counts.0 + 1, counts.1),
                CompositionSuggestionSeverity::Suggestion => (counts.0, counts.1 + 1),
            });

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Composition Diagnostics".to_owned(),
        Style::default().add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(format!(
        "{} warning, {} suggestion",
        warnings, suggestions_count
    )));

    for suggestion in suggestions {
        lines.push(Line::from(""));
        lines.push(Line::from(format!(
            "{}: {}",
            suggestion.severity.display_label(),
            suggestion.summary_label(doc)
        )));
        lines.push(Line::from(format!(
            "kind: {}",
            suggestion.kind.display_label()
        )));
        lines.push(Line::from(format!("why: {}", suggestion.why_text())));
        lines.push(Line::from(format!(
            "evidence: {}",
            suggestion.counts_label()
        )));
        lines.push(Line::from(format!("help: {}", suggestion.help_text())));
    }
}

fn append_description_and_docs(
    lines: &mut Vec<Line<'static>>,
    description: Option<&'static str>,
    docs: Option<&'static str>,
) {
    append_named_description_and_docs(lines, "", description, docs);
}

fn append_named_description_and_docs(
    lines: &mut Vec<Line<'static>>,
    prefix: &str,
    description: Option<&'static str>,
    docs: Option<&'static str>,
) {
    if let Some(description) = description {
        append_text_section(lines, section_label(prefix, "Description"), description);
    }
    if let Some(docs) = docs {
        append_text_section(lines, section_label(prefix, "Docs"), docs);
    }
}

fn append_text_section(lines: &mut Vec<Line<'static>>, heading: String, text: &'static str) {
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        heading,
        Style::default().add_modifier(Modifier::BOLD),
    )));
    for line in text.lines() {
        lines.push(Line::from(line.to_owned()));
    }
}

fn section_label(prefix: &str, suffix: &str) -> String {
    if prefix.is_empty() {
        suffix.to_owned()
    } else {
        format!("{prefix} {suffix}")
    }
}

fn yes_no(value: bool) -> &'static str {
    if value {
        "yes"
    } else {
        "no"
    }
}

fn docs_text(description: Option<&'static str>, docs: Option<&'static str>) -> Text<'static> {
    let mut lines = Vec::new();
    append_description_and_docs(&mut lines, description, docs);
    if lines.is_empty() {
        Text::from("No source-local docs.")
    } else {
        Text::from(lines)
    }
}

fn workspace_docs_text() -> Text<'static> {
    Text::from(vec![
        Line::from("Workspace diagrams do not have source-local docs of their own."),
        Line::from(
            "Use the workspace diagram to orient on exact linked structure, then drill into machines, states, or transitions for local docs.",
        ),
    ])
}

fn relation_docs_text(selection: RelationDetailSelection<'_>) -> Text<'static> {
    let mut lines = Vec::new();
    match selection {
        RelationDetailSelection::Exact(detail) => {
            append_named_description_and_docs(
                &mut lines,
                "source machine",
                detail.source_machine.description,
                detail.source_machine.docs,
            );
            if let Some(state) = detail.source_state {
                append_named_description_and_docs(
                    &mut lines,
                    "source state",
                    state.description,
                    state.docs,
                );
            }
            if let Some(transition) = detail.source_transition {
                append_named_description_and_docs(
                    &mut lines,
                    "source transition",
                    transition.description,
                    transition.docs,
                );
            }
            append_named_description_and_docs(
                &mut lines,
                "target machine",
                detail.target_machine.description,
                detail.target_machine.docs,
            );
            append_named_description_and_docs(
                &mut lines,
                "target state",
                detail.target_state.description,
                detail.target_state.docs,
            );
        }
        RelationDetailSelection::Heuristic { detail, .. } => {
            append_named_description_and_docs(
                &mut lines,
                "source machine",
                detail.source_machine.description,
                detail.source_machine.docs,
            );
            if let Some(state) = detail.source_state {
                append_named_description_and_docs(
                    &mut lines,
                    "source state",
                    state.description,
                    state.docs,
                );
            }
            if let Some(transition) = detail.source_transition {
                append_named_description_and_docs(
                    &mut lines,
                    "source transition",
                    transition.description,
                    transition.docs,
                );
            }
            append_named_description_and_docs(
                &mut lines,
                "target machine",
                detail.target_machine.description,
                detail.target_machine.docs,
            );
        }
    }
    if lines.is_empty() {
        Text::from("No source-local docs for the current relation.")
    } else {
        Text::from(lines)
    }
}

fn path_docs_text(path: &PathItem, doc: &CodebaseDoc) -> Text<'static> {
    let mut lines = Vec::new();
    if let Some(source_machine) = path
        .steps
        .first()
        .and_then(|step| doc.machine(step.from_machine))
    {
        append_named_description_and_docs(
            &mut lines,
            "source machine",
            source_machine.description,
            source_machine.docs,
        );
    }
    if let Some(target_machine) = doc.machine(path.target_machine) {
        append_named_description_and_docs(
            &mut lines,
            "target machine",
            target_machine.description,
            target_machine.docs,
        );
    }
    if lines.is_empty() {
        Text::from("No source-local docs for the current path.")
    } else {
        Text::from(lines)
    }
}

fn gap_docs_text(gap: &CompositionSuggestion, doc: &CodebaseDoc) -> Text<'static> {
    let mut lines = Vec::new();
    if let Some(source_machine) = gap.source_machine(doc) {
        append_named_description_and_docs(
            &mut lines,
            "source machine",
            source_machine.description,
            source_machine.docs,
        );
    }
    if let Some(target_machine) = gap.target_machine(doc) {
        append_named_description_and_docs(
            &mut lines,
            "target machine",
            target_machine.description,
            target_machine.docs,
        );
    }
    if lines.is_empty() {
        Text::from("No source-local docs for the current gap.")
    } else {
        Text::from(lines)
    }
}

fn machine_source_text(machine: &CodebaseMachine) -> Text<'static> {
    Text::from(vec![
        Line::from(format!("machine path: {}", machine.rust_type_path)),
        Line::from(format!("module path: {}", machine.module_path)),
        Line::from("definition location: not available in current linked exact surface"),
    ])
}

fn workspace_source_text(workspace_label: &str, doc: &CodebaseDoc) -> Text<'static> {
    let machine_count = doc.machines().len();
    let composition_count = doc
        .machines()
        .iter()
        .filter(|machine| machine.role.is_composition())
        .count();
    let relation_count = doc
        .relations()
        .iter()
        .filter(|relation| relation.source_machine() != relation.target_machine)
        .count();
    Text::from(vec![
        Line::from(format!("workspace manifest: {workspace_label}")),
        Line::from(format!("linked machines: {machine_count}")),
        Line::from(format!("composition machines: {composition_count}")),
        Line::from(format!("cross-machine exact relations: {relation_count}")),
        Line::from("observed surface: linked compiled CodebaseDoc rendered as Mermaid flowchart"),
    ])
}

fn state_source_text(machine: &CodebaseMachine, state: &CodebaseState) -> Text<'static> {
    Text::from(vec![
        Line::from(format!("machine path: {}", machine.rust_type_path)),
        Line::from(format!("state rust name: {}", state.rust_name)),
        Line::from("definition location: not available in current linked exact surface"),
    ])
}

fn transition_source_text(
    machine: &CodebaseMachine,
    transition: &CodebaseTransition,
) -> Text<'static> {
    Text::from(vec![
        Line::from(format!("machine path: {}", machine.rust_type_path)),
        Line::from(format!("transition method: {}", transition.method_name)),
        Line::from(format!("from state index: {}", transition.from)),
        Line::from("definition location: not available in current linked exact surface"),
    ])
}

fn validator_source_text(entry: &CodebaseValidatorEntry) -> Text<'static> {
    Text::from(vec![
        Line::from(format!("source module: {}", entry.source_module_path)),
        Line::from(format!("source type: {}", entry.source_type_display)),
        Line::from("definition location: not available in current linked exact surface"),
    ])
}

fn relation_source_text(selection: RelationDetailSelection<'_>) -> Text<'static> {
    match selection {
        RelationDetailSelection::Exact(detail) => {
            let mut lines = vec![
                Line::from("exact relation"),
                Line::from(format!("kind: {}", detail.relation.kind.display_label())),
                Line::from(format!("basis: {}", detail.relation.basis.display_label())),
                Line::from(format!(
                    "semantic: {}",
                    detail.relation.semantic.display_label()
                )),
                Line::from(format!(
                    "source machine: {}",
                    render_machine_label(detail.source_machine)
                )),
                Line::from(format!(
                    "source kind: {}",
                    exact_relation_source_label(detail.relation.source)
                )),
                Line::from(format!(
                    "target machine: {}",
                    render_machine_label(detail.target_machine)
                )),
                Line::from(format!(
                    "target state: {}",
                    render_state_label(detail.target_state)
                )),
                Line::from("source location: not available in current linked exact surface"),
            ];
            if let Some(state) = detail.source_state {
                lines.push(Line::from(format!(
                    "source state: {}",
                    render_state_label(state)
                )));
            }
            if let Some(transition) = detail.source_transition {
                lines.push(Line::from(format!(
                    "source transition: {}",
                    render_transition_label(transition)
                )));
            }
            if let Some(attested_via) = detail.relation.attested_via.as_ref() {
                lines.push(Line::from(format!(
                    "attested route: {}::{}",
                    attested_via.via_module_path, attested_via.route_name
                )));
            }
            Text::from(lines)
        }
        RelationDetailSelection::Heuristic {
            detail,
            shadowed_by_exact,
        } => {
            let mut lines = vec![
                Line::from("heuristic relation"),
                Line::from(format!(
                    "source item: {}",
                    render_heuristic_source_label(&detail)
                )),
                Line::from(format!(
                    "target machine: {}",
                    render_machine_label(detail.target_machine)
                )),
                Line::from(format!(
                    "location: {}:{}",
                    detail.relation.file_path.display(),
                    detail.relation.line_number
                )),
                Line::from(format!(
                    "matched path: {}",
                    detail.relation.matched_path_text
                )),
            ];
            if shadowed_by_exact {
                lines.push(Line::from(
                    "exact lane already covers this relationship and will hide it in mixed mode.",
                ));
            }
            if let Some(snippet) = detail.relation.snippet.as_deref() {
                lines.push(Line::from(format!("snippet: {snippet}")));
            }
            Text::from(lines)
        }
    }
}

fn path_source_text(path: &PathItem, doc: &CodebaseDoc) -> Text<'static> {
    let mut lines = vec![
        Line::from(format!("path kind: {}", path.kind.display_label())),
        Line::from(
            "exact path step locations are not available in the current linked exact surface",
        ),
    ];
    for (index, step) in path.steps.iter().enumerate() {
        let from = doc
            .machine(step.from_machine)
            .map(render_machine_label)
            .unwrap_or("<missing machine>");
        let to = doc
            .machine(step.to_machine)
            .map(render_machine_label)
            .unwrap_or("<missing machine>");
        lines.push(Line::from(format!(
            "{}. [{}] {} -> {} : {}",
            index + 1,
            step.kind.display_label(),
            from,
            to,
            step.label
        )));
    }
    Text::from(lines)
}

fn gap_source_text(gap: &CompositionSuggestion, doc: &CodebaseDoc) -> Text<'static> {
    Text::from(vec![
        Line::from(format!("severity: {}", gap.severity.display_label())),
        Line::from(format!("kind: {}", gap.kind.display_label())),
        Line::from(format!(
            "source machine: {}",
            gap.source_machine(doc)
                .map(render_machine_label)
                .unwrap_or("<missing machine>")
        )),
        Line::from(format!(
            "target machine: {}",
            gap.target_machine(doc)
                .map(render_machine_label)
                .unwrap_or("<missing machine>")
        )),
        Line::from(format!("evidence: {}", gap.counts_label())),
    ])
}

fn machine_explain_text(machine: &CodebaseMachine, app: &InspectorApp) -> Text<'static> {
    let (exact_summary_count, heuristic_summary_count) =
        app.machine_visible_summary_counts(machine.index);
    Text::from(format!(
        "{} is a {} machine with {} states, {} transitions, {} validator entries, {} exact summary edges, and {} heuristic summary edges.",
        render_machine_label(machine),
        machine.role.display_label(),
        machine.states.len(),
        machine.transitions.len(),
        machine.validator_entries.len(),
        exact_summary_count,
        heuristic_summary_count
    ))
}

fn workspace_explain_text(doc: &CodebaseDoc) -> Text<'static> {
    let machine_count = doc.machines().len();
    let composition_count = doc
        .machines()
        .iter()
        .filter(|machine| machine.role.is_composition())
        .count();
    Text::from(format!(
        "The workspace diagram is the exact linked flow surface across {machine_count} machine(s), including {composition_count} composition machine(s). Use it to understand the top-level handoff graph before drilling into a machine state diagram or an exact relation sequence diagram."
    ))
}

fn state_explain_text(machine: &CodebaseMachine, state: &CodebaseState) -> Text<'static> {
    Text::from(format!(
        "{}::{} has_data={}, direct_construction={}, graph_root={}.",
        render_machine_label(machine),
        state.rust_name,
        yes_no(state.has_data),
        yes_no(state.direct_construction_available),
        yes_no(state.is_graph_root)
    ))
}

fn transition_explain_text(
    machine: &CodebaseMachine,
    transition: &CodebaseTransition,
) -> Text<'static> {
    Text::from(format!(
        "{}::{} starts at state index {} and allows {} legal target state(s).",
        render_machine_label(machine),
        transition.method_name,
        transition.from,
        transition.to.len()
    ))
}

fn validator_explain_text(entry: &CodebaseValidatorEntry) -> Text<'static> {
    Text::from(format!(
        "{} rebuilds {} target state(s) from {}.",
        entry.display_label(),
        entry.target_states.len(),
        entry.source_type_display
    ))
}

fn relation_explain_text(selection: RelationDetailSelection<'_>) -> Text<'static> {
    match selection {
        RelationDetailSelection::Exact(detail) => Text::from(format!(
            "{} points to {}::{} through an exact {} relation with {} basis.",
            render_machine_label(detail.source_machine),
            render_machine_label(detail.target_machine),
            detail.target_state.rust_name,
            detail.relation.kind.display_label(),
            detail.relation.basis.display_label()
        )),
        RelationDetailSelection::Heuristic { detail, .. } => Text::from(format!(
            "{} suggests a weaker source-scanned coupling to {} based on {} evidence.",
            render_heuristic_source_label(&detail),
            render_machine_label(detail.target_machine),
            detail.relation.evidence_kind.display_label()
        )),
    }
}

fn path_explain_text(path: &PathItem, doc: &CodebaseDoc) -> Text<'static> {
    let target = doc
        .machine(path.target_machine)
        .map(render_machine_label)
        .unwrap_or("<missing machine>");
    Text::from(format!(
        "This {} path reaches {} in {} hop(s).",
        path.kind.display_label(),
        target,
        path.steps.len()
    ))
}

fn gap_explain_text(gap: &CompositionSuggestion, doc: &CodebaseDoc) -> Text<'static> {
    Text::from(format!(
        "{} {} because {}. {}",
        gap.severity.display_label(),
        gap.summary_label(doc),
        gap.why_text(),
        gap.help_text()
    ))
}

fn exact_relation_source_label(source: CodebaseRelationSource) -> String {
    match source {
        CodebaseRelationSource::StatePayload { field_name, .. } => format!(
            "state payload{}",
            field_name
                .map(|field_name| format!(" field `{field_name}`"))
                .unwrap_or_default()
        ),
        CodebaseRelationSource::MachineField { field_name, .. } => {
            format!("machine field {}", field_name.unwrap_or("<unnamed>"))
        }
        CodebaseRelationSource::TransitionParam {
            param_name,
            param_index,
            ..
        } => format!(
            "transition param {} ({})",
            param_name.unwrap_or("<unnamed>"),
            param_index
        ),
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vertical[1])[1]
}

#[cfg(test)]
mod tests {
    use super::*;

    use crossterm::event::KeyModifiers;
    use ratatui::backend::TestBackend;
    use std::fs;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[allow(dead_code)]
    mod task {
        use statum::{machine, state, transition};

        #[state]
        pub enum State {
            Idle,
            /// Task work is currently executing.
            Running,
            Done,
        }

        /// Handles the task lifecycle.
        #[machine]
        #[present(label = "Task Machine", description = "Owns the exact task lifecycle.")]
        pub struct Machine<State> {}

        #[transition]
        impl Machine<Idle> {
            /// Starts task execution.
            #[present(
                label = "Start Task",
                description = "Moves an idle task into running work."
            )]
            fn start(self) -> Machine<Running> {
                self.transition()
            }
        }

        #[transition]
        impl Machine<Running> {
            fn finish(self) -> Machine<Done> {
                self.transition()
            }
        }
    }

    #[allow(dead_code)]
    mod workflow {
        use statum::{machine, state, transition, validators, Error};

        #[state]
        pub enum State {
            Draft,
            /// Workflow execution is delegated to a running task.
            #[present(
                label = "In Progress",
                description = "Work is currently delegated to a running task."
            )]
            InProgress(super::task::Machine<super::task::Running>),
            Done,
        }

        /// Coordinates multi-step workflow progress.
        #[machine(role = composition)]
        #[present(
            label = "Workflow Machine",
            description = "Tracks workflow progress across task execution."
        )]
        pub struct Machine<State> {}

        #[transition]
        impl Machine<Draft> {
            /// Starts the workflow and hands control to the task machine.
            #[present(
                label = "Start Workflow",
                description = "Begins workflow execution with a running task."
            )]
            fn start(
                self,
                task: super::task::Machine<super::task::Running>,
            ) -> Machine<InProgress> {
                self.transition_with(task)
            }
        }

        #[transition]
        impl Machine<InProgress> {
            fn finish(self) -> Machine<Done> {
                self.transition()
            }
        }

        pub struct WorkflowRow {
            pub status: &'static str,
        }

        /// Rebuilds workflow machines from persisted workflow rows.
        #[validators(Machine)]
        impl WorkflowRow {
            fn is_draft(&self) -> statum::Result<()> {
                if self.status == "draft" {
                    Ok(())
                } else {
                    Err(Error::InvalidState)
                }
            }

            fn is_in_progress(&self) -> statum::Result<super::task::Machine<super::task::Running>> {
                if self.status == "running" {
                    Ok(super::task::Machine::<super::task::Running>::builder().build())
                } else {
                    Err(Error::InvalidState)
                }
            }

            fn is_done(&self) -> statum::Result<()> {
                if self.status == "done" {
                    Ok(())
                } else {
                    Err(Error::InvalidState)
                }
            }
        }
    }

    fn fixture_doc() -> CodebaseDoc {
        CodebaseDoc::linked().expect("linked codebase doc")
    }

    fn unique_temp_dir(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock before unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "statum-inspect-{label}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("create temp dir");
        path
    }

    fn empty_heuristic_overlay() -> HeuristicOverlay {
        HeuristicOverlay::from_parts(HeuristicStatusKind::Available, Vec::new(), Vec::new())
    }

    fn fixture_app(doc: CodebaseDoc, heuristic: HeuristicOverlay) -> InspectorApp {
        InspectorApp::new(
            doc,
            heuristic,
            CompositionSuggestionOverlay::default(),
            "/tmp/workspace/Cargo.toml".to_owned(),
        )
    }

    fn fixture_heuristic_overlay(doc: &CodebaseDoc) -> HeuristicOverlay {
        let workflow = doc
            .machines()
            .iter()
            .find(|machine| machine.label == Some("Workflow Machine"))
            .expect("workflow machine");
        let task = doc
            .machines()
            .iter()
            .find(|machine| machine.label == Some("Task Machine"))
            .expect("task machine");
        let transition = workflow
            .transitions
            .iter()
            .find(|transition| transition.method_name == "start")
            .expect("workflow start transition");

        HeuristicOverlay::from_parts(
            HeuristicStatusKind::Available,
            Vec::new(),
            vec![
                HeuristicRelation {
                    index: 0,
                    source: HeuristicRelationSource::Transition {
                        machine: workflow.index,
                        transition: transition.index,
                    },
                    target_machine: task.index,
                    evidence_kind: HeuristicEvidenceKind::Signature,
                    matched_path_text: "task::Machine < task::Running >".to_owned(),
                    file_path: PathBuf::from("/tmp/workspace/src/workflow.rs"),
                    line_number: 10,
                    snippet: Some(
                        "fn start(self, task: task::Machine<task::Running>) -> Machine<InProgress> {"
                            .to_owned(),
                    ),
                },
                HeuristicRelation {
                    index: 1,
                    source: HeuristicRelationSource::Transition {
                        machine: workflow.index,
                        transition: transition.index,
                    },
                    target_machine: task.index,
                    evidence_kind: HeuristicEvidenceKind::Body,
                    matched_path_text: "task::Receipt".to_owned(),
                    file_path: PathBuf::from("/tmp/workspace/src/workflow.rs"),
                    line_number: 11,
                    snippet: Some("let _receipt = task::Receipt;".to_owned()),
                },
            ],
        )
    }

    fn text_contents(text: Text<'_>) -> String {
        text_plain_string(&text)
    }

    #[test]
    fn app_renders_workspace_diagram_home_and_paths() {
        let doc = fixture_doc();
        let mut app = fixture_app(doc, empty_heuristic_overlay());
        let workflow_index = app
            .doc
            .machines()
            .iter()
            .position(|machine| machine.label == Some("Workflow Machine"))
            .expect("workflow machine should exist");
        app.select_machine(workflow_index);
        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).expect("terminal");

        terminal
            .draw(|frame| app.render(frame))
            .expect("render should succeed");

        let rendered = terminal.backend().buffer().content.clone();
        let text = rendered
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();

        let plan = app.center_diagram_plan();
        assert_eq!(plan.kind_label, "flowchart");
        let source = text_contents(plan.source);
        assert!(source.contains("graph TD"));
        assert!(text.contains("flowchart"));
        assert!(text.contains("Workspace /tmp/workspace/Cargo.toml"));

        app.machine_section = MachineSection::Paths;
        app.clamp_indices();
        terminal
            .draw(|frame| app.render(frame))
            .expect("render should succeed");

        let rendered = terminal.backend().buffer().content.clone();
        let text = rendered
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert!(text.contains("Task Machine"));
        assert!(text.contains("[1 hop]"));
    }

    #[test]
    fn workspace_home_detail_tabs_stay_workspace_scoped() {
        let doc = fixture_doc();
        let mut app = fixture_app(doc, empty_heuristic_overlay());
        let workflow_index = app
            .doc
            .machines()
            .iter()
            .position(|machine| machine.label == Some("Workflow Machine"))
            .expect("workflow machine should exist");
        app.select_machine(workflow_index);

        assert_eq!(
            app.current_selection_label(),
            "Workspace /tmp/workspace/Cargo.toml"
        );
        assert!(text_contents(app.current_summary_text())
            .contains("Workspace view is the top-level exact flow surface."));
        assert!(text_contents(app.current_docs_text())
            .contains("Workspace diagrams do not have source-local docs of their own."));
        assert!(text_contents(app.current_source_text())
            .contains("workspace manifest: /tmp/workspace/Cargo.toml"));
        assert!(text_contents(app.current_explain_text())
            .contains("The workspace diagram is the exact linked flow surface"));
    }

    #[test]
    fn app_renders_relation_and_path_provenance_cards() {
        let doc = fixture_doc();
        let overlay = fixture_heuristic_overlay(&doc);
        let mut app = fixture_app(doc, overlay);
        let workflow_index = app
            .doc
            .machines()
            .iter()
            .position(|machine| machine.label == Some("Workflow Machine"))
            .expect("workflow machine should exist");
        app.workspace_section = WorkspaceSection::Machines;
        app.select_machine(workflow_index);
        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).expect("terminal");

        app.machine_section = MachineSection::Relations;
        app.clamp_indices();
        terminal
            .draw(|frame| app.render(frame))
            .expect("render should succeed");

        let rendered = terminal.backend().buffer().content.clone();
        let text = rendered
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert!(text.contains("transition Start Workflow"));
        assert!(text.contains("transition param task (0)"));
        assert!(text.contains("direct type"));

        app.workspace_section = WorkspaceSection::Composition;
        app.machine_section = MachineSection::Paths;
        app.clamp_indices();
        terminal
            .draw(|frame| app.render(frame))
            .expect("render should succeed");

        let rendered = terminal.backend().buffer().content.clone();
        let text = rendered
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert!(text.contains("Workflow Machine -> Task Machine"));

        app.workspace_section = WorkspaceSection::Machines;
        app.machine_section = MachineSection::Relations;
        app.set_lane_mode(LaneMode::Heuristic);
        app.clamp_indices();
        terminal
            .draw(|frame| app.render(frame))
            .expect("render should succeed");

        let rendered = terminal.backend().buffer().content.clone();
        let text = rendered
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert!(text.contains("workflow.rs:10"));
    }

    #[test]
    fn app_renders_inline_search_reasons_and_detail_cards() {
        let doc = fixture_doc();
        let mut app = fixture_app(doc, empty_heuristic_overlay());
        let workflow_index = app
            .doc
            .machines()
            .iter()
            .position(|machine| machine.label == Some("Workflow Machine"))
            .expect("workflow machine should exist");
        app.workspace_section = WorkspaceSection::Machines;
        app.select_machine(workflow_index);
        app.search_scope = SearchScope::Docs;
        app.search_query = "persisted".to_owned();
        app.clamp_indices();

        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).expect("terminal");
        terminal
            .draw(|frame| app.render(frame))
            .expect("render should succeed");

        let rendered = terminal.backend().buffer().content.clone();
        let text = rendered
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert!(text.contains("match"));
        assert!(text.contains("docs:"));
        assert!(text.contains("SNAPSHOT"));
        assert!(text.contains("NOTES"));
    }

    #[test]
    fn relation_rich_machines_default_to_overview() {
        let doc = fixture_doc();
        let mut app = fixture_app(doc, empty_heuristic_overlay());
        let workflow_index = app
            .doc
            .machines()
            .iter()
            .position(|machine| machine.label == Some("Workflow Machine"))
            .expect("workflow machine should exist");

        app.select_machine(workflow_index);

        assert_eq!(app.machine_section, MachineSection::Overview);
        assert_eq!(app.summary_items().len(), 1);
    }

    #[test]
    fn app_navigation_reaches_relations_and_details() {
        let doc = fixture_doc();
        let mut app = fixture_app(doc, empty_heuristic_overlay());
        let workflow_index = app
            .doc
            .machines()
            .iter()
            .position(|machine| machine.label == Some("Workflow Machine"))
            .expect("workflow machine should exist");
        app.select_machine(workflow_index);
        app.machine_section = MachineSection::States;
        app.machine_item_index = app
            .machine_items()
            .iter()
            .position(|item| matches!(item, MachineItem::State(1)))
            .expect("in-progress state should exist");
        app.focus = Focus::Detail;
        app.clamp_indices();

        assert_eq!(app.focus, Focus::Detail);
        assert_eq!(app.relation_direction, RelationDirection::Outbound);
        assert_eq!(
            app.relation_subject(),
            Some(RelationSubject::State {
                machine: workflow_index,
                state: 1
            })
        );

        let detail = app
            .selected_relation_detail()
            .expect("selected relation detail should exist");
        let RelationDetailSelection::Exact(detail) = detail else {
            panic!("expected exact relation detail");
        };
        assert_eq!(detail.source_machine.label, Some("Workflow Machine"));
        assert_eq!(detail.target_machine.label, Some("Task Machine"));
    }

    #[test]
    fn app_search_filters_visible_machines_and_machine_items() {
        let doc = fixture_doc();
        let mut app = fixture_app(doc, empty_heuristic_overlay());

        app.handle_key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE));
        assert_eq!(app.input_mode, InputMode::Search);
        for ch in "workflow machine".chars() {
            app.handle_key(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE));
        }
        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        assert_eq!(app.input_mode, InputMode::Normal);
        assert_eq!(app.search_query, "workflow machine");
        assert_eq!(app.visible_machine_indices().len(), 1);
        assert_eq!(
            app.current_machine().map(render_machine_label),
            Some("Workflow Machine")
        );

        app.machine_section = MachineSection::Transitions;
        app.search_query = "start workflow".to_owned();
        app.clamp_indices();

        let machine = app.current_machine().expect("workflow machine");
        let labels = app
            .machine_items()
            .iter()
            .map(|item| app.machine_item_label(machine, item))
            .collect::<Vec<_>>();
        assert_eq!(labels, vec!["Start Workflow"]);
    }

    #[test]
    fn app_search_scope_can_narrow_to_docs() {
        let doc = fixture_doc();
        let mut app = fixture_app(doc, empty_heuristic_overlay());
        app.search_query = "persisted".to_owned();
        app.clamp_indices();

        assert!(app.current_machine().is_none());

        app.handle_key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE));
        assert_eq!(app.search_scope, SearchScope::Docs);
        assert_eq!(
            app.current_machine().map(render_machine_label),
            Some("Workflow Machine")
        );

        app.handle_key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE));
        assert_eq!(app.search_scope, SearchScope::Relations);
        assert!(app.current_machine().is_none());
    }

    #[test]
    fn app_search_scope_can_target_paths() {
        let doc = fixture_doc();
        let mut app = fixture_app(doc, empty_heuristic_overlay());
        app.search_query = "1 hop".to_owned();
        app.clamp_indices();

        assert!(app.current_machine().is_none());

        app.search_scope = SearchScope::Paths;
        app.clamp_indices();
        assert_eq!(
            app.current_machine().map(render_machine_label),
            Some("Workflow Machine")
        );

        app.machine_section = MachineSection::Paths;
        assert_eq!(app.path_items().len(), 1);
        assert_eq!(app.path_items()[0].kind, PathKind::Composition);
    }

    #[test]
    fn app_handles_search_with_no_matches() {
        let doc = fixture_doc();
        let mut app = fixture_app(doc, empty_heuristic_overlay());
        app.search_query = "missing-machine".to_owned();
        app.clamp_indices();

        assert!(app.current_machine().is_none());
        assert!(app.machine_items().is_empty());
        assert!(app.relation_items().is_empty());

        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).expect("terminal");
        terminal
            .draw(|frame| app.render(frame))
            .expect("render should succeed");

        let rendered = terminal.backend().buffer().content.clone();
        let text = rendered
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert!(text.contains("<no matches>"));
    }

    #[test]
    fn app_search_matches_relation_targets_and_keeps_source_machine_visible_in_relation_scope() {
        let doc = fixture_doc();
        let mut app = fixture_app(doc, empty_heuristic_overlay());
        app.search_query = "param".to_owned();
        app.search_scope = SearchScope::Relations;
        app.clamp_indices();

        let labels = app
            .visible_machine_indices()
            .iter()
            .copied()
            .filter_map(|machine_index| app.doc.machine(machine_index))
            .map(render_machine_label)
            .collect::<Vec<_>>();
        assert_eq!(labels, vec!["Task Machine", "Workflow Machine"]);
    }

    #[test]
    fn app_reuses_cached_visible_and_relation_views_until_invalidated() {
        let doc = fixture_doc();
        let mut app = fixture_app(doc, empty_heuristic_overlay());
        app.select_machine(0);
        app.machine_section = MachineSection::Overview;
        app.clamp_indices();

        let visible_first = app.visible_machine_indices();
        let visible_second = app.visible_machine_indices();
        assert!(Rc::ptr_eq(&visible_first, &visible_second));

        let summary_first = app.summary_items();
        let summary_second = app.summary_items();
        assert!(Rc::ptr_eq(&summary_first, &summary_second));

        app.handle_key(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::NONE));

        let summary_after_filter = app.summary_items();
        assert!(!Rc::ptr_eq(&summary_first, &summary_after_filter));
    }

    #[test]
    fn app_relation_filters_trim_summary_and_relation_lists() {
        let doc = fixture_doc();
        let mut app = fixture_app(doc, empty_heuristic_overlay());
        let workflow_index = app
            .doc
            .machines()
            .iter()
            .position(|machine| machine.label == Some("Workflow Machine"))
            .expect("workflow machine should exist");
        app.select_machine(workflow_index);
        app.machine_section = MachineSection::Overview;
        app.clamp_indices();

        assert_eq!(app.summary_items().len(), 1);
        match &app.summary_items()[0] {
            SummaryItem::Exact(item) => {
                assert_eq!(
                    item.group.display_label(),
                    "composition refs: payload, param"
                );
            }
            SummaryItem::Heuristic(_) => panic!("expected exact summary item"),
        }

        app.handle_key(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::NONE));
        assert_eq!(app.summary_items().len(), 1);
        match &app.summary_items()[0] {
            SummaryItem::Exact(item) => {
                assert_eq!(item.group.display_label(), "composition refs: param");
            }
            SummaryItem::Heuristic(_) => panic!("expected exact summary item"),
        }

        app.machine_section = MachineSection::States;
        app.invalidate_cache();
        app.machine_item_index = app
            .machine_items()
            .iter()
            .position(|item| matches!(item, MachineItem::State(1)))
            .expect("in-progress state should exist");
        assert!(app.relation_items().is_empty());

        app.machine_section = MachineSection::Transitions;
        app.invalidate_cache();
        app.machine_item_index = app
            .machine_items()
            .iter()
            .position(|item| matches!(item, MachineItem::Transition(0)))
            .expect("start transition should exist");
        let relation = app
            .relation_items()
            .first()
            .copied()
            .expect("transition-param relation should remain");
        match relation {
            RelationItem::Exact(index) => {
                let relation = app
                    .doc
                    .relation(index)
                    .expect("exact relation should exist");
                assert_eq!(relation.kind, CodebaseRelationKind::TransitionParam);
            }
            RelationItem::Heuristic(_) => panic!("expected exact relation"),
        }

        app.handle_key(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE));
        assert!(app.summary_items().is_empty());
        assert!(app.relation_items().is_empty());
    }

    #[test]
    fn escape_clears_focus_then_search_and_q_quits() {
        let doc = fixture_doc();
        let mut app = fixture_app(doc, empty_heuristic_overlay());
        app.focus = Focus::Detail;
        app.search_query = "workflow".to_owned();

        app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert_eq!(app.focus, Focus::Workspace);
        assert_eq!(app.search_query, "workflow");
        assert!(!app.should_quit);

        app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert_eq!(app.search_query, "");
        assert!(!app.should_quit);

        app.handle_key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE));
        assert!(app.should_quit);
    }

    #[test]
    fn help_overlay_toggles_and_absorbs_other_keys() {
        let doc = fixture_doc();
        let mut app = fixture_app(doc, empty_heuristic_overlay());
        let selected_before = app.selected_machine;

        app.handle_key(KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE));
        assert!(app.show_help);

        app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        assert!(app.show_help);
        assert_eq!(app.selected_machine, selected_before);

        app.handle_key(KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE));
        assert!(!app.show_help);
    }

    #[test]
    fn detail_pane_shows_descriptions_and_docs() {
        let doc = fixture_doc();
        let workflow = doc
            .machines()
            .iter()
            .find(|machine| machine.label == Some("Workflow Machine"))
            .expect("workflow machine");
        let in_progress = workflow
            .state_named("InProgress")
            .expect("in-progress state");
        let start = workflow
            .transitions
            .iter()
            .find(|transition| transition.method_name == "start")
            .expect("workflow start transition");
        let validator = workflow
            .validator_entries
            .first()
            .expect("workflow validator entry");
        let relation = doc
            .outbound_relations_for_machine(workflow.index)
            .next()
            .expect("workflow relation");
        let relation_detail = doc
            .relation_detail(relation.index)
            .expect("workflow relation detail");
        let summary = doc
            .machine_relation_groups()
            .iter()
            .find(|group| group.from_machine == workflow.index)
            .expect("workflow summary");
        let summary_item = SummaryItem::Exact(ExactSummaryItem {
            direction: SummaryDirection::Outbound,
            group: summary.clone(),
        });

        let machine_detail = text_contents(machine_detail_text(workflow, &doc, &[]));
        assert!(machine_detail.contains("role: composition"));
        assert!(machine_detail.contains("Description"));
        assert!(machine_detail.contains("Tracks workflow progress across task execution."));
        assert!(machine_detail.contains("Docs"));
        assert!(machine_detail.contains("Coordinates multi-step workflow progress."));

        let state_detail = text_contents(state_detail_text(in_progress));
        assert!(state_detail.contains("Description"));
        assert!(state_detail.contains("Work is currently delegated to a running task."));
        assert!(state_detail.contains("Docs"));
        assert!(state_detail.contains("Workflow execution is delegated to a running task."));

        let transition_detail = text_contents(transition_detail_text(start));
        assert!(transition_detail.contains("Description"));
        assert!(transition_detail.contains("Begins workflow execution with a running task."));
        assert!(transition_detail.contains("Docs"));
        assert!(transition_detail
            .contains("Starts the workflow and hands control to the task machine."));

        let validator_detail = text_contents(validator_detail_text(validator));
        assert!(validator_detail.contains("Docs"));
        assert!(
            validator_detail.contains("Rebuilds workflow machines from persisted workflow rows.")
        );

        let relation_text = text_contents(relation_detail_text(relation_detail));
        assert!(relation_text.contains("semantic: composition direct child"));
        assert!(relation_text.contains("source machine role: composition"));
        assert!(relation_text.contains("source machine Description"));
        assert!(relation_text.contains("source machine Docs"));
        assert!(relation_text.contains("target machine Description"));
        assert!(relation_text.contains("target machine Docs"));

        let summary_text = text_contents(summary_detail_text(&summary_item, &doc));
        assert!(summary_text.contains("semantic: composition-owned"));
        assert!(summary_text.contains("source machine Description"));
        assert!(summary_text.contains("source machine Docs"));
        assert!(summary_text.contains("target machine Description"));
        assert!(summary_text.contains("target machine Docs"));
    }

    #[test]
    fn diagram_tab_shows_machine_state_mermaid() {
        let doc = fixture_doc();
        let mut app = fixture_app(doc, empty_heuristic_overlay());
        app.workspace_section = WorkspaceSection::Machines;
        app.invalidate_cache();

        let diagram = text_contents(app.current_diagram_text());
        assert!(diagram.contains("stateDiagram-v2"));
        assert!(diagram.contains("Start Workflow"));
        assert!(diagram.contains("[*] -->"));
    }

    #[test]
    fn diagram_tab_shows_workspace_flow_mermaid_on_workspace_home() {
        let doc = fixture_doc();
        let app = fixture_app(doc, empty_heuristic_overlay());

        let diagram = text_contents(app.current_diagram_text());
        assert!(diagram.contains("graph TD"));
        assert!(diagram.contains("Workflow Machine"));
        assert!(diagram.contains("Task Machine"));
    }

    #[test]
    fn diagram_tab_shows_exact_relation_sequence_mermaid() {
        let doc = fixture_doc();
        let mut app = fixture_app(doc, empty_heuristic_overlay());
        app.machine_section = MachineSection::Relations;
        app.invalidate_cache();

        let diagram = text_contents(app.current_diagram_text());
        assert!(diagram.contains("sequenceDiagram"));
        assert!(diagram.contains("Task Machine"));
        assert!(diagram.contains("Workflow Machine"));
    }

    #[test]
    fn diagram_tab_marks_path_export_as_unimplemented() {
        let doc = fixture_doc();
        let mut app = fixture_app(doc, empty_heuristic_overlay());
        app.machine_section = MachineSection::Paths;
        app.invalidate_cache();

        let diagram = text_contents(app.current_diagram_text());
        assert!(diagram.contains("Exact composition-path Mermaid export is not implemented yet."));
    }

    #[test]
    fn mermaid_diagram_source_detects_supported_headers_after_comments() {
        assert_eq!(
            mermaid_diagram_source("%% comment\nstateDiagram-v2\nstate \"Draft\" as s0"),
            Some("stateDiagram-v2")
        );
        assert_eq!(
            mermaid_diagram_source("sequenceDiagram\nparticipant a as A"),
            Some("sequenceDiagram")
        );
        assert_eq!(mermaid_diagram_source("not a diagram"), None);
    }

    #[test]
    fn termaid_preview_with_missing_candidate_fails_closed() {
        let error = render_termaid_preview_with_candidates(
            "stateDiagram-v2\nstate \"Draft\" as s0",
            80,
            &[OsString::from("/definitely/missing/termaid")],
        )
        .expect_err("missing binary should fail closed");

        assert!(error.contains("termaid binary not found"));
    }

    #[test]
    fn termaid_preview_runs_candidate_and_returns_stdout() {
        let temp = unique_temp_dir("termaid-preview");
        let script = temp.join("fake-termaid");
        fs::write(&script, "#!/bin/sh\ncat\n").expect("write fake termaid");
        #[cfg(unix)]
        {
            let mut permissions = fs::metadata(&script)
                .expect("script metadata")
                .permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(&script, permissions).expect("make fake termaid executable");
        }

        let rendered = render_termaid_preview_with_candidates(
            "sequenceDiagram\nparticipant a as A",
            72,
            &[script.into_os_string()],
        )
        .expect("candidate should echo diagram");

        assert!(rendered.contains("sequenceDiagram"));
        assert!(rendered.contains("participant a as A"));

        fs::remove_dir_all(temp).expect("remove temp dir");
    }

    #[test]
    fn sibling_termaid_binary_discovers_adjacent_workspace_renderer() {
        let temp = unique_temp_dir("termaid-sibling");
        let workspace = temp.join("citacell");
        let renderer = temp
            .join("termaid")
            .join("target")
            .join("release")
            .join("termaid");
        fs::create_dir_all(&workspace).expect("create workspace dir");
        fs::create_dir_all(renderer.parent().expect("renderer parent"))
            .expect("create renderer dir");
        fs::write(&renderer, "").expect("touch renderer binary");

        let discovered = sibling_termaid_binary(workspace.to_str().expect("workspace path utf-8"))
            .expect("discover sibling renderer");

        assert_eq!(discovered, renderer);

        fs::remove_dir_all(temp).expect("remove temp dir");
    }

    #[test]
    fn machine_workspace_detail_shows_composition_diagnostics() {
        let doc = fixture_doc();
        let workflow = doc
            .machines()
            .iter()
            .find(|machine| machine.label == Some("Workflow Machine"))
            .expect("workflow machine");
        let workflow_index = workflow.index;
        let task = doc
            .machines()
            .iter()
            .find(|machine| machine.label == Some("Task Machine"))
            .expect("task machine");
        let task_index = task.index;
        let app = InspectorApp::new(
            doc,
            empty_heuristic_overlay(),
            CompositionSuggestionOverlay::from_suggestions(vec![CompositionSuggestion {
                index: 0,
                severity: CompositionSuggestionSeverity::Warning,
                kind: crate::suggestions::CompositionSuggestionKind::MissingCompositionRole,
                source_machine: workflow_index,
                target_machine: task_index,
                exact_relation_indices: vec![0],
                heuristic_relation_indices: Vec::new(),
                exact_counts: vec![CodebaseRelationCount {
                    kind: CodebaseRelationKind::TransitionParam,
                    basis: CodebaseRelationBasis::DirectTypeSyntax,
                    count: 1,
                }],
                heuristic_counts: Vec::new(),
            }]),
            "/tmp/workspace/Cargo.toml".to_owned(),
        );
        let workflow = app
            .doc
            .machines()
            .iter()
            .find(|machine| machine.label == Some("Workflow Machine"))
            .expect("workflow machine");

        let detail = text_contents(app.machine_workspace_detail_text(workflow));
        assert!(detail.contains("Composition Diagnostics"));
        assert!(detail.contains("warning: Workflow Machine -> Task Machine"));
        assert!(detail.contains("consider `#[machine(role = composition)]`"));
    }

    #[test]
    fn composition_view_shows_preferred_paths() {
        let doc = fixture_doc();
        let mut app = fixture_app(doc, empty_heuristic_overlay());
        let workflow_index = app
            .doc
            .machines()
            .iter()
            .position(|machine| machine.label == Some("Workflow Machine"))
            .expect("workflow machine");
        app.workspace_section = WorkspaceSection::Composition;
        app.select_machine(workflow_index);

        let path = app.selected_path_item().expect("composition path");
        assert_eq!(path.kind, PathKind::Composition);
        assert_eq!(path.steps.len(), 1);
        let detail = text_contents(app.path_detail_text(&path));
        assert!(detail.contains("path kind: composition"));
        assert!(detail.contains("[composition] Workflow Machine -> Task Machine"));
    }

    #[test]
    fn gaps_view_shows_gap_card_and_best_path() {
        let doc = fixture_doc();
        let workflow_index = doc
            .machines()
            .iter()
            .position(|machine| machine.label == Some("Workflow Machine"))
            .expect("workflow machine");
        let task_index = doc
            .machines()
            .iter()
            .position(|machine| machine.label == Some("Task Machine"))
            .expect("task machine");
        let mut app = InspectorApp::new(
            doc,
            empty_heuristic_overlay(),
            CompositionSuggestionOverlay::from_suggestions(vec![CompositionSuggestion {
                index: 0,
                severity: CompositionSuggestionSeverity::Warning,
                kind: crate::suggestions::CompositionSuggestionKind::MissingCompositionRole,
                source_machine: workflow_index,
                target_machine: task_index,
                exact_relation_indices: vec![0],
                heuristic_relation_indices: Vec::new(),
                exact_counts: vec![CodebaseRelationCount {
                    kind: CodebaseRelationKind::TransitionParam,
                    basis: CodebaseRelationBasis::DirectTypeSyntax,
                    count: 1,
                }],
                heuristic_counts: Vec::new(),
            }]),
            "/tmp/workspace/Cargo.toml".to_owned(),
        );
        app.workspace_section = WorkspaceSection::Gaps;
        app.clamp_indices();

        let gap_text = text_contents(app.gap_card_text());
        assert!(gap_text.contains("severity: warning"));
        assert!(gap_text.contains("source machine: Workflow Machine"));
        assert!(gap_text.contains("target machine: Task Machine"));

        let path = app.selected_path_item().expect("gap path");
        assert_eq!(path.kind, PathKind::Composition);
        let path_text = text_contents(app.path_detail_text(&path));
        assert!(path_text.contains("to: Task Machine"));
    }

    #[test]
    fn gaps_paths_ignore_gap_search_text_once_a_gap_is_selected() {
        let doc = fixture_doc();
        let workflow_index = doc
            .machines()
            .iter()
            .position(|machine| machine.label == Some("Workflow Machine"))
            .expect("workflow machine");
        let task_index = doc
            .machines()
            .iter()
            .position(|machine| machine.label == Some("Task Machine"))
            .expect("task machine");
        let mut app = InspectorApp::new(
            doc,
            empty_heuristic_overlay(),
            CompositionSuggestionOverlay::from_suggestions(vec![CompositionSuggestion {
                index: 0,
                severity: CompositionSuggestionSeverity::Warning,
                kind: crate::suggestions::CompositionSuggestionKind::MissingCompositionRole,
                source_machine: workflow_index,
                target_machine: task_index,
                exact_relation_indices: vec![0],
                heuristic_relation_indices: Vec::new(),
                exact_counts: vec![CodebaseRelationCount {
                    kind: CodebaseRelationKind::TransitionParam,
                    basis: CodebaseRelationBasis::DirectTypeSyntax,
                    count: 1,
                }],
                heuristic_counts: Vec::new(),
            }]),
            "/tmp/workspace/Cargo.toml".to_owned(),
        );
        app.workspace_section = WorkspaceSection::Gaps;
        app.search_query = "missing composition role".to_owned();
        app.clamp_indices();

        let path = app.selected_path_item().expect("gap path");
        assert_eq!(path.target_machine, task_index);
    }

    #[test]
    fn app_selects_lane_modes_and_surfaces_heuristic_relations() {
        let doc = fixture_doc();
        let overlay = fixture_heuristic_overlay(&doc);
        let mut app = fixture_app(doc, overlay);
        let workflow_index = app
            .doc
            .machines()
            .iter()
            .position(|machine| machine.label == Some("Workflow Machine"))
            .expect("workflow machine should exist");
        app.select_machine(workflow_index);
        app.machine_section = MachineSection::Transitions;
        app.machine_item_index = 0;

        assert_eq!(app.lane_mode, LaneMode::Exact);
        app.handle_key(KeyEvent::new(KeyCode::Char('h'), KeyModifiers::NONE));
        assert_eq!(app.lane_mode, LaneMode::Heuristic);
        let heuristic_items = app.relation_items();
        assert_eq!(heuristic_items.len(), 2);
        assert!(matches!(heuristic_items[0], RelationItem::Heuristic(_)));

        app.handle_key(KeyEvent::new(KeyCode::Char('m'), KeyModifiers::NONE));
        assert_eq!(app.lane_mode, LaneMode::Mixed);

        app.handle_key(KeyEvent::new(KeyCode::Char('e'), KeyModifiers::NONE));
        assert_eq!(app.lane_mode, LaneMode::Exact);
    }

    #[test]
    fn mixed_lane_hides_heuristic_relations_covered_by_exact() {
        let doc = fixture_doc();
        let overlay = fixture_heuristic_overlay(&doc);
        let mut app = fixture_app(doc, overlay);
        let workflow_index = app
            .doc
            .machines()
            .iter()
            .position(|machine| machine.label == Some("Workflow Machine"))
            .expect("workflow machine should exist");
        app.select_machine(workflow_index);
        app.machine_section = MachineSection::Transitions;
        app.machine_item_index = 0;
        app.lane_mode = LaneMode::Mixed;

        let relation_items = app.relation_items();
        assert_eq!(relation_items.len(), 1);
        assert!(matches!(relation_items[0], RelationItem::Exact(_)));

        app.machine_section = MachineSection::Overview;
        let summary_items = app.summary_items();
        assert_eq!(summary_items.len(), 1);
        assert!(matches!(summary_items[0], SummaryItem::Exact(_)));
    }

    #[test]
    fn heuristic_filters_only_trim_heuristic_lane() {
        let doc = fixture_doc();
        let overlay = fixture_heuristic_overlay(&doc);
        let mut app = fixture_app(doc, overlay);
        let workflow_index = app
            .doc
            .machines()
            .iter()
            .position(|machine| machine.label == Some("Workflow Machine"))
            .expect("workflow machine should exist");
        app.select_machine(workflow_index);
        app.workspace_section = WorkspaceSection::Machines;
        app.machine_section = MachineSection::Transitions;
        app.machine_item_index = 0;
        app.lane_mode = LaneMode::Heuristic;

        assert_eq!(app.relation_items().len(), 2);
        app.handle_key(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE));
        assert_eq!(app.relation_items().len(), 1);
        let detail = app
            .selected_relation_detail()
            .expect("signature heuristic relation should remain");
        let RelationDetailSelection::Heuristic { detail, .. } = detail else {
            panic!("expected heuristic relation detail");
        };
        assert_eq!(
            detail.relation.evidence_kind,
            HeuristicEvidenceKind::Signature
        );
        app.handle_key(KeyEvent::new(KeyCode::Char('0'), KeyModifiers::NONE));
        assert_eq!(app.relation_items().len(), 2);
    }

    #[test]
    fn heuristic_detail_marks_exact_shadowing() {
        let doc = fixture_doc();
        let overlay = fixture_heuristic_overlay(&doc);
        let mut app = fixture_app(doc, overlay);
        let workflow_index = app
            .doc
            .machines()
            .iter()
            .position(|machine| machine.label == Some("Workflow Machine"))
            .expect("workflow machine should exist");
        app.select_machine(workflow_index);
        app.workspace_section = WorkspaceSection::Machines;
        app.machine_section = MachineSection::Transitions;
        app.machine_item_index = 0;
        app.lane_mode = LaneMode::Heuristic;
        app.focus = Focus::Detail;
        app.clamp_indices();

        let text = text_contents(app.detail_text());
        assert!(text.contains("exact lane already covers this relationship"));
    }

    #[test]
    fn empty_state_relations_hint_toward_overview_and_transitions() {
        let doc = fixture_doc();
        let mut app = fixture_app(doc, empty_heuristic_overlay());
        let workflow_index = app
            .doc
            .machines()
            .iter()
            .position(|machine| machine.label == Some("Workflow Machine"))
            .expect("workflow machine should exist");
        app.select_machine(workflow_index);
        app.workspace_section = WorkspaceSection::Machines;
        app.machine_section = MachineSection::States;
        app.machine_item_index = app
            .machine_items()
            .iter()
            .position(|item| matches!(item, MachineItem::State(0)))
            .expect("draft state should exist");
        app.focus = Focus::Detail;

        let text = text_contents(app.detail_text());
        assert!(text.contains("No exact relations for state Draft [build]."));
        assert!(text.contains("Try Overview to inspect machine-level edges"));
        assert!(text
            .contains("Try Transitions to inspect transition-parameter and attested-route edges."));
    }

    #[test]
    fn exact_lane_empty_relations_can_point_to_heuristic_mode() {
        let doc = fixture_doc();
        let overlay = fixture_heuristic_overlay(&doc);
        let mut app = fixture_app(doc, overlay);
        let workflow_index = app
            .doc
            .machines()
            .iter()
            .position(|machine| machine.label == Some("Workflow Machine"))
            .expect("workflow machine should exist");
        app.select_machine(workflow_index);
        app.workspace_section = WorkspaceSection::Machines;
        app.machine_section = MachineSection::States;
        app.machine_item_index = app
            .machine_items()
            .iter()
            .position(|item| matches!(item, MachineItem::State(0)))
            .expect("draft state should exist");
        app.focus = Focus::Detail;

        let text = text_contents(app.detail_text());
        assert!(text.contains("Switch to heuristic (`h`) or mixed (`m`) mode"));
    }

    #[test]
    fn unavailable_heuristic_lane_is_explicit() {
        let doc = fixture_doc();
        let overlay = HeuristicOverlay::from_parts(
            HeuristicStatusKind::Unavailable,
            vec![HeuristicDiagnostic {
                context: "package fixture-app".to_owned(),
                message: "failed to parse source".to_owned(),
            }],
            Vec::new(),
        );
        let mut app = fixture_app(doc, overlay);
        app.lane_mode = LaneMode::Heuristic;
        app.workspace_section = WorkspaceSection::Machines;
        app.focus = Focus::Detail;

        let text = text_contents(app.detail_text());
        assert!(text.contains("heuristics unavailable"));
        assert!(text.contains("failed to parse source"));
    }
}
