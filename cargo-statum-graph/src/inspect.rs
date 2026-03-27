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
    CodebaseDoc, CodebaseMachine, CodebaseMachineRelationGroup, CodebaseRelation,
    CodebaseRelationBasis, CodebaseRelationCount, CodebaseRelationDetail, CodebaseRelationKind,
    CodebaseRelationSource, CodebaseState, CodebaseTransition, CodebaseValidatorEntry,
};

pub fn run(doc: CodebaseDoc, workspace_label: String) -> io::Result<()> {
    let mut terminal = setup_terminal()?;
    let mut app = InspectorApp::new(doc, workspace_label);

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
    Machines,
    MachineView,
    Relations,
}

impl Focus {
    fn label(self) -> &'static str {
        match self {
            Self::Machines => "machines",
            Self::MachineView => "machine",
            Self::Relations => "relations",
        }
    }

    fn next(self) -> Self {
        match self {
            Self::Machines => Self::MachineView,
            Self::MachineView => Self::Relations,
            Self::Relations => Self::Machines,
        }
    }

    fn previous(self) -> Self {
        match self {
            Self::Machines => Self::Relations,
            Self::MachineView => Self::Machines,
            Self::Relations => Self::MachineView,
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
struct SummaryItem {
    direction: SummaryDirection,
    group: CodebaseMachineRelationGroup,
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

#[derive(Debug)]
struct InspectorApp {
    doc: CodebaseDoc,
    workspace_label: String,
    selected_machine: usize,
    input_mode: InputMode,
    search_query: String,
    filters: ExactFilters,
    focus: Focus,
    machine_section: MachineSection,
    machine_item_index: usize,
    relation_direction: RelationDirection,
    relation_index: usize,
    should_quit: bool,
}

impl InspectorApp {
    fn new(doc: CodebaseDoc, workspace_label: String) -> Self {
        Self {
            doc,
            workspace_label,
            selected_machine: 0,
            input_mode: InputMode::Normal,
            search_query: String::new(),
            filters: ExactFilters::default(),
            focus: Focus::Machines,
            machine_section: MachineSection::States,
            machine_item_index: 0,
            relation_direction: RelationDirection::Outbound,
            relation_index: 0,
            should_quit: false,
        }
    }

    fn handle_key(&mut self, key: KeyEvent) {
        if self.input_mode == InputMode::Search {
            self.handle_search_key(key);
            self.clamp_indices();
            return;
        }

        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
            KeyCode::Char('/') => self.input_mode = InputMode::Search,
            KeyCode::Char('0') => self.filters.clear(),
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
            Focus::Machines => self.focus = self.focus.previous(),
            Focus::MachineView => self.machine_section = self.machine_section.previous(),
            Focus::Relations => self.relation_direction = self.relation_direction.toggle(),
        }
    }

    fn move_right(&mut self) {
        match self.focus {
            Focus::Machines => self.focus = self.focus.next(),
            Focus::MachineView => self.machine_section = self.machine_section.next(),
            Focus::Relations => self.relation_direction = self.relation_direction.toggle(),
        }
    }

    fn move_up(&mut self) {
        match self.focus {
            Focus::Machines => {
                let visible = self.visible_machine_indices();
                let Some(current_position) = visible
                    .iter()
                    .position(|machine_index| *machine_index == self.selected_machine)
                else {
                    if let Some(&first) = visible.first() {
                        self.selected_machine = first;
                    }
                    return;
                };
                if current_position > 0 {
                    self.selected_machine = visible[current_position - 1];
                }
            }
            Focus::MachineView => {
                self.machine_item_index = self.machine_item_index.saturating_sub(1);
            }
            Focus::Relations => {
                self.relation_index = self.relation_index.saturating_sub(1);
            }
        }
    }

    fn move_down(&mut self) {
        match self.focus {
            Focus::Machines => {
                let visible = self.visible_machine_indices();
                let Some(current_position) = visible
                    .iter()
                    .position(|machine_index| *machine_index == self.selected_machine)
                else {
                    if let Some(&first) = visible.first() {
                        self.selected_machine = first;
                    }
                    return;
                };
                if let Some(&next) = visible.get(current_position + 1) {
                    self.selected_machine = next;
                }
            }
            Focus::MachineView => {
                self.machine_item_index = self.machine_item_index.saturating_add(1);
            }
            Focus::Relations => {
                self.relation_index = self.relation_index.saturating_add(1);
            }
        }
    }

    fn clamp_indices(&mut self) {
        let visible_machines = self.visible_machine_indices();
        if visible_machines.is_empty() {
            self.selected_machine = 0;
            self.machine_item_index = 0;
            self.relation_index = 0;
            return;
        }

        if !visible_machines.contains(&self.selected_machine) {
            self.selected_machine = visible_machines[0];
        }
        self.machine_item_index = self
            .machine_item_index
            .min(self.machine_items().len().saturating_sub(1));
        self.relation_index = self
            .relation_index
            .min(self.relation_items().len().saturating_sub(1));
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
            MachineItem::Summary(item) => {
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
                    "{arrow} {} : {}",
                    render_machine_label(peer_machine),
                    item.group.display_label()
                )
            }
        }
    }

    fn summary_items(&self) -> Vec<SummaryItem> {
        let Some(machine) = self.current_machine() else {
            return Vec::new();
        };
        self.filtered_machine_relation_groups()
            .into_iter()
            .filter_map(|group| {
                if group.from_machine == machine.index && group.from_machine != group.to_machine {
                    Some(SummaryItem {
                        direction: SummaryDirection::Outbound,
                        group,
                    })
                } else if group.to_machine == machine.index
                    && group.from_machine != group.to_machine
                {
                    Some(SummaryItem {
                        direction: SummaryDirection::Inbound,
                        group,
                    })
                } else {
                    None
                }
            })
            .collect()
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

    fn relation_items(&self) -> Vec<&CodebaseRelation> {
        let query = self.normalized_query();
        let Some(subject) = self.relation_subject() else {
            return Vec::new();
        };

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
            .filter(|relation| {
                self.doc
                    .relation_detail(relation.index)
                    .is_some_and(|detail| self.relation_matches_query(&detail, query.as_deref()))
            })
            .collect()
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
            for relation_index in &relation_indices {
                let relation = self
                    .doc
                    .relation(*relation_index)
                    .expect("filtered relation index should resolve");
                *counts.entry((relation.kind, relation.basis)).or_default() += 1;
            }

            filtered.push(CodebaseMachineRelationGroup {
                index: filtered.len(),
                from_machine: group.from_machine,
                to_machine: group.to_machine,
                relation_indices,
                counts: counts
                    .into_iter()
                    .map(|((kind, basis), count)| CodebaseRelationCount { kind, basis, count })
                    .collect(),
            });
        }
        filtered
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

    fn selected_relation(&self) -> Option<&CodebaseRelation> {
        self.relation_items().get(self.relation_index).copied()
    }

    fn selected_relation_detail(&self) -> Option<CodebaseRelationDetail<'_>> {
        self.selected_relation()
            .and_then(|relation| self.doc.relation_detail(relation.index))
    }

    fn disconnected_group_count(&self) -> usize {
        let visible_machine_indices = self.visible_machine_indices();
        if visible_machine_indices.is_empty() {
            return 0;
        }

        let mut adjacency = vec![Vec::new(); self.doc.machines().len()];
        for group in self.visible_machine_relation_groups() {
            if group.from_machine == group.to_machine {
                continue;
            }
            adjacency[group.from_machine].push(group.to_machine);
            adjacency[group.to_machine].push(group.from_machine);
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
            .constraints([Constraint::Min(0), Constraint::Length(4)])
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
        self.render_machine_view(frame, center[0]);
        self.render_relations(frame, center[1]);
        self.render_detail(frame, horizontal[2]);
        self.render_status(frame, vertical[1]);
    }

    fn render_workspace(&self, frame: &mut Frame, area: Rect) {
        let block = titled_block(
            format!("Workspace {}", self.workspace_label),
            self.focus == Focus::Machines,
        );
        let inner = block.inner(area);
        frame.render_widget(block, area);

        let sections = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(6), Constraint::Min(0)])
            .split(inner);

        let visible_machine_indices = self.visible_machine_indices();
        let visible_machine_count = visible_machine_indices.len();
        let total_machine_count = self.doc.machines().len();
        let visible_summary_edges = self
            .visible_machine_relation_groups()
            .into_iter()
            .filter(|group| group.from_machine != group.to_machine)
            .count();
        let search_status = if self.has_search_query() {
            format!("/{}", self.search_query.trim())
        } else {
            "<none>".to_owned()
        };

        let counts = Paragraph::new(Text::from(vec![
            Line::from(format!(
                "machines: {visible_machine_count}/{total_machine_count}"
            )),
            Line::from(format!("groups: {}", self.disconnected_group_count())),
            Line::from(format!("summary edges: {visible_summary_edges}")),
            Line::from(format!("search: {search_status}")),
            Line::from(format!(
                "filters: kind={} basis={}",
                self.filters.kind_summary(),
                self.filters.basis_summary()
            )),
        ]));
        frame.render_widget(counts, sections[0]);

        let items = visible_machine_indices
            .iter()
            .filter_map(|machine_index| self.doc.machine(*machine_index))
            .map(|machine| ListItem::new(render_machine_label(machine).to_owned()))
            .collect::<Vec<_>>();
        let selected = visible_machine_indices
            .iter()
            .position(|machine_index| *machine_index == self.selected_machine);
        let mut state = ListState::default().with_selected(selected);
        let list = if items.is_empty() {
            List::new(vec![ListItem::new("<no matches>")])
        } else {
            List::new(items)
        }
        .highlight_style(Style::default().fg(Color::Black).bg(Color::Cyan))
        .highlight_symbol("> ");
        frame.render_stateful_widget(list, sections[1], &mut state);
    }

    fn render_machine_view(&self, frame: &mut Frame, area: Rect) {
        let title = self
            .current_machine()
            .map(|machine| format!("Machine {}", render_machine_label(machine)))
            .unwrap_or_else(|| "Machine <no matches>".to_owned());
        let block = titled_block(title, self.focus == Focus::MachineView);
        let inner = block.inner(area);
        frame.render_widget(block, area);

        let sections = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(0)])
            .split(inner);
        let tabs = Tabs::new(
            MachineSection::ORDER
                .into_iter()
                .map(|section| Line::from(section.label()))
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
            format!("Relations {}", subject_label),
            self.focus == Focus::Relations,
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
                let detail = self
                    .doc
                    .relation_detail(relation.index)
                    .expect("relation detail should resolve");
                ListItem::new(render_relation_label(&detail))
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
            "tab shift-tab h/l j/k / q 1 payload 2 field 3 param 4 direct 5 ref 0 clear"
        };
        let status = Paragraph::new(Text::from(vec![
            Line::from(vec![
                Span::styled("focus ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(self.focus.label()),
                Span::raw("  "),
                Span::styled("mode ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(self.input_mode.label()),
            ]),
            Line::from(format!("search {search_status}")),
            Line::from(format!(
                "relation filters kind={} basis={}",
                self.filters.kind_summary(),
                self.filters.basis_summary()
            )),
            Line::from(key_help),
        ]))
        .wrap(Wrap { trim: false });
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
            Focus::Machines => self
                .current_machine()
                .map(|machine| machine_detail_text(machine, &self.doc))
                .unwrap_or_else(|| Text::from("<no matches>")),
            Focus::MachineView => self.machine_detail_selection_text(),
            Focus::Relations => self
                .selected_relation_detail()
                .map(relation_detail_text)
                .unwrap_or_else(|| Text::from(self.empty_list_label())),
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
                    .unwrap_or_else(|| machine_detail_text(machine, &self.doc)),
                _ => machine_detail_text(machine, &self.doc),
            },
            MachineSection::Transitions => match self.selected_machine_item() {
                Some(MachineItem::Transition(transition_index)) => machine
                    .transition(transition_index)
                    .map(transition_detail_text)
                    .unwrap_or_else(|| machine_detail_text(machine, &self.doc)),
                _ => machine_detail_text(machine, &self.doc),
            },
            MachineSection::Validators => match self.selected_machine_item() {
                Some(MachineItem::Validator(entry_index)) => machine
                    .validator_entry(entry_index)
                    .map(validator_detail_text)
                    .unwrap_or_else(|| machine_detail_text(machine, &self.doc)),
                _ => machine_detail_text(machine, &self.doc),
            },
            MachineSection::Summary => match self.selected_machine_item() {
                Some(MachineItem::Summary(summary)) => summary_detail_text(&summary, &self.doc),
                _ => machine_detail_text(machine, &self.doc),
            },
        }
    }

    fn has_search_query(&self) -> bool {
        !self.search_query.trim().is_empty()
    }

    fn normalized_query(&self) -> Option<String> {
        let trimmed = self.search_query.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_ascii_lowercase())
    }

    fn empty_list_label(&self) -> &'static str {
        if self.has_search_query() || self.filters.has_active() {
            "<no matches>"
        } else {
            "<none>"
        }
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
            || self.machine_relations_match_query(machine.index, query)
    }

    fn machine_relations_match_query(&self, machine_index: usize, query: Option<&str>) -> bool {
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
        let peer_machine = match item.direction {
            SummaryDirection::Outbound => self.doc.machine(item.group.to_machine),
            SummaryDirection::Inbound => self.doc.machine(item.group.from_machine),
        }
        .expect("summary peer machine should exist");
        let direction = match item.direction {
            SummaryDirection::Outbound => "outbound",
            SummaryDirection::Inbound => "inbound",
        };

        Self::query_matches_any(
            query,
            [
                direction.to_owned(),
                item.group.display_label(),
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

fn render_relation_label(detail: &CodebaseRelationDetail<'_>) -> String {
    format!(
        "{} ({}) -> {} :: {}",
        detail.relation.kind.display_label(),
        detail.relation.basis.display_label(),
        render_machine_label(detail.target_machine),
        render_state_label(detail.target_state)
    )
}

fn machine_detail_text(machine: &CodebaseMachine, doc: &CodebaseDoc) -> Text<'static> {
    let mut lines = vec![
        Line::from(format!("machine: {}", render_machine_label(machine))),
        Line::from(format!("path: {}", machine.rust_type_path)),
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

fn summary_detail_text(summary: &SummaryItem, doc: &CodebaseDoc) -> Text<'static> {
    let (source_machine, target_machine) = match summary.direction {
        SummaryDirection::Outbound => (summary.group.from_machine, summary.group.to_machine),
        SummaryDirection::Inbound => (summary.group.from_machine, summary.group.to_machine),
    };
    let source_machine = doc
        .machine(source_machine)
        .expect("summary source machine should exist");
    let target_machine = doc
        .machine(target_machine)
        .expect("summary target machine should exist");
    let direction_label = match summary.direction {
        SummaryDirection::Outbound => "outbound",
        SummaryDirection::Inbound => "inbound",
    };

    let mut lines = vec![
        Line::from(format!("{direction_label} summary edge")),
        Line::from(format!("from: {}", render_machine_label(source_machine))),
        Line::from(format!("to: {}", render_machine_label(target_machine))),
        Line::from(format!("label: {}", summary.group.display_label())),
        Line::from(format!(
            "relation count: {}",
            summary.group.relation_indices.len()
        )),
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
            "source machine: {}",
            render_machine_label(detail.source_machine)
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
        "target state: {}",
        render_state_label(detail.target_state)
    )));
    if let Some(reference_type) = detail.relation.declared_reference_type {
        lines.push(Line::from(format!("declared ref type: {reference_type}")));
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

    Text::from(lines)
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
        #[machine]
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
        let mut app = InspectorApp::new(doc, "/tmp/workspace/Cargo.toml".to_owned());
        app.selected_machine = app
            .doc
            .machines()
            .iter()
            .position(|machine| machine.label == Some("Workflow Machine"))
            .expect("workflow machine should exist");
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

        assert!(text.contains("machines: 2"));
        assert!(text.contains("summary edges: 1"));
        assert!(text.contains("Workflow Machine"));
        assert!(text.contains("Task Machine"));
        assert!(text.contains("Draft [build]"));
        assert!(text.contains("Relations for state Draft [build]"));
    }

    #[test]
    fn app_navigation_reaches_relations_and_details() {
        let doc = fixture_doc();
        let mut app = InspectorApp::new(doc, "/tmp/workspace/Cargo.toml".to_owned());

        app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        app.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        app.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));

        assert_eq!(app.focus, Focus::Relations);
        assert_eq!(app.relation_direction, RelationDirection::Outbound);
        assert_eq!(
            app.relation_subject(),
            Some(RelationSubject::State {
                machine: 1,
                state: 1
            })
        );

        let detail = app
            .selected_relation_detail()
            .expect("selected relation detail should exist");
        assert_eq!(detail.source_machine.label, Some("Workflow Machine"));
        assert_eq!(detail.target_machine.label, Some("Task Machine"));
    }

    #[test]
    fn app_search_filters_visible_machines_and_machine_items() {
        let doc = fixture_doc();
        let mut app = InspectorApp::new(doc, "/tmp/workspace/Cargo.toml".to_owned());

        app.handle_key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE));
        assert_eq!(app.input_mode, InputMode::Search);
        for ch in "persisted".chars() {
            app.handle_key(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE));
        }
        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        assert_eq!(app.input_mode, InputMode::Normal);
        assert_eq!(app.search_query, "persisted");
        assert_eq!(app.visible_machine_indices(), vec![1]);
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
        let mut app = InspectorApp::new(doc, "/tmp/workspace/Cargo.toml".to_owned());
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
        let mut app = InspectorApp::new(doc, "/tmp/workspace/Cargo.toml".to_owned());
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
        let mut app = InspectorApp::new(doc, "/tmp/workspace/Cargo.toml".to_owned());
        app.selected_machine = app
            .doc
            .machines()
            .iter()
            .position(|machine| machine.label == Some("Workflow Machine"))
            .expect("workflow machine should exist");
        app.machine_section = MachineSection::Summary;
        app.clamp_indices();

        assert_eq!(app.summary_items().len(), 1);
        assert_eq!(
            app.summary_items()[0].group.display_label(),
            "exact refs: payload, param"
        );

        app.handle_key(KeyEvent::new(KeyCode::Char('3'), KeyModifiers::NONE));
        assert_eq!(app.summary_items().len(), 1);
        assert_eq!(
            app.summary_items()[0].group.display_label(),
            "exact refs: param"
        );

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
        assert_eq!(relation.kind, CodebaseRelationKind::TransitionParam);

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
        let summary_item = SummaryItem {
            direction: SummaryDirection::Outbound,
            group: summary,
        };

        let machine_detail = text_contents(machine_detail_text(workflow, &doc));
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
        assert!(relation_text.contains("source machine Description"));
        assert!(relation_text.contains("source machine Docs"));
        assert!(relation_text.contains("target machine Description"));
        assert!(relation_text.contains("target machine Docs"));

        let summary_text = text_contents(summary_detail_text(&summary_item, &doc));
        assert!(summary_text.contains("source machine Description"));
        assert!(summary_text.contains("source machine Docs"));
        assert!(summary_text.contains("target machine Description"));
        assert!(summary_text.contains("target machine Docs"));
    }
}
