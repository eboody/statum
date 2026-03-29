use std::collections::{BTreeMap, BTreeSet};
use std::io;

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
    CodebaseDoc, CodebaseMachine, CodebaseMachineRelationGroup,
    CodebaseMachineRelationGroupSemantic, CodebaseRelation, CodebaseRelationBasis,
    CodebaseRelationCount, CodebaseRelationDetail, CodebaseRelationKind, CodebaseRelationSource,
    CodebaseState, CodebaseTransition, CodebaseValidatorEntry,
};

use crate::heuristics::{
    HeuristicDiagnostic, HeuristicEvidenceKind, HeuristicMachineRelationGroup, HeuristicOverlay,
    HeuristicRelation, HeuristicRelationCount, HeuristicRelationDetail, HeuristicRelationSource,
    HeuristicStatusKind,
};
use crate::journeys::{
    JourneyNodeReference, JourneyNodeRole, JourneyOverlay, JourneySegmentKind, ResolvedJourney,
    ResolvedJourneyNode, ResolvedJourneySegment, visible_journey_counts,
};
use crate::suggestions::{
    CompositionSuggestion, CompositionSuggestionOverlay, CompositionSuggestionSeverity,
};

pub fn run(
    doc: CodebaseDoc,
    heuristic: HeuristicOverlay,
    suggestions: CompositionSuggestionOverlay,
    journeys: JourneyOverlay,
    workspace_label: String,
) -> io::Result<()> {
    let mut terminal = setup_terminal()?;
    let mut app = InspectorApp::new(doc, heuristic, suggestions, journeys, workspace_label);

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
    BottomView,
}

impl Focus {
    fn next(self) -> Self {
        match self {
            Self::Workspace => Self::MainView,
            Self::MainView => Self::BottomView,
            Self::BottomView => Self::Workspace,
        }
    }

    fn previous(self) -> Self {
        match self {
            Self::Workspace => Self::BottomView,
            Self::MainView => Self::Workspace,
            Self::BottomView => Self::MainView,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WorkspaceSection {
    Machines,
    Journeys,
}

impl WorkspaceSection {
    fn label(self) -> &'static str {
        match self {
            Self::Machines => "Machines",
            Self::Journeys => "Journeys",
        }
    }

    fn next(self, has_journeys: bool) -> Self {
        match (self, has_journeys) {
            (_, false) => Self::Machines,
            (Self::Machines, true) => Self::Journeys,
            (Self::Journeys, true) => Self::Machines,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MachineSection {
    States,
    Transitions,
    Validators,
    Summary,
}

impl MachineSection {
    const ORDER: [Self; 4] = [
        Self::States,
        Self::Transitions,
        Self::Validators,
        Self::Summary,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::States => "States",
            Self::Transitions => "Transitions",
            Self::Validators => "Validators",
            Self::Summary => "Summary",
        }
    }

    fn next(self) -> Self {
        match self {
            Self::States => Self::Transitions,
            Self::Transitions => Self::Validators,
            Self::Validators => Self::Summary,
            Self::Summary => Self::States,
        }
    }

    fn previous(self) -> Self {
        match self {
            Self::States => Self::Summary,
            Self::Transitions => Self::States,
            Self::Validators => Self::Transitions,
            Self::Summary => Self::Validators,
        }
    }
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

    fn toggle(self) -> Self {
        match self {
            Self::Outbound => Self::Inbound,
            Self::Inbound => Self::Outbound,
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

    fn next(self) -> Self {
        match self {
            Self::Exact => Self::Heuristic,
            Self::Heuristic => Self::Mixed,
            Self::Mixed => Self::Exact,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SummaryDirection {
    Outbound,
    Inbound,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ExactSummaryItem {
    direction: SummaryDirection,
    group: CodebaseMachineRelationGroup,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct HeuristicSummaryItem {
    direction: SummaryDirection,
    group: HeuristicMachineRelationGroup,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum SummaryItem {
    Exact(ExactSummaryItem),
    Heuristic(HeuristicSummaryItem),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum InputMode {
    Normal,
    Search,
}

impl InputMode {
    fn label(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Search => "search",
        }
    }
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
    Summary(SummaryItem),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum JourneyNodeItem {
    Node(usize),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum JourneySegmentItem {
    Segment(usize),
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

#[derive(Debug)]
struct InspectorApp {
    doc: CodebaseDoc,
    heuristic: HeuristicOverlay,
    suggestions: CompositionSuggestionOverlay,
    journeys: JourneyOverlay,
    workspace_label: String,
    workspace_section: WorkspaceSection,
    selected_machine: usize,
    selected_journey: usize,
    input_mode: InputMode,
    search_query: String,
    filters: ExactFilters,
    heuristic_filters: HeuristicFilters,
    lane_mode: LaneMode,
    focus: Focus,
    machine_section: MachineSection,
    machine_item_index: usize,
    journey_node_index: usize,
    relation_direction: RelationDirection,
    relation_index: usize,
    should_quit: bool,
}

impl InspectorApp {
    fn new(
        doc: CodebaseDoc,
        heuristic: HeuristicOverlay,
        suggestions: CompositionSuggestionOverlay,
        journeys: JourneyOverlay,
        workspace_label: String,
    ) -> Self {
        let workspace_section = if journeys.is_empty() {
            WorkspaceSection::Machines
        } else {
            WorkspaceSection::Journeys
        };
        let mut app = Self {
            doc,
            heuristic,
            suggestions,
            journeys,
            workspace_label,
            workspace_section,
            selected_machine: 0,
            selected_journey: 0,
            input_mode: InputMode::Normal,
            search_query: String::new(),
            filters: ExactFilters::default(),
            heuristic_filters: HeuristicFilters::default(),
            lane_mode: LaneMode::Exact,
            focus: Focus::Workspace,
            machine_section: MachineSection::States,
            machine_item_index: 0,
            journey_node_index: 0,
            relation_direction: RelationDirection::Outbound,
            relation_index: 0,
            should_quit: false,
        };
        app.clamp_indices();
        if app.workspace_section == WorkspaceSection::Machines {
            if let Some(machine) = app.current_machine() {
                app.machine_section = app.preferred_machine_section(machine.index);
            }
        }
        app
    }

    fn focus_label(&self) -> &'static str {
        match (self.workspace_section, self.focus) {
            (WorkspaceSection::Machines, Focus::Workspace) => "machines",
            (WorkspaceSection::Machines, Focus::MainView) => "machine",
            (WorkspaceSection::Machines, Focus::BottomView) => "relations",
            (WorkspaceSection::Journeys, Focus::Workspace) => "journeys",
            (WorkspaceSection::Journeys, Focus::MainView) => "journey",
            (WorkspaceSection::Journeys, Focus::BottomView) => "segments",
        }
    }

    fn shows_journey_heuristics(&self) -> bool {
        self.lane_mode.shows_heuristic()
    }

    fn current_journey(&self) -> Option<&ResolvedJourney> {
        let visible = self.visible_journey_indices();
        let journey_index = visible
            .iter()
            .find(|journey_index| **journey_index == self.selected_journey)
            .copied()
            .or_else(|| visible.first().copied())?;
        self.journeys.journey(journey_index)
    }

    fn visible_journey_indices(&self) -> Vec<usize> {
        let query = self.normalized_query();
        self.journeys
            .journeys()
            .iter()
            .filter(|journey| self.journey_matches_query(journey, query.as_deref()))
            .map(|journey| journey.index)
            .collect()
    }

    fn select_journey(&mut self, journey_index: usize) {
        self.selected_journey = journey_index;
        self.journey_node_index = 0;
        self.relation_index = 0;
    }

    fn journey_node_items(&self) -> Vec<JourneyNodeItem> {
        let Some(journey) = self.current_journey() else {
            return Vec::new();
        };
        journey
            .nodes
            .iter()
            .filter(|node| self.journey_node_matches_query(journey, node, self.normalized_query().as_deref()))
            .map(|node| JourneyNodeItem::Node(node.index))
            .collect()
    }

    fn journey_segment_items(&self) -> Vec<JourneySegmentItem> {
        let Some(journey) = self.current_journey() else {
            return Vec::new();
        };
        journey
            .segments
            .iter()
            .filter(|segment| {
                self.journey_segment_matches_query(journey, segment, self.normalized_query().as_deref())
            })
            .map(|segment| JourneySegmentItem::Segment(segment.index))
            .collect()
    }

    fn selected_journey_node(&self) -> Option<&ResolvedJourneyNode> {
        let item = self.journey_node_items().get(self.journey_node_index).copied()?;
        let JourneyNodeItem::Node(index) = item;
        self.current_journey()?.nodes.get(index)
    }

    fn selected_journey_segment(&self) -> Option<&ResolvedJourneySegment> {
        let item = self.journey_segment_items().get(self.relation_index).copied()?;
        let JourneySegmentItem::Segment(index) = item;
        self.current_journey()?.segments.get(index)
    }

    fn select_machine(&mut self, machine_index: usize) {
        self.selected_machine = machine_index;
        self.machine_section = self.preferred_machine_section(machine_index);
        self.machine_item_index = 0;
        self.relation_index = 0;
    }

    fn current_machine(&self) -> Option<&CodebaseMachine> {
        let visible_machines = self.visible_machine_indices();
        let machine_index = visible_machines
            .iter()
            .find(|machine_index| **machine_index == self.selected_machine)
            .copied()
            .or_else(|| visible_machines.first().copied())?;
        self.doc.machine(machine_index)
    }

    fn visible_machine_indices(&self) -> Vec<usize> {
        let query = self.normalized_query();
        self.doc
            .machines()
            .iter()
            .filter(|machine| self.machine_matches_query(machine, query.as_deref()))
            .map(|machine| machine.index)
            .collect()
    }

    fn preferred_machine_section(&self, machine_index: usize) -> MachineSection {
        if self.machine_visible_summary_counts(machine_index) != (0, 0) {
            MachineSection::Summary
        } else {
            MachineSection::States
        }
    }

    fn machine_suggestions(&self, machine_index: usize) -> Vec<&CompositionSuggestion> {
        self.suggestions.machine_suggestions(machine_index).collect()
    }

    fn composition_diagnostic_counts(&self) -> (usize, usize) {
        (
            self.suggestions.warning_count(),
            self.suggestions.suggestion_count(),
        )
    }
}

impl InspectorApp {
    fn handle_key(&mut self, key: KeyEvent) {
        if self.input_mode == InputMode::Search {
            self.handle_search_key(key);
            self.clamp_indices();
            return;
        }

        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
            KeyCode::Char('/') => self.input_mode = InputMode::Search,
            KeyCode::Char('0') => {
                self.filters.clear();
                self.heuristic_filters.clear();
            }
            KeyCode::Char('1') => self.filters.toggle_kind(CodebaseRelationKind::StatePayload),
            KeyCode::Char('2') => self.filters.toggle_kind(CodebaseRelationKind::MachineField),
            KeyCode::Char('3') => self
                .filters
                .toggle_kind(CodebaseRelationKind::TransitionParam),
            KeyCode::Char('4') => self
                .filters
                .toggle_basis(CodebaseRelationBasis::DirectTypeSyntax),
            KeyCode::Char('5') => self
                .filters
                .toggle_basis(CodebaseRelationBasis::DeclaredReferenceType),
            KeyCode::Char('6') => self
                .heuristic_filters
                .toggle_evidence_kind(HeuristicEvidenceKind::Signature),
            KeyCode::Char('7') => self
                .heuristic_filters
                .toggle_evidence_kind(HeuristicEvidenceKind::Body),
            KeyCode::Char('m') => {
                self.lane_mode = self.lane_mode.next();
                if self.workspace_section == WorkspaceSection::Machines {
                    if let Some(machine) = self.current_machine() {
                        self.machine_section = self.preferred_machine_section(machine.index);
                        self.machine_item_index = 0;
                        self.relation_index = 0;
                    }
                }
            }
            KeyCode::Char('w') => {
                self.workspace_section =
                    self.workspace_section.next(!self.journeys.is_empty());
                self.machine_item_index = 0;
                self.journey_node_index = 0;
                self.relation_index = 0;
            }
            KeyCode::Tab => self.focus = self.focus.next(),
            KeyCode::BackTab => self.focus = self.focus.previous(),
            KeyCode::Left | KeyCode::Char('h') => self.move_left(),
            KeyCode::Right | KeyCode::Char('l') => self.move_right(),
            KeyCode::Up | KeyCode::Char('k') => self.move_up(),
            KeyCode::Down | KeyCode::Char('j') => self.move_down(),
            _ => {}
        }

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
                if self.workspace_section == WorkspaceSection::Machines {
                    self.machine_section = self.machine_section.previous();
                }
            }
            Focus::BottomView => {
                if self.workspace_section == WorkspaceSection::Machines {
                    self.relation_direction = self.relation_direction.toggle();
                }
            }
        }
    }

    fn move_right(&mut self) {
        match self.focus {
            Focus::Workspace => self.focus = self.focus.next(),
            Focus::MainView => {
                if self.workspace_section == WorkspaceSection::Machines {
                    self.machine_section = self.machine_section.next();
                }
            }
            Focus::BottomView => {
                if self.workspace_section == WorkspaceSection::Machines {
                    self.relation_direction = self.relation_direction.toggle();
                }
            }
        }
    }

    fn move_up(&mut self) {
        match self.focus {
            Focus::Workspace => match self.workspace_section {
                WorkspaceSection::Machines => {
                    let visible = self.visible_machine_indices();
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
                WorkspaceSection::Journeys => {
                    let visible = self.visible_journey_indices();
                    let Some(current_position) = visible
                        .iter()
                        .position(|journey_index| *journey_index == self.selected_journey)
                    else {
                        if let Some(&first) = visible.first() {
                            self.select_journey(first);
                        }
                        return;
                    };
                    if current_position > 0 {
                        self.select_journey(visible[current_position - 1]);
                    }
                }
            }
            Focus::MainView => match self.workspace_section {
                WorkspaceSection::Machines => {
                    self.machine_item_index = self.machine_item_index.saturating_sub(1);
                }
                WorkspaceSection::Journeys => {
                    self.journey_node_index = self.journey_node_index.saturating_sub(1);
                }
            }
            Focus::BottomView => {
                self.relation_index = self.relation_index.saturating_sub(1);
            }
        }
    }

    fn move_down(&mut self) {
        match self.focus {
            Focus::Workspace => match self.workspace_section {
                WorkspaceSection::Machines => {
                    let visible = self.visible_machine_indices();
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
                WorkspaceSection::Journeys => {
                    let visible = self.visible_journey_indices();
                    let Some(current_position) = visible
                        .iter()
                        .position(|journey_index| *journey_index == self.selected_journey)
                    else {
                        if let Some(&first) = visible.first() {
                            self.select_journey(first);
                        }
                        return;
                    };
                    if let Some(&next) = visible.get(current_position + 1) {
                        self.select_journey(next);
                    }
                }
            }
            Focus::MainView => match self.workspace_section {
                WorkspaceSection::Machines => {
                    self.machine_item_index = self.machine_item_index.saturating_add(1);
                }
                WorkspaceSection::Journeys => {
                    self.journey_node_index = self.journey_node_index.saturating_add(1);
                }
            }
            Focus::BottomView => {
                self.relation_index = self.relation_index.saturating_add(1);
            }
        }
    }

    fn clamp_indices(&mut self) {
        match self.workspace_section {
            WorkspaceSection::Machines => {
                let visible_machines = self.visible_machine_indices();
                if visible_machines.is_empty() {
                    self.selected_machine = 0;
                    self.machine_item_index = 0;
                    self.relation_index = 0;
                    return;
                }

                if !visible_machines.contains(&self.selected_machine) {
                    self.select_machine(visible_machines[0]);
                }
                self.machine_item_index = self
                    .machine_item_index
                    .min(self.machine_items().len().saturating_sub(1));
                self.relation_index = self
                    .relation_index
                    .min(self.relation_items().len().saturating_sub(1));
            }
            WorkspaceSection::Journeys => {
                let visible_journeys = self.visible_journey_indices();
                if visible_journeys.is_empty() {
                    self.selected_journey = 0;
                    self.journey_node_index = 0;
                    self.relation_index = 0;
                    return;
                }

                if !visible_journeys.contains(&self.selected_journey) {
                    self.select_journey(visible_journeys[0]);
                }
                self.journey_node_index = self
                    .journey_node_index
                    .min(self.journey_node_items().len().saturating_sub(1));
                self.relation_index = self
                    .relation_index
                    .min(self.journey_segment_items().len().saturating_sub(1));
            }
        }
    }

    fn machine_visible_summary_counts(&self, machine_index: usize) -> (usize, usize) {
        let exact = if self.lane_mode.shows_exact() {
            self.filtered_machine_relation_groups()
                .into_iter()
                .filter(|group| {
                    group.from_machine != group.to_machine
                        && (group.from_machine == machine_index || group.to_machine == machine_index)
                })
                .count()
        } else {
            0
        };
        let heuristic = if self.lane_mode.shows_heuristic() {
            self.filtered_heuristic_machine_relation_groups()
                .into_iter()
                .filter(|group| {
                    group.from_machine != group.to_machine
                        && (group.from_machine == machine_index || group.to_machine == machine_index)
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
            .into_iter()
            .any(|group| {
                group.from_machine != group.to_machine
                    && (group.from_machine == machine_index || group.to_machine == machine_index)
            })
    }

    fn machine_section_label(&self, machine: &CodebaseMachine, section: MachineSection) -> String {
        match section {
            MachineSection::States => format!("States ({})", machine.states.len()),
            MachineSection::Transitions => {
                format!("Transitions ({})", machine.transitions.len())
            }
            MachineSection::Validators => {
                format!("Validators ({})", machine.validator_entries.len())
            }
            MachineSection::Summary => {
                let (exact, heuristic) = self.machine_visible_summary_counts(machine.index);
                match self.lane_mode {
                    LaneMode::Exact => format!("Summary ({exact})"),
                    LaneMode::Heuristic => format!("Summary ({heuristic})"),
                    LaneMode::Mixed => format!("Summary ({exact}e/{heuristic}h)"),
                }
            }
        }
    }

    fn machine_items(&self) -> Vec<MachineItem> {
        let Some(machine) = self.current_machine() else {
            return Vec::new();
        };
        let query = self.normalized_query();
        match self.machine_section {
            MachineSection::States => machine
                .states
                .iter()
                .filter(|state| self.state_matches_query(state, query.as_deref()))
                .map(|state| MachineItem::State(state.index))
                .collect(),
            MachineSection::Transitions => machine
                .transitions
                .iter()
                .filter(|transition| self.transition_matches_query(transition, query.as_deref()))
                .map(|transition| MachineItem::Transition(transition.index))
                .collect(),
            MachineSection::Validators => machine
                .validator_entries
                .iter()
                .filter(|entry| self.validator_matches_query(entry, query.as_deref()))
                .map(|entry| MachineItem::Validator(entry.index))
                .collect(),
            MachineSection::Summary => self
                .summary_items()
                .into_iter()
                .filter(|item| self.summary_item_matches_query(item, query.as_deref()))
                .map(MachineItem::Summary)
                .collect(),
        }
    }

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
            MachineItem::Summary(SummaryItem::Exact(item)) => {
                let arrow = match item.direction {
                    SummaryDirection::Outbound => "out",
                    SummaryDirection::Inbound => "in",
                };
                let peer_machine = match item.direction {
                    SummaryDirection::Outbound => self.doc.machine(item.group.to_machine),
                    SummaryDirection::Inbound => self.doc.machine(item.group.from_machine),
                }
                .expect("summary peer machine should exist");
                format!(
                    "[exact] {arrow} {} : {}",
                    render_machine_label(peer_machine),
                    item.group.display_label()
                )
            }
            MachineItem::Summary(SummaryItem::Heuristic(item)) => {
                let arrow = match item.direction {
                    SummaryDirection::Outbound => "out",
                    SummaryDirection::Inbound => "in",
                };
                let peer_machine = match item.direction {
                    SummaryDirection::Outbound => self.doc.machine(item.group.to_machine),
                    SummaryDirection::Inbound => self.doc.machine(item.group.from_machine),
                }
                .expect("summary peer machine should exist");
                format!(
                    "[heur] {arrow} {} : {}",
                    render_machine_label(peer_machine),
                    item.group.display_label()
                )
            }
        }
    }

    fn journey_node_label(&self, journey: &ResolvedJourney, node: &ResolvedJourneyNode) -> String {
        let role = match node.role {
            JourneyNodeRole::Entry => "entry",
            JourneyNodeRole::Step => "step",
            JourneyNodeRole::Outcome => "outcome",
        };
        format!("[{role}] {}", self.journey_node_subject_label(journey, node))
    }

    fn journey_node_subject_label(
        &self,
        _journey: &ResolvedJourney,
        node: &ResolvedJourneyNode,
    ) -> String {
        match &node.reference {
            JourneyNodeReference::Machine { machine } => self
                .doc
                .machine(*machine)
                .map(|machine| render_machine_label(machine).to_owned())
                .unwrap_or_else(|| "<missing machine>".to_owned()),
            JourneyNodeReference::State { machine, state } => self
                .doc
                .machine(*machine)
                .and_then(|machine| machine.state(*state).map(|state| (machine, state)))
                .map(|(machine, state)| {
                    format!(
                        "{} :: {}",
                        render_machine_label(machine),
                        render_state_label(state)
                    )
                })
                .unwrap_or_else(|| "<missing state>".to_owned()),
            JourneyNodeReference::Validator {
                machine,
                entry,
                source_type_display,
            } => self
                .doc
                .machine(*machine)
                .and_then(|machine| machine.validator_entry(*entry).map(|entry| (machine, entry)))
                .map(|(machine, entry)| {
                    format!(
                        "{} -> {}",
                        entry.display_label(),
                        render_machine_label(machine)
                    )
                })
                .unwrap_or_else(|| format!("{source_type_display}::into_machine()")),
            JourneyNodeReference::Bridge { type_display, .. } => {
                format!("bridge {type_display}")
            }
        }
    }

    fn journey_segment_label(
        &self,
        journey: &ResolvedJourney,
        segment: &ResolvedJourneySegment,
    ) -> String {
        let kind = segment.visible_kind(self.shows_journey_heuristics()).display_label();
        let from = journey
            .nodes
            .get(segment.from_node)
            .map(|node| self.journey_node_subject_label(journey, node))
            .unwrap_or_else(|| "<missing>".to_owned());
        let to = journey
            .nodes
            .get(segment.to_node)
            .map(|node| self.journey_node_subject_label(journey, node))
            .unwrap_or_else(|| "<missing>".to_owned());
        format!("[{kind}] {from} -> {to}")
    }

    fn summary_items(&self) -> Vec<SummaryItem> {
        let Some(machine) = self.current_machine() else {
            return Vec::new();
        };
        let mut items = Vec::new();

        if self.lane_mode.shows_exact() {
            items.extend(
                self.filtered_machine_relation_groups()
                    .into_iter()
                    .filter_map(|group| {
                        if group.from_machine == machine.index
                            && group.from_machine != group.to_machine
                        {
                            Some(SummaryItem::Exact(ExactSummaryItem {
                                direction: SummaryDirection::Outbound,
                                group,
                            }))
                        } else if group.to_machine == machine.index
                            && group.from_machine != group.to_machine
                        {
                            Some(SummaryItem::Exact(ExactSummaryItem {
                                direction: SummaryDirection::Inbound,
                                group,
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
                    .into_iter()
                    .filter_map(|group| {
                        if group.from_machine == machine.index
                            && group.from_machine != group.to_machine
                        {
                            Some(SummaryItem::Heuristic(HeuristicSummaryItem {
                                direction: SummaryDirection::Outbound,
                                group,
                            }))
                        } else if group.to_machine == machine.index
                            && group.from_machine != group.to_machine
                        {
                            Some(SummaryItem::Heuristic(HeuristicSummaryItem {
                                direction: SummaryDirection::Inbound,
                                group,
                            }))
                        } else {
                            None
                        }
                    }),
            );
        }

        items
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
            MachineSection::Validators | MachineSection::Summary => {
                Some(RelationSubject::Machine {
                    machine: machine.index,
                })
            }
        }
    }

    fn relation_items(&self) -> Vec<RelationItem> {
        let query = self.normalized_query();
        let Some(subject) = self.relation_subject() else {
            return Vec::new();
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
        items
    }

    fn filtered_machine_relation_groups(&self) -> Vec<CodebaseMachineRelationGroup> {
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
        filtered
    }

    fn filtered_heuristic_machine_relation_groups(&self) -> Vec<HeuristicMachineRelationGroup> {
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
        filtered
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

    fn visible_machine_relation_groups(&self) -> Vec<CodebaseMachineRelationGroup> {
        let visible_machine_indices = self
            .visible_machine_indices()
            .into_iter()
            .collect::<BTreeSet<_>>();
        self.filtered_machine_relation_groups()
            .into_iter()
            .filter(|group| {
                visible_machine_indices.contains(&group.from_machine)
                    && visible_machine_indices.contains(&group.to_machine)
            })
            .collect()
    }

    fn visible_heuristic_machine_relation_groups(&self) -> Vec<HeuristicMachineRelationGroup> {
        let visible_machine_indices = self
            .visible_machine_indices()
            .into_iter()
            .collect::<BTreeSet<_>>();
        self.filtered_heuristic_machine_relation_groups()
            .into_iter()
            .filter(|group| {
                visible_machine_indices.contains(&group.from_machine)
                    && visible_machine_indices.contains(&group.to_machine)
            })
            .collect()
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
        let visible_machine_indices = self.visible_machine_indices();
        if visible_machine_indices.is_empty() {
            return 0;
        }

        let mut adjacency = vec![Vec::new(); self.doc.machines().len()];
        if self.lane_mode.shows_exact() {
            for group in self.visible_machine_relation_groups() {
                if group.from_machine == group.to_machine {
                    continue;
                }
                adjacency[group.from_machine].push(group.to_machine);
                adjacency[group.to_machine].push(group.from_machine);
            }
        }
        if self.lane_mode.shows_heuristic() {
            for group in self.visible_heuristic_machine_relation_groups() {
                if group.from_machine == group.to_machine {
                    continue;
                }
                adjacency[group.from_machine].push(group.to_machine);
                adjacency[group.to_machine].push(group.from_machine);
            }
        }

        let mut seen = vec![false; self.doc.machines().len()];
        let mut groups = 0;
        for machine_index in visible_machine_indices {
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

        groups
    }

    fn render(&self, frame: &mut Frame) {
        let vertical = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Length(6)])
            .split(frame.area());
        let horizontal = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(32),
                Constraint::Min(48),
                Constraint::Length(38),
            ])
            .split(vertical[0]);
        let center = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(12), Constraint::Length(12)])
            .split(horizontal[1]);

        self.render_workspace(frame, horizontal[0]);
        match self.workspace_section {
            WorkspaceSection::Machines => {
                self.render_machine_view(frame, center[0]);
                self.render_relations(frame, center[1]);
            }
            WorkspaceSection::Journeys => {
                self.render_journey_view(frame, center[0]);
                self.render_journey_segments(frame, center[1]);
            }
        }
        self.render_detail(frame, horizontal[2]);
        self.render_status(frame, vertical[1]);
    }

    fn render_workspace(&self, frame: &mut Frame, area: Rect) {
        let block = titled_block(
            format!("Workspace {}", self.workspace_label),
            self.focus == Focus::Workspace,
        );
        let inner = block.inner(area);
        frame.render_widget(block, area);

        let sections = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Length(6), Constraint::Min(0)])
            .split(inner);

        let tabs = Tabs::new(
            [WorkspaceSection::Machines, WorkspaceSection::Journeys]
                .into_iter()
                .map(|section| Line::from(section.label()))
                .collect::<Vec<_>>(),
        )
        .select(match self.workspace_section {
            WorkspaceSection::Machines => 0,
            WorkspaceSection::Journeys => 1,
        })
        .highlight_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        );
        frame.render_widget(tabs, sections[0]);

        let visible_machine_indices = self.visible_machine_indices();
        let visible_machine_count = visible_machine_indices.len();
        let total_machine_count = self.doc.machines().len();
        let visible_journey_indices = self.visible_journey_indices();
        let visible_journey_count = visible_journey_indices.len();
        let total_journey_count = self.journeys.journeys().len();
        let visible_exact_summary_edges = self
            .visible_machine_relation_groups()
            .into_iter()
            .filter(|group| group.from_machine != group.to_machine)
            .count();
        let visible_heuristic_summary_edges = self
            .visible_heuristic_machine_relation_groups()
            .into_iter()
            .filter(|group| group.from_machine != group.to_machine)
            .count();
        let search_status = if self.has_search_query() {
            format!("/{}", self.search_query.trim())
        } else {
            "<none>".to_owned()
        };

        let mut lines = match self.workspace_section {
            WorkspaceSection::Machines => vec![
                Line::from(format!(
                    "machines: {visible_machine_count}/{total_machine_count}"
                )),
                Line::from(format!("groups: {}", self.disconnected_group_count())),
            ],
            WorkspaceSection::Journeys => vec![Line::from(format!(
                "journeys: {visible_journey_count}/{total_journey_count}"
            ))],
        };
        if self.workspace_section == WorkspaceSection::Machines && self.lane_mode.shows_exact() {
            lines.push(Line::from(format!(
                "exact summary edges: {visible_exact_summary_edges}"
            )));
        }
        if self.workspace_section == WorkspaceSection::Machines && self.lane_mode.shows_heuristic()
        {
            lines.push(Line::from(format!(
                "heuristic summary edges: {visible_heuristic_summary_edges}"
            )));
        }
        lines.push(Line::from(format!("search: {search_status}")));
        lines.push(Line::from(format!("lane: {}", self.lane_mode.label())));
        let counts = Paragraph::new(Text::from(lines));
        frame.render_widget(counts, sections[1]);

        let (items, selected) = match self.workspace_section {
            WorkspaceSection::Machines => (
                visible_machine_indices
                    .iter()
                    .filter_map(|machine_index| self.doc.machine(*machine_index))
                    .map(|machine| ListItem::new(render_machine_label(machine).to_owned()))
                    .collect::<Vec<_>>(),
                visible_machine_indices
                    .iter()
                    .position(|machine_index| *machine_index == self.selected_machine),
            ),
            WorkspaceSection::Journeys => (
                visible_journey_indices
                    .iter()
                    .filter_map(|journey_index| self.journeys.journey(*journey_index))
                    .map(|journey| {
                        let counts =
                            visible_journey_counts(journey, self.shows_journey_heuristics());
                        ListItem::new(format!(
                            "{} [{}e {}d {}h {}m]",
                            journey.display_label(),
                            counts.exact,
                            counts.declared,
                            counts.heuristic,
                            counts.missing
                        ))
                    })
                    .collect::<Vec<_>>(),
                visible_journey_indices
                    .iter()
                    .position(|journey_index| *journey_index == self.selected_journey),
            ),
        };
        let mut state = ListState::default().with_selected(selected);
        let list = if items.is_empty() {
            List::new(vec![ListItem::new("<no matches>")])
        } else {
            List::new(items)
        }
        .highlight_style(Style::default().fg(Color::Black).bg(Color::Cyan))
        .highlight_symbol("> ");
        frame.render_stateful_widget(list, sections[2], &mut state);
    }

    fn render_machine_view(&self, frame: &mut Frame, area: Rect) {
        let title = self
            .current_machine()
            .map(|machine| {
                let (exact, heuristic) = self.machine_visible_summary_counts(machine.index);
                match self.lane_mode {
                    LaneMode::Exact if exact > 0 => {
                        format!(
                            "Machine {} [{} exact edge{}]",
                            render_machine_label(machine),
                            exact,
                            if exact == 1 { "" } else { "s" }
                        )
                    }
                    LaneMode::Heuristic if heuristic > 0 => {
                        format!(
                            "Machine {} [{} heuristic edge{}]",
                            render_machine_label(machine),
                            heuristic,
                            if heuristic == 1 { "" } else { "s" }
                        )
                    }
                    LaneMode::Mixed if exact > 0 || heuristic > 0 => format!(
                        "Machine {} [{} exact, {} heuristic]",
                        render_machine_label(machine),
                        exact,
                        heuristic
                    ),
                    _ => format!("Machine {}", render_machine_label(machine)),
                }
            })
            .unwrap_or_else(|| "Machine <no matches>".to_owned());
        let block = titled_block(title, self.focus == Focus::MainView);
        let inner = block.inner(area);
        frame.render_widget(block, area);

        let sections = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(0)])
            .split(inner);
        let tabs = Tabs::new(
            MachineSection::ORDER
                .into_iter()
                .map(|section| {
                    self.current_machine()
                        .map(|machine| Line::from(self.machine_section_label(machine, section)))
                        .unwrap_or_else(|| Line::from(section.label()))
                })
                .collect::<Vec<_>>(),
        )
        .select(
            MachineSection::ORDER
                .iter()
                .position(|section| *section == self.machine_section)
                .expect("current machine section should exist"),
        )
        .highlight_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        );
        frame.render_widget(tabs, sections[0]);

        let items = self.machine_items();
        let visible_items = self
            .current_machine()
            .map(|machine| {
                items
                    .iter()
                    .map(|item| ListItem::new(self.machine_item_label(machine, item)))
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
        .highlight_style(Style::default().fg(Color::Black).bg(Color::Cyan))
        .highlight_symbol("> ");
        frame.render_stateful_widget(list, sections[1], &mut state);
    }

    fn render_relations(&self, frame: &mut Frame, area: Rect) {
        let subject_label = self.relation_subject_label();
        let block = titled_block(
            format!("Relations [{}] {}", self.lane_mode.label(), subject_label),
            self.focus == Focus::BottomView,
        );
        let inner = block.inner(area);
        frame.render_widget(block, area);

        let sections = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(0)])
            .split(inner);
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
        .highlight_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        );
        frame.render_widget(tabs, sections[0]);

        let relation_labels = self
            .relation_items()
            .into_iter()
            .map(|relation| {
                let label = match relation {
                    RelationItem::Exact(index) => self
                        .doc
                        .relation_detail(index)
                        .map(|detail| render_relation_label(&detail))
                        .unwrap_or_else(|| "[exact] <missing relation>".to_owned()),
                    RelationItem::Heuristic(index) => self
                        .heuristic
                        .relation_detail(&self.doc, index)
                        .map(|detail| render_heuristic_relation_label(&detail))
                        .unwrap_or_else(|| "[heur] <missing relation>".to_owned()),
                };
                ListItem::new(label)
            })
            .collect::<Vec<_>>();
        let empty = relation_labels.is_empty();
        let mut state = ListState::default().with_selected((!empty).then_some(self.relation_index));
        let list = if empty {
            List::new(vec![ListItem::new(self.empty_list_label())])
        } else {
            List::new(relation_labels)
        }
        .highlight_style(Style::default().fg(Color::Black).bg(Color::Cyan))
        .highlight_symbol("> ");
        frame.render_stateful_widget(list, sections[1], &mut state);
    }

    fn render_journey_view(&self, frame: &mut Frame, area: Rect) {
        let title = self
            .current_journey()
            .map(|journey| {
                let counts = visible_journey_counts(journey, self.shows_journey_heuristics());
                format!(
                    "Journey {} [{} exact, {} declared, {} heuristic, {} missing]",
                    journey.display_label(),
                    counts.exact,
                    counts.declared,
                    counts.heuristic,
                    counts.missing
                )
            })
            .unwrap_or_else(|| "Journey <no matches>".to_owned());
        let block = titled_block(title, self.focus == Focus::MainView);
        let inner = block.inner(area);
        frame.render_widget(block, area);

        let items = self
            .current_journey()
            .map(|journey| {
                self.journey_node_items()
                    .iter()
                    .map(|item| match item {
                        JourneyNodeItem::Node(index) => journey
                            .nodes
                            .get(*index)
                            .map(|node| ListItem::new(self.journey_node_label(journey, node)))
                            .unwrap_or_else(|| ListItem::new("<missing journey node>")),
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let empty = items.is_empty();
        let mut state =
            ListState::default().with_selected((!empty).then_some(self.journey_node_index));
        let list = if empty {
            List::new(vec![ListItem::new(self.empty_list_label())])
        } else {
            List::new(items)
        }
        .highlight_style(Style::default().fg(Color::Black).bg(Color::Cyan))
        .highlight_symbol("> ");
        frame.render_stateful_widget(list, inner, &mut state);
    }

    fn render_journey_segments(&self, frame: &mut Frame, area: Rect) {
        let title = self
            .current_journey()
            .map(|journey| format!("Journey Segments [{}] {}", self.lane_mode.label(), journey.display_label()))
            .unwrap_or_else(|| "Journey Segments".to_owned());
        let block = titled_block(title, self.focus == Focus::BottomView);
        let inner = block.inner(area);
        frame.render_widget(block, area);

        let items = self
            .current_journey()
            .map(|journey| {
                self.journey_segment_items()
                    .iter()
                    .map(|item| match item {
                        JourneySegmentItem::Segment(index) => journey
                            .segments
                            .get(*index)
                            .map(|segment| {
                                ListItem::new(self.journey_segment_label(journey, segment))
                            })
                            .unwrap_or_else(|| ListItem::new("<missing journey segment>")),
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let empty = items.is_empty();
        let mut state = ListState::default().with_selected((!empty).then_some(self.relation_index));
        let list = if empty {
            List::new(vec![ListItem::new(self.empty_list_label())])
        } else {
            List::new(items)
        }
        .highlight_style(Style::default().fg(Color::Black).bg(Color::Cyan))
        .highlight_symbol("> ");
        frame.render_stateful_widget(list, inner, &mut state);
    }

    fn render_detail(&self, frame: &mut Frame, area: Rect) {
        let block = titled_block("Detail", false);
        let inner = block.inner(area);
        frame.render_widget(block, area);
        frame.render_widget(
            Paragraph::new(self.detail_text()).wrap(Wrap { trim: false }),
            inner,
        );
    }

    fn render_status(&self, frame: &mut Frame, area: Rect) {
        let search_status = if self.has_search_query() {
            format!("/{}", self.search_query.trim())
        } else {
            "<none>".to_owned()
        };
        let key_help = if self.input_mode == InputMode::Search {
            "type to search, enter/esc to finish, backspace to delete"
        } else {
            "tab shift-tab h/l j/k / w section m lane q 1 payload 2 field 3 param 4 direct 5 ref 6 sig 7 body 0 clear"
        };
        let mut lines = vec![
            Line::from(vec![
                Span::styled("focus ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(self.focus_label()),
                Span::raw("  "),
                Span::styled("mode ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(self.input_mode.label()),
                Span::raw("  "),
                Span::styled("section ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(self.workspace_section.label()),
                Span::raw("  "),
                Span::styled("lane ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(self.lane_mode.label()),
            ]),
            Line::from(format!("search {search_status}")),
        ];
        if self.workspace_section == WorkspaceSection::Machines && self.lane_mode.shows_exact() {
            lines.push(Line::from(format!(
                "exact filters kind={} basis={}",
                self.filters.kind_summary(),
                self.filters.basis_summary()
            )));
        }
        if self.workspace_section == WorkspaceSection::Machines {
            let (warnings, suggestions) = self.composition_diagnostic_counts();
            lines.push(Line::from(format!(
                "composition diagnostics {} warning, {} suggestion",
                warnings, suggestions
            )));
            if self.heuristic.status() != HeuristicStatusKind::Available {
                lines.push(Line::from(format!(
                    "heuristics {} ({})",
                    self.heuristic.status().display_label(),
                    self.heuristic.diagnostics().len()
                )));
            }
        }
        if self.workspace_section == WorkspaceSection::Machines && self.lane_mode.shows_heuristic()
        {
            lines.push(Line::from(format!(
                "heuristic filters evidence={}",
                self.heuristic_filters.evidence_summary()
            )));
            if self.heuristic.status() == HeuristicStatusKind::Available {
                lines.push(Line::from(format!(
                    "heuristics {} ({})",
                    self.heuristic.status().display_label(),
                    self.heuristic.diagnostics().len()
                )));
            }
        } else if self.workspace_section == WorkspaceSection::Journeys {
            lines.push(Line::from(
                "journeys show exact, declared, heuristic, and missing segments per lane",
            ));
        }
        lines.push(Line::from(key_help));
        let status = Paragraph::new(Text::from(lines)).wrap(Wrap { trim: false });
        frame.render_widget(status, area);
    }

    fn relation_subject_label(&self) -> String {
        match self.relation_subject() {
            None => "<no matches>".to_owned(),
            Some(RelationSubject::Machine { machine }) => {
                let machine = self
                    .doc
                    .machine(machine)
                    .expect("machine subject should exist");
                format!("for machine {}", render_machine_label(machine))
            }
            Some(RelationSubject::State { machine, state }) => {
                let machine = self
                    .doc
                    .machine(machine)
                    .expect("state subject machine should exist");
                let state = machine.state(state).expect("state subject should exist");
                format!("for state {}", render_state_label(state))
            }
            Some(RelationSubject::Transition {
                machine,
                transition,
            }) => {
                let machine = self
                    .doc
                    .machine(machine)
                    .expect("transition subject machine should exist");
                let transition = machine
                    .transition(transition)
                    .expect("transition subject should exist");
                format!("for transition {}", render_transition_label(transition))
            }
        }
    }

    fn detail_text(&self) -> Text<'static> {
        match self.focus {
            Focus::Workspace => match self.workspace_section {
                WorkspaceSection::Machines => self
                    .current_machine()
                    .map(|machine| self.machine_workspace_detail_text(machine))
                    .unwrap_or_else(|| Text::from("<no matches>")),
                WorkspaceSection::Journeys => self
                    .current_journey()
                    .map(|journey| journey_detail_text(journey, self.shows_journey_heuristics()))
                    .unwrap_or_else(|| Text::from("<no matches>")),
            },
            Focus::MainView => match self.workspace_section {
                WorkspaceSection::Machines => self.machine_detail_selection_text(),
                WorkspaceSection::Journeys => self
                    .selected_journey_node()
                    .and_then(|node| self.current_journey().map(|journey| journey_node_detail_text(journey, node, &self.doc)))
                    .unwrap_or_else(|| self.empty_journey_node_text()),
            },
            Focus::BottomView => match self.workspace_section {
                WorkspaceSection::Machines => self
                    .selected_relation_detail()
                    .map(relation_detail_selection_text)
                    .unwrap_or_else(|| self.empty_relation_text()),
                WorkspaceSection::Journeys => self
                    .selected_journey_segment()
                    .and_then(|segment| self.current_journey().map(|journey| self.journey_segment_detail_text(journey, segment)))
                    .unwrap_or_else(|| self.empty_journey_segment_text()),
            },
        }
    }

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
            MachineSection::Summary => match self.selected_machine_item() {
                Some(MachineItem::Summary(summary)) => summary_detail_text(&summary, &self.doc),
                _ => self.machine_workspace_detail_text(machine),
            },
        }
    }

    fn machine_workspace_detail_text(&self, machine: &CodebaseMachine) -> Text<'static> {
        machine_detail_text(
            machine,
            &self.doc,
            &self.machine_suggestions(machine.index),
        )
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
                    Line::from("State selections only show relations attached directly to that state."),
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
                "Try Summary to inspect machine-level edges ({} exact, {} heuristic visible).",
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
                "Switch to heuristic or mixed mode with `m` to inspect weaker source-scanned couplings.",
            ));
        }

        Text::from(lines)
    }

    fn empty_journey_node_text(&self) -> Text<'static> {
        let Some(journey) = self.current_journey() else {
            return Text::from(self.empty_list_label());
        };
        Text::from(vec![
            Line::from(format!(
                "No journey nodes are visible for `{}`.",
                journey.display_label()
            )),
            Line::from("Try clearing search filters or selecting a different journey."),
        ])
    }

    fn empty_journey_segment_text(&self) -> Text<'static> {
        let Some(journey) = self.current_journey() else {
            return Text::from(self.empty_list_label());
        };
        Text::from(vec![
            Line::from(format!(
                "No journey segments are visible for `{}`.",
                journey.display_label()
            )),
            Line::from("Try clearing search filters or switching lane mode with `m`."),
        ])
    }

    fn journey_segment_detail_text(
        &self,
        journey: &ResolvedJourney,
        segment: &ResolvedJourneySegment,
    ) -> Text<'static> {
        journey_segment_detail_text(journey, segment, self.shows_journey_heuristics(), &self.doc)
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
        if Self::query_matches_any(
            query,
            [
                machine.module_path.to_owned(),
                machine.rust_type_path.to_owned(),
                render_machine_label(machine).to_owned(),
                machine.description.unwrap_or_default().to_owned(),
                machine.docs.unwrap_or_default().to_owned(),
            ],
        ) {
            return true;
        }

        machine
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
                .any(|entry| self.validator_matches_query(entry, query))
            || (self.lane_mode.shows_exact()
                && self.machine_exact_relations_match_query(machine.index, query))
            || (self.lane_mode.shows_heuristic()
                && self.machine_heuristic_relations_match_query(machine.index, query))
            || self.machine_suggestions_match_query(machine.index, query)
    }

    fn machine_suggestions_match_query(
        &self,
        machine_index: usize,
        query: Option<&str>,
    ) -> bool {
        self.machine_suggestions(machine_index).into_iter().any(|suggestion| {
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
        Self::query_matches_any(
            query,
            [
                state.rust_name.to_owned(),
                render_state_label(state),
                state.description.unwrap_or_default().to_owned(),
                state.docs.unwrap_or_default().to_owned(),
            ],
        )
    }

    fn transition_matches_query(
        &self,
        transition: &CodebaseTransition,
        query: Option<&str>,
    ) -> bool {
        Self::query_matches_any(
            query,
            [
                transition.method_name.to_owned(),
                render_transition_label(transition).to_owned(),
                transition.description.unwrap_or_default().to_owned(),
                transition.docs.unwrap_or_default().to_owned(),
            ],
        )
    }

    fn validator_matches_query(&self, entry: &CodebaseValidatorEntry, query: Option<&str>) -> bool {
        Self::query_matches_any(
            query,
            [
                entry.display_label().into_owned(),
                entry.source_module_path.to_owned(),
                entry.source_type_display.to_owned(),
                entry.docs.unwrap_or_default().to_owned(),
            ],
        )
    }

    fn summary_item_matches_query(&self, item: &SummaryItem, query: Option<&str>) -> bool {
        let Some(machine) = self.current_machine() else {
            return false;
        };
        let (direction, label, peer_machine) = match item {
            SummaryItem::Exact(item) => {
                let peer_machine = match item.direction {
                    SummaryDirection::Outbound => self.doc.machine(item.group.to_machine),
                    SummaryDirection::Inbound => self.doc.machine(item.group.from_machine),
                }
                .expect("summary peer machine should exist");
                let direction = match item.direction {
                    SummaryDirection::Outbound => "outbound exact",
                    SummaryDirection::Inbound => "inbound exact",
                };
                (direction, item.group.display_label(), peer_machine)
            }
            SummaryItem::Heuristic(item) => {
                let peer_machine = match item.direction {
                    SummaryDirection::Outbound => self.doc.machine(item.group.to_machine),
                    SummaryDirection::Inbound => self.doc.machine(item.group.from_machine),
                }
                .expect("summary peer machine should exist");
                let direction = match item.direction {
                    SummaryDirection::Outbound => "outbound heuristic",
                    SummaryDirection::Inbound => "inbound heuristic",
                };
                (direction, item.group.display_label(), peer_machine)
            }
        };

        Self::query_matches_any(
            query,
            [
                direction.to_owned(),
                label,
                render_machine_label(machine).to_owned(),
                render_machine_label(peer_machine).to_owned(),
                machine.description.unwrap_or_default().to_owned(),
                machine.docs.unwrap_or_default().to_owned(),
                peer_machine.description.unwrap_or_default().to_owned(),
                peer_machine.docs.unwrap_or_default().to_owned(),
            ],
        )
    }

    fn relation_matches_query(
        &self,
        detail: &CodebaseRelationDetail<'_>,
        query: Option<&str>,
    ) -> bool {
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
            candidates.push(producer.transition.description.unwrap_or_default().to_owned());
            candidates.push(producer.transition.docs.unwrap_or_default().to_owned());
        }

        Self::query_matches_any(query, candidates)
    }

    fn heuristic_relation_matches_query(
        &self,
        detail: &HeuristicRelationDetail<'_>,
        query: Option<&str>,
    ) -> bool {
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

    fn journey_matches_query(&self, journey: &ResolvedJourney, query: Option<&str>) -> bool {
        if Self::query_matches_any(
            query,
            [
                journey.full_id.clone(),
                journey.display_label().to_owned(),
                journey.docs.unwrap_or_default().to_owned(),
            ],
        ) {
            return true;
        }

        journey
            .nodes
            .iter()
            .any(|node| self.journey_node_matches_query(journey, node, query))
            || journey
                .segments
                .iter()
                .any(|segment| self.journey_segment_matches_query(journey, segment, query))
    }

    fn journey_node_matches_query(
        &self,
        journey: &ResolvedJourney,
        node: &ResolvedJourneyNode,
        query: Option<&str>,
    ) -> bool {
        let mut candidates = vec![
            node.role.display_label().to_owned(),
            self.journey_node_subject_label(journey, node),
        ];

        match &node.reference {
            JourneyNodeReference::Machine { machine } => {
                if let Some(machine) = self.doc.machine(*machine) {
                    candidates.push(render_machine_label(machine).to_owned());
                    candidates.push(machine.rust_type_path.to_owned());
                    candidates.push(machine.description.unwrap_or_default().to_owned());
                    candidates.push(machine.docs.unwrap_or_default().to_owned());
                }
            }
            JourneyNodeReference::State { machine, state } => {
                if let Some((machine, state)) = self
                    .doc
                    .machine(*machine)
                    .and_then(|machine| machine.state(*state).map(|state| (machine, state)))
                {
                    candidates.push(render_machine_label(machine).to_owned());
                    candidates.push(machine.rust_type_path.to_owned());
                    candidates.push(render_state_label(state));
                    candidates.push(state.rust_name.to_owned());
                    candidates.push(state.description.unwrap_or_default().to_owned());
                    candidates.push(state.docs.unwrap_or_default().to_owned());
                }
            }
            JourneyNodeReference::Validator {
                machine,
                entry,
                source_type_display,
            } => {
                candidates.push((*source_type_display).to_owned());
                if let Some((machine, entry)) = self
                    .doc
                    .machine(*machine)
                    .and_then(|machine| machine.validator_entry(*entry).map(|entry| (machine, entry)))
                {
                    candidates.push(render_machine_label(machine).to_owned());
                    candidates.push(machine.rust_type_path.to_owned());
                    candidates.push(entry.display_label().into_owned());
                    candidates.push(entry.docs.unwrap_or_default().to_owned());
                }
            }
            JourneyNodeReference::Bridge {
                type_display,
                resolved_type_name,
                ..
            } => {
                candidates.push((*type_display).to_owned());
                candidates.push((*resolved_type_name).to_owned());
            }
        }

        Self::query_matches_any(query, candidates)
    }

    fn journey_segment_matches_query(
        &self,
        journey: &ResolvedJourney,
        segment: &ResolvedJourneySegment,
        query: Option<&str>,
    ) -> bool {
        let mut candidates = vec![
            segment.visible_kind(self.shows_journey_heuristics()).display_label().to_owned(),
            segment.visible_basis(self.shows_journey_heuristics()).display_label().to_owned(),
            self.journey_segment_label(journey, segment),
        ];
        if let Some(label) = &segment.exact_label {
            candidates.push(label.clone());
        }
        if let Some(label) = &segment.heuristic_label {
            candidates.push(label.clone());
        }
        if let Some(from_node) = journey.nodes.get(segment.from_node) {
            candidates.push(self.journey_node_subject_label(journey, from_node));
        }
        if let Some(to_node) = journey.nodes.get(segment.to_node) {
            candidates.push(self.journey_node_subject_label(journey, to_node));
        }

        Self::query_matches_any(query, candidates)
    }
}

fn titled_block(title: impl Into<Line<'static>>, focused: bool) -> Block<'static> {
    let border_style = if focused {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default()
    };
    Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(border_style)
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

fn journey_detail_text(journey: &ResolvedJourney, shows_heuristic: bool) -> Text<'static> {
    let counts = visible_journey_counts(journey, shows_heuristic);
    let mut lines = vec![
        Line::from(format!("journey: {}", journey.display_label())),
        Line::from(format!("id: {}", journey.full_id)),
        Line::from(format!("nodes: {}", journey.nodes.len())),
        Line::from(format!("segments: {}", journey.segments.len())),
        Line::from(format!(
            "coverage: {} exact, {} declared, {} heuristic, {} missing",
            counts.exact, counts.declared, counts.heuristic, counts.missing
        )),
    ];
    append_description_and_docs(&mut lines, None, journey.docs);
    Text::from(lines)
}

fn journey_node_detail_text(
    journey: &ResolvedJourney,
    node: &ResolvedJourneyNode,
    doc: &CodebaseDoc,
) -> Text<'static> {
    let mut lines = vec![Line::from(format!("journey role: {}", node.role.display_label()))];
    match &node.reference {
        JourneyNodeReference::Machine { machine } => {
            let machine = doc.machine(*machine).expect("journey machine should exist");
            lines.push(Line::from(format!(
                "machine: {}",
                render_machine_label(machine)
            )));
            lines.push(Line::from(format!("path: {}", machine.rust_type_path)));
            append_description_and_docs(&mut lines, machine.description, machine.docs);
        }
        JourneyNodeReference::State { machine, state } => {
            let machine = doc.machine(*machine).expect("journey state machine should exist");
            let state = machine.state(*state).expect("journey state should exist");
            lines.push(Line::from(format!(
                "machine: {}",
                render_machine_label(machine)
            )));
            lines.push(Line::from(format!("state: {}", render_state_label(state))));
            append_named_description_and_docs(
                &mut lines,
                "machine",
                machine.description,
                machine.docs,
            );
            append_named_description_and_docs(&mut lines, "state", state.description, state.docs);
        }
        JourneyNodeReference::Validator {
            machine,
            entry,
            source_type_display: _,
        } => {
            let machine = doc.machine(*machine).expect("journey validator machine should exist");
            let entry = machine
                .validator_entry(*entry)
                .expect("journey validator entry should exist");
            lines.push(Line::from(format!("validator: {}", entry.display_label())));
            lines.push(Line::from(format!(
                "machine: {}",
                render_machine_label(machine)
            )));
            lines.push(Line::from(format!("module: {}", entry.source_module_path)));
            lines.push(Line::from(format!("target states: {:?}", entry.target_states)));
            append_named_description_and_docs(
                &mut lines,
                "machine",
                machine.description,
                machine.docs,
            );
            append_named_description_and_docs(&mut lines, "validator", None, entry.docs);
        }
        JourneyNodeReference::Bridge {
            type_display,
            resolved_type_name,
            declared_reference_target,
        } => {
            lines.push(Line::from(format!("bridge type: {type_display}")));
            lines.push(Line::from(format!("resolved type: {resolved_type_name}")));
            match declared_reference_target {
                Some(target) => {
                    let machine = doc
                        .machine(target.machine)
                        .expect("journey bridge machine should exist");
                    let state = machine
                        .state(target.state)
                        .expect("journey bridge state should exist");
                    lines.push(Line::from("machine_ref: yes"));
                    lines.push(Line::from(format!(
                        "machine_ref target: {} :: {}",
                        render_machine_label(machine),
                        render_state_label(state)
                    )));
                }
                None => lines.push(Line::from("machine_ref: no")),
            }
        }
    }

    if let Some(journey_docs) = journey.docs {
        append_named_description_and_docs(&mut lines, "journey", None, Some(journey_docs));
    }

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

fn journey_segment_detail_text(
    journey: &ResolvedJourney,
    segment: &ResolvedJourneySegment,
    shows_heuristic: bool,
    doc: &CodebaseDoc,
) -> Text<'static> {
    let from = journey
        .nodes
        .get(segment.from_node)
        .expect("journey segment source node should exist");
    let to = journey
        .nodes
        .get(segment.to_node)
        .expect("journey segment target node should exist");
    let mut lines = vec![
        Line::from(format!(
            "kind: {}",
            segment.visible_kind(shows_heuristic).display_label()
        )),
        Line::from(format!(
            "basis: {}",
            segment.visible_basis(shows_heuristic).display_label()
        )),
        Line::from(format!("from: {}", from.role.display_label())),
        Line::from(format!("to: {}", to.role.display_label())),
    ];

    if let Some(machine_index) = segment.from_machine {
        let machine = doc.machine(machine_index).expect("journey source machine");
        lines.push(Line::from(format!(
            "source machine: {}",
            render_machine_label(machine)
        )));
    }
    if let Some(machine_index) = segment.to_machine {
        let machine = doc.machine(machine_index).expect("journey target machine");
        lines.push(Line::from(format!(
            "target machine: {}",
            render_machine_label(machine)
        )));
    }
    if segment.same_machine {
        lines.push(Line::from("same machine segment: yes"));
    }
    if let Some(label) = &segment.exact_label {
        lines.push(Line::from(format!(
            "exact cover: {} ({})",
            segment.exact_count, label
        )));
    }
    if let Some(label) = &segment.heuristic_label {
        lines.push(Line::from(format!(
            "heuristic cover: {} ({})",
            segment.heuristic_count, label
        )));
    }
    if segment.visible_kind(shows_heuristic) == JourneySegmentKind::Missing
        && segment.heuristic_count > 0
    {
        lines.push(Line::from(
            "heuristic cover exists, but the current lane hides it; switch lane mode with `m`.",
        ));
    }
    if segment.visible_kind(shows_heuristic) == JourneySegmentKind::DeclaredBridge {
        lines.push(Line::from(
            "This segment is declared explicitly as a narrative bridge, not inferred from the exact graph.",
        ));
    }

    append_named_description_and_docs(&mut lines, "journey", None, journey.docs);
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

    let mut lines = vec![
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

fn append_composition_suggestions(
    lines: &mut Vec<Line<'static>>,
    suggestions: &[&CompositionSuggestion],
    doc: &CodebaseDoc,
) {
    let (warnings, suggestions_count) = suggestions.iter().fold((0usize, 0usize), |counts, item| {
        match item.severity {
            CompositionSuggestionSeverity::Warning => (counts.0 + 1, counts.1),
            CompositionSuggestionSeverity::Suggestion => (counts.0, counts.1 + 1),
        }
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
        lines.push(Line::from(format!("kind: {}", suggestion.kind.display_label())));
        lines.push(Line::from(format!("why: {}", suggestion.why_text())));
        lines.push(Line::from(format!("evidence: {}", suggestion.counts_label())));
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

#[cfg(test)]
mod tests {
    use super::*;

    use crossterm::event::KeyModifiers;
    use ratatui::backend::TestBackend;
    use std::path::PathBuf;

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

    fn empty_heuristic_overlay() -> HeuristicOverlay {
        HeuristicOverlay::from_parts(HeuristicStatusKind::Available, Vec::new(), Vec::new())
    }

    fn empty_journey_overlay() -> JourneyOverlay {
        JourneyOverlay::default()
    }

    fn empty_suggestion_overlay() -> CompositionSuggestionOverlay {
        CompositionSuggestionOverlay::default()
    }

    fn fixture_journey_overlay(doc: &CodebaseDoc) -> JourneyOverlay {
        crate::journeys::collect_journey_overlay(doc, &empty_heuristic_overlay())
            .expect("journey overlay")
    }

    fn fixture_app(doc: CodebaseDoc, heuristic: HeuristicOverlay) -> InspectorApp {
        let suggestions = crate::suggestions::collect_composition_suggestions(&doc, &heuristic);
        InspectorApp::new(
            doc,
            heuristic,
            suggestions,
            empty_journey_overlay(),
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

    #[test]
    fn app_renders_workspace_machine_and_relation_views() {
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

        assert!(text.contains("machines:"));
        assert!(text.contains("exact summary edges:"));
        assert!(text.contains("Workflow Machine"));
        assert!(text.contains("Task Machine"));
        assert!(text.contains("Machine Workflow Machine [1 exact edge]"));
        assert!(text.contains("Relations [exact] for machine Workflow Machine"));
    }

    #[test]
    fn app_defaults_to_journeys_when_declared_journeys_exist() {
        let doc = fixture_doc();
        let journeys = fixture_journey_overlay(&doc);
        let app = InspectorApp::new(
            doc,
            empty_heuristic_overlay(),
            empty_suggestion_overlay(),
            journeys,
            "/tmp/workspace/Cargo.toml".to_owned(),
        );

        assert_eq!(app.workspace_section, WorkspaceSection::Journeys);
        assert_eq!(app.focus, Focus::Workspace);
        assert_eq!(
            app.current_journey().map(ResolvedJourney::display_label),
            Some("Workflow Story")
        );
    }

    #[test]
    fn journey_detail_pane_shows_bridge_and_segment_explanations() {
        let doc = fixture_doc();
        let journeys = fixture_journey_overlay(&doc);
        let mut app = InspectorApp::new(
            doc,
            empty_heuristic_overlay(),
            empty_suggestion_overlay(),
            journeys,
            "/tmp/workspace/Cargo.toml".to_owned(),
        );

        app.focus = Focus::MainView;
        app.journey_node_index = 2;
        let node_text = text_contents(app.detail_text());
        assert!(node_text.contains("bridge type:"));
        assert!(node_text.contains("machine_ref: yes"));

        app.focus = Focus::BottomView;
        app.relation_index = 1;
        let segment_text = text_contents(app.detail_text());
        assert!(segment_text.contains("kind: declared bridge"));
        assert!(segment_text.contains("basis: declared bridge"));
        assert!(segment_text.contains("declared explicitly as a narrative bridge"));
    }

    #[test]
    fn journey_workspace_detail_respects_lane_visibility() {
        let journey = ResolvedJourney {
            index: 0,
            full_id: "workflow::story".to_owned(),
            module_path: "workflow",
            id: "story",
            label: Some("Story"),
            docs: None,
            nodes: Vec::new(),
            segments: vec![ResolvedJourneySegment {
                index: 0,
                from_node: 0,
                to_node: 0,
                from_machine: None,
                to_machine: None,
                declared_bridge: false,
                same_machine: false,
                exact_is_composition_owned: false,
                exact_label: None,
                exact_count: 0,
                heuristic_label: Some("heuristic refs: body".to_owned()),
                heuristic_count: 1,
            }],
        };

        let exact_text = text_contents(journey_detail_text(&journey, false));
        assert!(exact_text.contains("coverage: 0 exact, 0 declared, 0 heuristic, 1 missing"));

        let mixed_text = text_contents(journey_detail_text(&journey, true));
        assert!(mixed_text.contains("coverage: 0 exact, 0 declared, 1 heuristic, 0 missing"));
    }

    #[test]
    fn journey_segments_prefer_composition_exact_basis() {
        let segment = ResolvedJourneySegment {
            index: 0,
            from_node: 0,
            to_node: 1,
            from_machine: Some(0),
            to_machine: Some(1),
            declared_bridge: false,
            same_machine: false,
            exact_is_composition_owned: true,
            exact_label: Some("composition refs: payload".to_owned()),
            exact_count: 1,
            heuristic_label: None,
            heuristic_count: 0,
        };

        assert_eq!(
            segment.visible_basis(false),
            crate::journeys::JourneySegmentBasis::CompositionMachineRelation
        );
    }

    #[test]
    fn relation_rich_machines_default_to_summary() {
        let doc = fixture_doc();
        let mut app = fixture_app(doc, empty_heuristic_overlay());
        let workflow_index = app
            .doc
            .machines()
            .iter()
            .position(|machine| machine.label == Some("Workflow Machine"))
            .expect("workflow machine should exist");

        app.select_machine(workflow_index);

        assert_eq!(app.machine_section, MachineSection::Summary);
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
        app.focus = Focus::BottomView;
        app.clamp_indices();

        assert_eq!(app.focus, Focus::BottomView);
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
        for ch in "persisted".chars() {
            app.handle_key(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE));
        }
        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        assert_eq!(app.input_mode, InputMode::Normal);
        assert_eq!(app.search_query, "persisted");
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
    fn app_search_matches_relation_targets_and_keeps_source_machine_visible() {
        let doc = fixture_doc();
        let mut app = fixture_app(doc, empty_heuristic_overlay());
        app.search_query = "param".to_owned();
        app.clamp_indices();

        let labels = app
            .visible_machine_indices()
            .into_iter()
            .filter_map(|machine_index| app.doc.machine(machine_index))
            .map(render_machine_label)
            .collect::<Vec<_>>();
        assert_eq!(labels, vec!["Task Machine", "Workflow Machine"]);
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
        app.machine_section = MachineSection::Summary;
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

        app.handle_key(KeyEvent::new(KeyCode::Char('3'), KeyModifiers::NONE));
        assert_eq!(app.summary_items().len(), 1);
        match &app.summary_items()[0] {
            SummaryItem::Exact(item) => {
                assert_eq!(item.group.display_label(), "composition refs: param");
            }
            SummaryItem::Heuristic(_) => panic!("expected exact summary item"),
        }

        app.machine_section = MachineSection::States;
        app.machine_item_index = app
            .machine_items()
            .iter()
            .position(|item| matches!(item, MachineItem::State(1)))
            .expect("in-progress state should exist");
        assert!(app.relation_items().is_empty());

        app.machine_section = MachineSection::Transitions;
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

        app.handle_key(KeyEvent::new(KeyCode::Char('5'), KeyModifiers::NONE));
        assert!(app.summary_items().is_empty());
        assert!(app.relation_items().is_empty());
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
            .into_iter()
            .find(|group| group.from_machine == workflow.index)
            .expect("workflow summary");
        let summary_item = SummaryItem::Exact(ExactSummaryItem {
            direction: SummaryDirection::Outbound,
            group: summary,
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
            empty_journey_overlay(),
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
    fn app_cycles_lane_modes_and_surfaces_heuristic_relations() {
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
        app.handle_key(KeyEvent::new(KeyCode::Char('m'), KeyModifiers::NONE));
        assert_eq!(app.lane_mode, LaneMode::Heuristic);
        let heuristic_items = app.relation_items();
        assert_eq!(heuristic_items.len(), 2);
        assert!(matches!(heuristic_items[0], RelationItem::Heuristic(_)));

        app.handle_key(KeyEvent::new(KeyCode::Char('m'), KeyModifiers::NONE));
        assert_eq!(app.lane_mode, LaneMode::Mixed);
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

        app.machine_section = MachineSection::Summary;
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
        app.machine_section = MachineSection::Transitions;
        app.machine_item_index = 0;
        app.lane_mode = LaneMode::Heuristic;

        assert_eq!(app.relation_items().len(), 2);
        app.handle_key(KeyEvent::new(KeyCode::Char('6'), KeyModifiers::NONE));
        assert_eq!(app.relation_items().len(), 1);
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
        app.machine_section = MachineSection::Transitions;
        app.machine_item_index = 0;
        app.lane_mode = LaneMode::Heuristic;
        app.focus = Focus::BottomView;

        let text = text_contents(app.detail_text());
        assert!(text.contains("exact lane already covers this relationship"));
    }

    #[test]
    fn empty_state_relations_hint_toward_summary_and_transitions() {
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
            .position(|item| matches!(item, MachineItem::State(0)))
            .expect("draft state should exist");
        app.focus = Focus::BottomView;

        let text = text_contents(app.detail_text());
        assert!(text.contains("No exact relations for state Draft [build]."));
        assert!(text.contains("Try Summary to inspect machine-level edges"));
        assert!(text.contains("Try Transitions to inspect transition-parameter and attested-route edges."));
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
        app.machine_section = MachineSection::States;
        app.machine_item_index = app
            .machine_items()
            .iter()
            .position(|item| matches!(item, MachineItem::State(0)))
            .expect("draft state should exist");
        app.focus = Focus::BottomView;

        let text = text_contents(app.detail_text());
        assert!(text.contains("Switch to heuristic or mixed mode with `m`"));
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
        app.focus = Focus::BottomView;

        let text = text_contents(app.detail_text());
        assert!(text.contains("heuristics unavailable"));
        assert!(text.contains("failed to parse source"));
    }
}
