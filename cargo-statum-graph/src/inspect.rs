use std::io;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
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
    CodebaseRelationDetail, CodebaseRelationSource, CodebaseState, CodebaseTransition,
    CodebaseValidatorEntry,
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

#[derive(Debug)]
struct InspectorApp {
    doc: CodebaseDoc,
    workspace_label: String,
    selected_machine: usize,
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
            focus: Focus::Machines,
            machine_section: MachineSection::States,
            machine_item_index: 0,
            relation_direction: RelationDirection::Outbound,
            relation_index: 0,
            should_quit: false,
        }
    }

    fn handle_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
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
            Focus::Machines => self.selected_machine = self.selected_machine.saturating_sub(1),
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
                self.selected_machine = self
                    .selected_machine
                    .saturating_add(1)
                    .min(self.doc.machines().len().saturating_sub(1));
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
        if self.doc.machines().is_empty() {
            self.selected_machine = 0;
            self.machine_item_index = 0;
            self.relation_index = 0;
            return;
        }

        self.selected_machine = self.selected_machine.min(self.doc.machines().len() - 1);
        self.machine_item_index = self
            .machine_item_index
            .min(self.machine_items().len().saturating_sub(1));
        self.relation_index = self
            .relation_index
            .min(self.relation_items().len().saturating_sub(1));
    }

    fn current_machine(&self) -> &CodebaseMachine {
        &self.doc.machines()[self.selected_machine]
    }

    fn machine_items(&self) -> Vec<String> {
        let machine = self.current_machine();
        match self.machine_section {
            MachineSection::States => machine.states.iter().map(render_state_label).collect(),
            MachineSection::Transitions => machine
                .transitions
                .iter()
                .map(|transition| render_transition_label(transition).to_owned())
                .collect(),
            MachineSection::Validators => machine
                .validator_entries
                .iter()
                .map(|entry| entry.display_label().into_owned())
                .collect(),
            MachineSection::Summary => self
                .summary_items()
                .into_iter()
                .map(|item| {
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
                })
                .collect(),
        }
    }

    fn summary_items(&self) -> Vec<SummaryItem> {
        self.doc
            .machine_relation_groups()
            .into_iter()
            .filter_map(|group| {
                if group.from_machine == self.current_machine().index
                    && group.from_machine != group.to_machine
                {
                    Some(SummaryItem {
                        direction: SummaryDirection::Outbound,
                        group,
                    })
                } else if group.to_machine == self.current_machine().index
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

    fn relation_subject(&self) -> RelationSubject {
        let machine = self.current_machine();
        match self.machine_section {
            MachineSection::States => machine
                .states
                .get(self.machine_item_index)
                .map(|state| RelationSubject::State {
                    machine: machine.index,
                    state: state.index,
                })
                .unwrap_or(RelationSubject::Machine {
                    machine: machine.index,
                }),
            MachineSection::Transitions => machine
                .transitions
                .get(self.machine_item_index)
                .map(|transition| RelationSubject::Transition {
                    machine: machine.index,
                    transition: transition.index,
                })
                .unwrap_or(RelationSubject::Machine {
                    machine: machine.index,
                }),
            MachineSection::Validators | MachineSection::Summary => RelationSubject::Machine {
                machine: machine.index,
            },
        }
    }

    fn relation_items(&self) -> Vec<&CodebaseRelation> {
        match (self.relation_subject(), self.relation_direction) {
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
        }
    }

    fn selected_relation(&self) -> Option<&CodebaseRelation> {
        self.relation_items().get(self.relation_index).copied()
    }

    fn selected_relation_detail(&self) -> Option<CodebaseRelationDetail<'_>> {
        self.selected_relation()
            .and_then(|relation| self.doc.relation_detail(relation.index))
    }

    fn disconnected_group_count(&self) -> usize {
        if self.doc.machines().is_empty() {
            return 0;
        }

        let mut adjacency = vec![Vec::new(); self.doc.machines().len()];
        for group in self.doc.machine_relation_groups() {
            if group.from_machine == group.to_machine {
                continue;
            }
            adjacency[group.from_machine].push(group.to_machine);
            adjacency[group.to_machine].push(group.from_machine);
        }

        let mut seen = vec![false; self.doc.machines().len()];
        let mut groups = 0;
        for machine in self.doc.machines() {
            if seen[machine.index] {
                continue;
            }
            groups += 1;
            let mut stack = vec![machine.index];
            seen[machine.index] = true;
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
            .constraints([Constraint::Min(0), Constraint::Length(3)])
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
            .constraints([Constraint::Length(4), Constraint::Min(0)])
            .split(inner);

        let counts = Paragraph::new(Text::from(vec![
            Line::from(format!("machines: {}", self.doc.machines().len())),
            Line::from(format!("groups: {}", self.disconnected_group_count())),
            Line::from(format!(
                "summary edges: {}",
                self.doc
                    .machine_relation_groups()
                    .into_iter()
                    .filter(|group| group.from_machine != group.to_machine)
                    .count()
            )),
        ]));
        frame.render_widget(counts, sections[0]);

        let items = self
            .doc
            .machines()
            .iter()
            .map(|machine| ListItem::new(render_machine_label(machine).to_owned()))
            .collect::<Vec<_>>();
        let mut state = ListState::default().with_selected(Some(self.selected_machine));
        let list = List::new(items)
            .highlight_style(Style::default().fg(Color::Black).bg(Color::Cyan))
            .highlight_symbol("> ");
        frame.render_stateful_widget(list, sections[1], &mut state);
    }

    fn render_machine_view(&self, frame: &mut Frame, area: Rect) {
        let block = titled_block(
            format!("Machine {}", render_machine_label(self.current_machine())),
            self.focus == Focus::MachineView,
        );
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

        let items = self
            .machine_items()
            .into_iter()
            .map(ListItem::new)
            .collect::<Vec<_>>();
        let empty = items.is_empty();
        let mut state =
            ListState::default().with_selected((!empty).then_some(self.machine_item_index));
        let list = if empty {
            List::new(vec![ListItem::new("<none>")])
        } else {
            List::new(items)
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
            List::new(vec![ListItem::new("<none>")])
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
        let status = Paragraph::new(Text::from(vec![
            Line::from(vec![
                Span::styled("focus ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(self.focus.label()),
                Span::raw("  "),
                Span::styled("keys ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw("tab shift-tab h/l j/k q"),
            ]),
            Line::from("machine tabs switch with h/l while machine view is focused; relations toggle inbound/outbound with h/l."),
        ]))
        .wrap(Wrap { trim: false });
        frame.render_widget(status, area);
    }

    fn relation_subject_label(&self) -> String {
        match self.relation_subject() {
            RelationSubject::Machine { machine } => {
                let machine = self
                    .doc
                    .machine(machine)
                    .expect("machine subject should exist");
                format!("for machine {}", render_machine_label(machine))
            }
            RelationSubject::State { machine, state } => {
                let machine = self
                    .doc
                    .machine(machine)
                    .expect("state subject machine should exist");
                let state = machine.state(state).expect("state subject should exist");
                format!("for state {}", render_state_label(state))
            }
            RelationSubject::Transition {
                machine,
                transition,
            } => {
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
            Focus::Machines => machine_detail_text(self.current_machine(), &self.doc),
            Focus::MachineView => self.machine_detail_selection_text(),
            Focus::Relations => self
                .selected_relation_detail()
                .map(relation_detail_text)
                .unwrap_or_else(|| Text::from("<no relation selected>")),
        }
    }

    fn machine_detail_selection_text(&self) -> Text<'static> {
        let machine = self.current_machine();
        match self.machine_section {
            MachineSection::States => machine
                .states
                .get(self.machine_item_index)
                .map(state_detail_text)
                .unwrap_or_else(|| machine_detail_text(machine, &self.doc)),
            MachineSection::Transitions => machine
                .transitions
                .get(self.machine_item_index)
                .map(transition_detail_text)
                .unwrap_or_else(|| machine_detail_text(machine, &self.doc)),
            MachineSection::Validators => machine
                .validator_entries
                .get(self.machine_item_index)
                .map(validator_detail_text)
                .unwrap_or_else(|| machine_detail_text(machine, &self.doc)),
            MachineSection::Summary => self
                .summary_items()
                .get(self.machine_item_index)
                .map(|summary| summary_detail_text(summary, &self.doc))
                .unwrap_or_else(|| machine_detail_text(machine, &self.doc)),
        }
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
    Text::from(vec![
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
    ])
}

fn state_detail_text(state: &CodebaseState) -> Text<'static> {
    Text::from(vec![
        Line::from(format!("state: {}", render_state_label(state))),
        Line::from(format!("rust name: {}", state.rust_name)),
        Line::from(format!("has data: {}", yes_no(state.has_data))),
        Line::from(format!(
            "direct construction: {}",
            yes_no(state.direct_construction_available)
        )),
        Line::from(format!("graph root: {}", yes_no(state.is_graph_root))),
    ])
}

fn transition_detail_text(transition: &CodebaseTransition) -> Text<'static> {
    Text::from(vec![
        Line::from(format!(
            "transition: {}",
            render_transition_label(transition)
        )),
        Line::from(format!("method: {}", transition.method_name)),
        Line::from(format!("from state index: {}", transition.from)),
        Line::from(format!("target count: {}", transition.to.len())),
        Line::from(format!("targets: {:?}", transition.to)),
    ])
}

fn validator_detail_text(entry: &CodebaseValidatorEntry) -> Text<'static> {
    Text::from(vec![
        Line::from(format!("validator: {}", entry.display_label())),
        Line::from(format!("module: {}", entry.source_module_path)),
        Line::from(format!("target states: {:?}", entry.target_states)),
    ])
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

    Text::from(vec![
        Line::from(format!("{direction_label} summary edge")),
        Line::from(format!("from: {}", render_machine_label(source_machine))),
        Line::from(format!("to: {}", render_machine_label(target_machine))),
        Line::from(format!("label: {}", summary.group.display_label())),
        Line::from(format!(
            "relation count: {}",
            summary.group.relation_indices.len()
        )),
    ])
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

    Text::from(lines)
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
            Running,
            Done,
        }

        #[machine]
        #[present(label = "Task Machine")]
        pub struct Machine<State> {}

        #[transition]
        impl Machine<Idle> {
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
            InProgress(super::task::Machine<super::task::Running>),
            Done,
        }

        #[machine]
        #[present(label = "Workflow Machine")]
        pub struct Machine<State> {}

        #[transition]
        impl Machine<Draft> {
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
            RelationSubject::State {
                machine: 1,
                state: 1
            }
        );

        let detail = app
            .selected_relation_detail()
            .expect("selected relation detail should exist");
        assert_eq!(detail.source_machine.label, Some("Workflow Machine"));
        assert_eq!(detail.target_machine.label, Some("Task Machine"));
    }
}
