//! Pure deterministic formatters for `text`, `markdown`, and hand-written RFC 8259 `json`.

use std::fmt::Write as _;

use phonengine::{DerivationStep, StageTrace, Word};

use super::args::OutputFormat;

/// Formatted row for the `run` command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunRow {
    /// Formatted input word.
    pub input: String,
    /// Formatted final output word.
    pub output: String,
    /// Ordered rule IDs that modified the word.
    pub rules_fired: Vec<String>,
}

/// Formatted single non-vacuous step inside a `trace` output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepRow {
    /// Rule identifier.
    pub rule_id: String,
    /// Human-readable rule name.
    pub rule_name: String,
    /// Formatted word immediately before the rule applied.
    pub before: String,
    /// Formatted word immediately after the rule applied.
    pub after: String,
}

/// Formatted per-stage trace inside a staged `trace` output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StageTraceRow {
    /// Stage identifier.
    pub stage_id: String,
    /// Human-readable stage name.
    pub stage_name: String,
    /// Formatted word entering this stage.
    pub input: String,
    /// Formatted word leaving this stage.
    pub output: String,
    /// Non-vacuous rule steps executed within this stage.
    pub steps: Vec<StepRow>,
}

/// Body of a `trace` result: flat steps or chronological stages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TraceBody {
    /// Flat rule program steps.
    Flat(Vec<StepRow>),
    /// Root-to-target stage traces in chronological order.
    Staged(Vec<StageTraceRow>),
}

/// Formatted row for the `trace` command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceRow {
    /// Formatted initial input word.
    pub input: String,
    /// Formatted final output word.
    pub output: String,
    /// Trace breakdown (flat steps or chronological stages).
    pub body: TraceBody,
}

/// Formatted row for the `compare` command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompareRow {
    /// Formatted initial input word.
    pub input: String,
    /// Outcome words aligned with the selected `stages` order.
    pub outcomes: Vec<String>,
}

/// Formats a [`Word`] using either `--spaced` or compact notation.
#[must_use]
pub fn format_word(word: &Word, spaced: bool) -> String {
    if spaced {
        word.to_spaced()
    } else {
        word.to_compact()
    }
}

/// Converts raw [`DerivationStep`] slices into formatted [`StepRow`]s.
#[must_use]
pub fn build_step_rows(steps: &[DerivationStep], spaced: bool) -> Vec<StepRow> {
    steps
        .iter()
        .map(|step| StepRow {
            rule_id: step.rule_id().to_owned(),
            rule_name: step.rule_name().to_owned(),
            before: format_word(step.word_before(), spaced),
            after: format_word(step.word_after(), spaced),
        })
        .collect()
}

/// Converts chronological [`StageTrace`] slices into formatted [`StageTraceRow`]s.
#[must_use]
pub fn build_stage_rows(stages: &[StageTrace], spaced: bool) -> Vec<StageTraceRow> {
    stages
        .iter()
        .map(|stage| StageTraceRow {
            stage_id: stage.stage_id().to_owned(),
            stage_name: stage.stage_name().to_owned(),
            input: format_word(stage.trace().initial(), spaced),
            output: format_word(stage.final_word(), spaced),
            steps: build_step_rows(stage.trace().steps(), spaced),
        })
        .collect()
}

/// Formats `run` results in the requested [`OutputFormat`].
#[must_use]
pub fn format_run(stage: Option<&str>, rows: &[RunRow], format: OutputFormat) -> String {
    match format {
        OutputFormat::Text => format_run_text(rows),
        OutputFormat::Markdown => format_run_markdown(rows),
        OutputFormat::Json => format_run_json(stage, rows),
    }
}

/// Formats `trace` results in the requested [`OutputFormat`].
#[must_use]
pub fn format_trace(stage: Option<&str>, rows: &[TraceRow], format: OutputFormat) -> String {
    match format {
        OutputFormat::Text => format_trace_text(stage, rows),
        OutputFormat::Markdown => format_trace_markdown(stage, rows),
        OutputFormat::Json => format_trace_json(stage, rows),
    }
}

/// Formats `compare` results in the requested [`OutputFormat`].
#[must_use]
pub fn format_compare(stages: &[String], rows: &[CompareRow], format: OutputFormat) -> String {
    match format {
        OutputFormat::Text => format_compare_text(stages, rows),
        OutputFormat::Markdown => format_compare_markdown(stages, rows),
        OutputFormat::Json => format_compare_json(stages, rows),
    }
}

fn format_run_text(rows: &[RunRow]) -> String {
    let mut out = String::new();
    for row in rows {
        let _ = writeln!(out, "{} -> {}", row.input, row.output);
    }
    out
}

fn format_run_markdown(rows: &[RunRow]) -> String {
    let mut out = String::from("| Input | Output |\n| --- | --- |\n");
    for row in rows {
        let _ = writeln!(
            out,
            "| {} | {} |",
            escape_markdown_cell(&row.input),
            escape_markdown_cell(&row.output)
        );
    }
    out
}

fn format_trace_text(stage: Option<&str>, rows: &[TraceRow]) -> String {
    let mut out = String::new();
    for (idx, row) in rows.iter().enumerate() {
        if idx > 0 {
            out.push('\n');
        }
        match (&row.body, stage) {
            (TraceBody::Flat(steps), _) => {
                let _ = writeln!(out, "{} -> {}", row.input, row.output);
                if steps.is_empty() {
                    out.push_str("  (no changes)\n");
                } else {
                    for step in steps {
                        let _ = writeln!(
                            out,
                            "  [{}] {}: {} -> {}",
                            step.rule_id, step.rule_name, step.before, step.after
                        );
                    }
                }
            }
            (TraceBody::Staged(stages), Some(target_stage)) => {
                let _ = writeln!(
                    out,
                    "{} -> {} (stage: {target_stage})",
                    row.input, row.output
                );
                for st in stages {
                    let _ = writeln!(
                        out,
                        "  stage {} ({}): {} -> {}",
                        st.stage_id, st.stage_name, st.input, st.output
                    );
                    if st.steps.is_empty() {
                        out.push_str("    (no changes)\n");
                    } else {
                        for step in &st.steps {
                            let _ = writeln!(
                                out,
                                "    [{}] {}: {} -> {}",
                                step.rule_id, step.rule_name, step.before, step.after
                            );
                        }
                    }
                }
            }
            (TraceBody::Staged(_), None) => {
                let _ = writeln!(out, "{} -> {}", row.input, row.output);
            }
        }
    }
    out
}

fn format_trace_markdown(stage: Option<&str>, rows: &[TraceRow]) -> String {
    let mut out = String::new();
    for (idx, row) in rows.iter().enumerate() {
        if idx > 0 {
            out.push('\n');
        }
        match (&row.body, stage) {
            (TraceBody::Flat(steps), _) => {
                let _ = writeln!(out, "## {} -> {}\n", row.input, row.output);
                append_markdown_steps(&mut out, steps);
            }
            (TraceBody::Staged(stages), Some(target_stage)) => {
                let _ = writeln!(
                    out,
                    "## {} -> {} (stage: {target_stage})",
                    row.input, row.output
                );
                for st in stages {
                    let _ = writeln!(
                        out,
                        "\n### {} — {} ({} -> {})\n",
                        st.stage_id, st.stage_name, st.input, st.output
                    );
                    append_markdown_steps(&mut out, &st.steps);
                }
            }
            (TraceBody::Staged(_), None) => {
                let _ = writeln!(out, "## {} -> {}\n", row.input, row.output);
            }
        }
    }
    out
}

fn append_markdown_steps(out: &mut String, steps: &[StepRow]) {
    if steps.is_empty() {
        out.push_str("_No changes._\n");
        return;
    }
    out.push_str("| Rule ID | Rule | Before | After |\n| --- | --- | --- | --- |\n");
    for step in steps {
        let _ = writeln!(
            out,
            "| {} | {} | {} | {} |",
            escape_markdown_cell(&step.rule_id),
            escape_markdown_cell(&step.rule_name),
            escape_markdown_cell(&step.before),
            escape_markdown_cell(&step.after)
        );
    }
}

fn format_compare_text(stages: &[String], rows: &[CompareRow]) -> String {
    if rows.is_empty() {
        return String::new();
    }

    let mut widths = Vec::with_capacity(1 + stages.len());
    widths.push(
        rows.iter()
            .map(|r| r.input.chars().count())
            .max()
            .unwrap_or(0)
            .max("Input".chars().count())
            .max(3),
    );
    for (idx, stage_id) in stages.iter().enumerate() {
        let max_cell = rows
            .iter()
            .filter_map(|r| r.outcomes.get(idx))
            .map(|s| s.chars().count())
            .max()
            .unwrap_or(0);
        widths.push(max_cell.max(stage_id.chars().count()).max(3));
    }

    let mut out = String::new();
    let mut headers = Vec::with_capacity(1 + stages.len());
    headers.push("Input");
    for stage_id in stages {
        headers.push(stage_id.as_str());
    }
    append_aligned_row(&mut out, &headers, &widths);

    let separators: Vec<String> = widths.iter().map(|w| "-".repeat(*w)).collect();
    let sep_refs: Vec<&str> = separators.iter().map(String::as_str).collect();
    append_aligned_row(&mut out, &sep_refs, &widths);

    for row in rows {
        let mut cells = Vec::with_capacity(1 + stages.len());
        cells.push(row.input.as_str());
        for outcome in &row.outcomes {
            cells.push(outcome.as_str());
        }
        append_aligned_row(&mut out, &cells, &widths);
    }
    out
}

fn append_aligned_row(out: &mut String, cells: &[&str], widths: &[usize]) {
    for (idx, (cell, width)) in cells.iter().zip(widths.iter()).enumerate() {
        if idx > 0 {
            out.push_str("  ");
        }
        out.push_str(cell);
        if idx + 1 < cells.len() {
            let char_len = cell.chars().count();
            for _ in char_len..*width {
                out.push(' ');
            }
        }
    }
    out.push('\n');
}

fn format_compare_markdown(stages: &[String], rows: &[CompareRow]) -> String {
    let mut out = String::from("| Input");
    for stage_id in stages {
        out.push_str(" | ");
        out.push_str(&escape_markdown_cell(stage_id));
    }
    out.push_str(" |\n| ---");
    for _ in stages {
        out.push_str(" | ---");
    }
    out.push_str(" |\n");

    for row in rows {
        out.push_str("| ");
        out.push_str(&escape_markdown_cell(&row.input));
        for outcome in &row.outcomes {
            out.push_str(" | ");
            out.push_str(&escape_markdown_cell(outcome));
        }
        out.push_str(" |\n");
    }
    out
}

fn escape_markdown_cell(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '|' => escaped.push_str("\\|"),
            '\n' | '\r' => escaped.push(' '),
            _ => escaped.push(ch),
        }
    }
    escaped
}

fn format_run_json(stage: Option<&str>, rows: &[RunRow]) -> String {
    let mut out = String::from("{\n");
    push_indent(&mut out, 1);
    out.push_str("\"command\": \"run\",\n");
    push_indent(&mut out, 1);
    out.push_str("\"stage\": ");
    push_json_opt_str(&mut out, stage);
    out.push_str(",\n");
    push_indent(&mut out, 1);
    if rows.is_empty() {
        out.push_str("\"results\": []\n}\n");
        return out;
    }
    out.push_str("\"results\": [\n");
    for (idx, row) in rows.iter().enumerate() {
        push_indent(&mut out, 2);
        out.push_str("{\n");
        push_indent(&mut out, 3);
        out.push_str("\"input\": ");
        push_json_str(&mut out, &row.input);
        out.push_str(",\n");
        push_indent(&mut out, 3);
        out.push_str("\"output\": ");
        push_json_str(&mut out, &row.output);
        out.push_str(",\n");
        push_indent(&mut out, 3);
        out.push_str("\"rules_fired\": ");
        push_json_inline_str_array(&mut out, &row.rules_fired);
        out.push('\n');
        push_indent(&mut out, 2);
        out.push('}');
        if idx + 1 < rows.len() {
            out.push(',');
        }
        out.push('\n');
    }
    push_indent(&mut out, 1);
    out.push_str("]\n}\n");
    out
}

fn format_trace_json(stage: Option<&str>, rows: &[TraceRow]) -> String {
    let mut out = String::from("{\n");
    push_indent(&mut out, 1);
    out.push_str("\"command\": \"trace\",\n");
    push_indent(&mut out, 1);
    out.push_str("\"stage\": ");
    push_json_opt_str(&mut out, stage);
    out.push_str(",\n");
    push_indent(&mut out, 1);
    if rows.is_empty() {
        out.push_str("\"results\": []\n}\n");
        return out;
    }
    out.push_str("\"results\": [\n");
    for (idx, row) in rows.iter().enumerate() {
        push_indent(&mut out, 2);
        out.push_str("{\n");
        push_indent(&mut out, 3);
        out.push_str("\"input\": ");
        push_json_str(&mut out, &row.input);
        out.push_str(",\n");
        push_indent(&mut out, 3);
        out.push_str("\"output\": ");
        push_json_str(&mut out, &row.output);
        out.push_str(",\n");
        match &row.body {
            TraceBody::Flat(steps) => {
                push_indent(&mut out, 3);
                out.push_str("\"steps\": ");
                push_json_steps(&mut out, steps, 3);
                out.push('\n');
            }
            TraceBody::Staged(stages) => {
                push_indent(&mut out, 3);
                out.push_str("\"stages\": ");
                push_json_stages(&mut out, stages, 3);
                out.push('\n');
            }
        }
        push_indent(&mut out, 2);
        out.push('}');
        if idx + 1 < rows.len() {
            out.push(',');
        }
        out.push('\n');
    }
    push_indent(&mut out, 1);
    out.push_str("]\n}\n");
    out
}

fn push_json_stages(out: &mut String, stages: &[StageTraceRow], level: usize) {
    if stages.is_empty() {
        out.push_str("[]");
        return;
    }
    out.push_str("[\n");
    for (idx, st) in stages.iter().enumerate() {
        push_indent(out, level + 1);
        out.push_str("{\n");
        push_indent(out, level + 2);
        out.push_str("\"stage_id\": ");
        push_json_str(out, &st.stage_id);
        out.push_str(",\n");
        push_indent(out, level + 2);
        out.push_str("\"stage_name\": ");
        push_json_str(out, &st.stage_name);
        out.push_str(",\n");
        push_indent(out, level + 2);
        out.push_str("\"input\": ");
        push_json_str(out, &st.input);
        out.push_str(",\n");
        push_indent(out, level + 2);
        out.push_str("\"output\": ");
        push_json_str(out, &st.output);
        out.push_str(",\n");
        push_indent(out, level + 2);
        out.push_str("\"steps\": ");
        push_json_steps(out, &st.steps, level + 2);
        out.push('\n');
        push_indent(out, level + 1);
        out.push('}');
        if idx + 1 < stages.len() {
            out.push(',');
        }
        out.push('\n');
    }
    push_indent(out, level);
    out.push(']');
}

fn push_json_steps(out: &mut String, steps: &[StepRow], level: usize) {
    if steps.is_empty() {
        out.push_str("[]");
        return;
    }
    out.push_str("[\n");
    for (idx, step) in steps.iter().enumerate() {
        push_indent(out, level + 1);
        out.push_str("{\n");
        push_indent(out, level + 2);
        out.push_str("\"rule_id\": ");
        push_json_str(out, &step.rule_id);
        out.push_str(",\n");
        push_indent(out, level + 2);
        out.push_str("\"rule_name\": ");
        push_json_str(out, &step.rule_name);
        out.push_str(",\n");
        push_indent(out, level + 2);
        out.push_str("\"before\": ");
        push_json_str(out, &step.before);
        out.push_str(",\n");
        push_indent(out, level + 2);
        out.push_str("\"after\": ");
        push_json_str(out, &step.after);
        out.push('\n');
        push_indent(out, level + 1);
        out.push('}');
        if idx + 1 < steps.len() {
            out.push(',');
        }
        out.push('\n');
    }
    push_indent(out, level);
    out.push(']');
}

fn format_compare_json(stages: &[String], rows: &[CompareRow]) -> String {
    let mut out = String::from("{\n");
    push_indent(&mut out, 1);
    out.push_str("\"command\": \"compare\",\n");
    push_indent(&mut out, 1);
    out.push_str("\"stages\": ");
    push_json_inline_str_array(&mut out, stages);
    out.push_str(",\n");
    push_indent(&mut out, 1);
    if rows.is_empty() {
        out.push_str("\"results\": []\n}\n");
        return out;
    }
    out.push_str("\"results\": [\n");
    for (idx, row) in rows.iter().enumerate() {
        push_indent(&mut out, 2);
        out.push_str("{\n");
        push_indent(&mut out, 3);
        out.push_str("\"input\": ");
        push_json_str(&mut out, &row.input);
        out.push_str(",\n");
        push_indent(&mut out, 3);
        if stages.is_empty() {
            out.push_str("\"outcomes\": {}\n");
        } else {
            out.push_str("\"outcomes\": {\n");
            for (s_idx, stage_id) in stages.iter().enumerate() {
                push_indent(&mut out, 4);
                push_json_str(&mut out, stage_id);
                out.push_str(": ");
                let outcome = row.outcomes.get(s_idx).map_or("", String::as_str);
                push_json_str(&mut out, outcome);
                if s_idx + 1 < stages.len() {
                    out.push(',');
                }
                out.push('\n');
            }
            push_indent(&mut out, 3);
            out.push_str("}\n");
        }
        push_indent(&mut out, 2);
        out.push('}');
        if idx + 1 < rows.len() {
            out.push(',');
        }
        out.push('\n');
    }
    push_indent(&mut out, 1);
    out.push_str("]\n}\n");
    out
}

fn push_indent(out: &mut String, level: usize) {
    for _ in 0..level {
        out.push_str("  ");
    }
}

fn push_json_opt_str(out: &mut String, value: Option<&str>) {
    match value {
        Some(text) => push_json_str(out, text),
        None => out.push_str("null"),
    }
}

fn push_json_inline_str_array(out: &mut String, values: &[String]) {
    out.push('[');
    for (idx, value) in values.iter().enumerate() {
        if idx > 0 {
            out.push_str(", ");
        }
        push_json_str(out, value);
    }
    out.push(']');
}

fn push_json_str(out: &mut String, value: &str) {
    out.push('"');
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{0008}' => out.push_str("\\b"),
            '\u{000C}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_string_escaping_handles_control_chars_quotes_and_ipa() {
        let mut rendered = String::new();
        push_json_str(&mut rendered, "a\"b\\c\n\r\t\u{0008}\u{000C}\u{0001}ʰˈeː");
        assert_eq!(rendered, "\"a\\\"b\\\\c\\n\\r\\t\\b\\f\\u0001ʰˈeː\"");
    }

    #[test]
    fn formats_run_across_text_markdown_and_json() {
        let rows = vec![RunRow {
            input: "vitam".to_owned(),
            output: "vida".to_owned(),
            rules_fired: vec!["R1".to_owned(), "R3".to_owned()],
        }];

        assert_eq!(
            format_run(None, &rows, OutputFormat::Text),
            "vitam -> vida\n"
        );
        assert_eq!(format_run(None, &[], OutputFormat::Text), "");

        assert_eq!(
            format_run(None, &rows, OutputFormat::Markdown),
            "| Input | Output |\n| --- | --- |\n| vitam | vida |\n"
        );

        let expected_json = concat!(
            "{\n",
            "  \"command\": \"run\",\n",
            "  \"stage\": null,\n",
            "  \"results\": [\n",
            "    {\n",
            "      \"input\": \"vitam\",\n",
            "      \"output\": \"vida\",\n",
            "      \"rules_fired\": [\"R1\", \"R3\"]\n",
            "    }\n",
            "  ]\n",
            "}\n",
        );
        assert_eq!(format_run(None, &rows, OutputFormat::Json), expected_json);
    }

    #[test]
    fn formats_flat_and_staged_traces_across_all_formats() {
        let flat_rows = vec![
            TraceRow {
                input: "vitam".to_owned(),
                output: "vida".to_owned(),
                body: TraceBody::Flat(vec![StepRow {
                    rule_id: "R1".to_owned(),
                    rule_name: "Final -m deletion".to_owned(),
                    before: "vitam".to_owned(),
                    after: "vita".to_owned(),
                }]),
            },
            TraceRow {
                input: "sala".to_owned(),
                output: "sala".to_owned(),
                body: TraceBody::Flat(Vec::new()),
            },
        ];

        assert_eq!(
            format_trace(None, &flat_rows, OutputFormat::Text),
            concat!(
                "vitam -> vida\n",
                "  [R1] Final -m deletion: vitam -> vita\n",
                "\n",
                "sala -> sala\n",
                "  (no changes)\n",
            )
        );
        assert_eq!(
            format_trace(None, &flat_rows, OutputFormat::Markdown),
            concat!(
                "## vitam -> vida\n\n",
                "| Rule ID | Rule | Before | After |\n",
                "| --- | --- | --- | --- |\n",
                "| R1 | Final -m deletion | vitam | vita |\n",
                "\n",
                "## sala -> sala\n\n",
                "_No changes._\n",
            )
        );

        let staged_rows = vec![TraceRow {
            input: "tʰeːs".to_owned(),
            output: "tʰeːs".to_owned(),
            body: TraceBody::Staged(vec![StageTraceRow {
                stage_id: "pie".to_owned(),
                stage_name: "Proto-Indo-European".to_owned(),
                input: "tʰeːs".to_owned(),
                output: "tʰeːs".to_owned(),
                steps: Vec::new(),
            }]),
        }];
        assert_eq!(
            format_trace(Some("attic"), &staged_rows, OutputFormat::Json),
            concat!(
                "{\n",
                "  \"command\": \"trace\",\n",
                "  \"stage\": \"attic\",\n",
                "  \"results\": [\n",
                "    {\n",
                "      \"input\": \"tʰeːs\",\n",
                "      \"output\": \"tʰeːs\",\n",
                "      \"stages\": [\n",
                "        {\n",
                "          \"stage_id\": \"pie\",\n",
                "          \"stage_name\": \"Proto-Indo-European\",\n",
                "          \"input\": \"tʰeːs\",\n",
                "          \"output\": \"tʰeːs\",\n",
                "          \"steps\": []\n",
                "        }\n",
                "      ]\n",
                "    }\n",
                "  ]\n",
                "}\n",
            )
        );
    }

    #[test]
    fn formats_compare_and_escapes_markdown_pipes() {
        let stages = vec!["attic".to_owned(), "ionic".to_owned()];
        let rows = vec![CompareRow {
            input: "maːtʰeːs".to_owned(),
            outcomes: vec!["meːtʰeːs".to_owned(), "meːtʰeːs".to_owned()],
        }];

        assert_eq!(format_compare(&stages, &[], OutputFormat::Text), "");
        assert_eq!(
            format_compare(&stages, &rows, OutputFormat::Text),
            concat!(
                "Input     attic     ionic\n",
                "--------  --------  --------\n",
                "maːtʰeːs  meːtʰeːs  meːtʰeːs\n",
            )
        );
        assert_eq!(
            format_compare(&stages, &rows, OutputFormat::Markdown),
            "| Input | attic | ionic |\n| --- | --- | --- |\n| maːtʰeːs | meːtʰeːs | meːtʰeːs |\n"
        );
        assert_eq!(
            format_compare(&stages, &rows, OutputFormat::Json),
            concat!(
                "{\n",
                "  \"command\": \"compare\",\n",
                "  \"stages\": [\"attic\", \"ionic\"],\n",
                "  \"results\": [\n",
                "    {\n",
                "      \"input\": \"maːtʰeːs\",\n",
                "      \"outcomes\": {\n",
                "        \"attic\": \"meːtʰeːs\",\n",
                "        \"ionic\": \"meːtʰeːs\"\n",
                "      }\n",
                "    }\n",
                "  ]\n",
                "}\n",
            )
        );

        assert_eq!(escape_markdown_cell("a|b"), "a\\|b");
    }
}
