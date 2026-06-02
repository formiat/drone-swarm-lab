/// Export a ComparisonReport as a markdown table fragment.
pub fn export_markdown(report: &crate::ComparisonReport) -> String {
    format!("{}", report)
}
