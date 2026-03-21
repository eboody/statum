pub(crate) fn tracked_file_matches(tracked_file_path: Option<&str>, file_path: &str) -> bool {
    tracked_file_path == Some(file_path)
}
