use super::LineModulePath;
use super::module_ranges::{ModuleRange, scan_inline_module_ranges};
use crate::source::scan::tokenize_source;

#[derive(Clone, Debug)]
struct LineCoverage {
    path: String,
    depth: usize,
    close_boundary: bool,
}

pub(super) fn build_line_module_paths(content: &str) -> Option<Vec<LineModulePath>> {
    let line_count = content.lines().count().max(1);
    let tokens = tokenize_source(content);
    let module_ranges = scan_inline_module_ranges(&tokens);
    let mut line_coverages = vec![Vec::new(); line_count];

    for range in module_ranges {
        add_line_range(&mut line_coverages, range);
    }

    Some(
        line_coverages
            .into_iter()
            .map(resolve_line_coverage)
            .collect(),
    )
}

fn add_line_range(line_coverages: &mut [Vec<LineCoverage>], range: ModuleRange) {
    if range.start_line == 0 || range.end_line < range.start_line {
        return;
    }

    for line in range.start_line..=range.end_line {
        if let Some(slot) = line_coverages.get_mut(line - 1) {
            slot.push(LineCoverage {
                path: range.path.clone(),
                depth: range.depth,
                close_boundary: line == range.close_line,
            });
        }
    }
}

fn resolve_line_coverage(coverages: Vec<LineCoverage>) -> LineModulePath {
    if coverages.is_empty() {
        return LineModulePath::Unset;
    }

    let max_depth = coverages
        .iter()
        .map(|coverage| coverage.depth)
        .max()
        .expect("non-empty");

    let mut deepest = coverages
        .iter()
        .filter(|coverage| coverage.depth == max_depth)
        .collect::<Vec<_>>();
    deepest.sort_by(|left, right| left.path.cmp(&right.path));
    deepest.dedup_by(|left, right| left.path == right.path);

    if deepest.len() != 1 {
        return LineModulePath::Ambiguous;
    }

    let deepest = deepest[0];
    let overlaps_other_path = coverages
        .iter()
        .any(|coverage| coverage.path != deepest.path);
    if overlaps_other_path && deepest.close_boundary {
        return LineModulePath::Ambiguous;
    }

    LineModulePath::Exact {
        path: deepest.path.clone(),
        depth: deepest.depth,
    }
}
