//! Section-level operations for incremental wiki updates.
//!
//! Rather than asking the LLM to fully rewrite an article on every update, we
//! ask it to emit a list of structured operations against the existing article.
//! The applier merges them in. Untouched sections stay byte-identical, which
//! makes the review diff localized to what actually changed and preserves the
//! existing citation graph.

use serde::{Deserialize, Serialize};

/// A single operation against an existing wiki article.
///
/// Headings in `AppendToSection` / `ReplaceSection` must exactly match one of
/// the existing `##` or `###` headings (trimmed, case-sensitive). A missing
/// heading is treated as a hallucination and causes the whole proposal to be
/// discarded — we do not fuzzy-match.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(tag = "op")]
pub enum WikiSectionOp {
    /// No change — the new sources don't warrant updating the article.
    NoChange,
    /// Append `content` to the body of an existing section.
    AppendToSection { heading: String, content: String },
    /// Replace the body of an existing section (heading line preserved).
    ReplaceSection { heading: String, content: String },
    /// Insert a brand-new section. If `after_heading` is `None`, the section
    /// is appended to the end of the document.
    InsertSection {
        after_heading: Option<String>,
        heading: String,
        content: String,
    },
}

/// Flat wire shape that LLMs actually emit. Matches the structured-output
/// JSON schema: all fields are plain strings, all are required, empty string
/// is the sentinel for "not applicable." This avoids `["string", "null"]`
/// type unions (unreliable on smaller local models) and `oneOf` discriminated
/// unions (rejected by some provider strict modes) while staying aligned with
/// the convention used by `extraction.rs` for tag extraction.
#[derive(Debug, Clone, Deserialize)]
pub struct WikiSectionOpWire {
    pub op: String,
    #[serde(default)]
    pub heading: String,
    #[serde(default)]
    pub after_heading: String,
    #[serde(default)]
    pub content: String,
}

impl WikiSectionOpWire {
    /// Validate and convert the wire shape into the strict enum.
    ///
    /// Returns a descriptive error (for logs + user-facing "LLM hallucinated"
    /// messaging) if the op string is unknown or a required field is empty
    /// for the chosen variant.
    pub fn into_op(self) -> Result<WikiSectionOp, String> {
        match self.op.as_str() {
            "NoChange" => Ok(WikiSectionOp::NoChange),
            "AppendToSection" => {
                if self.heading.trim().is_empty() {
                    return Err("AppendToSection requires a non-empty heading".to_string());
                }
                if self.content.trim().is_empty() {
                    return Err("AppendToSection requires non-empty content".to_string());
                }
                Ok(WikiSectionOp::AppendToSection {
                    heading: self.heading,
                    content: self.content,
                })
            }
            "ReplaceSection" => {
                if self.heading.trim().is_empty() {
                    return Err("ReplaceSection requires a non-empty heading".to_string());
                }
                if self.content.trim().is_empty() {
                    return Err("ReplaceSection requires non-empty content".to_string());
                }
                Ok(WikiSectionOp::ReplaceSection {
                    heading: self.heading,
                    content: self.content,
                })
            }
            "InsertSection" => {
                if self.heading.trim().is_empty() {
                    return Err("InsertSection requires a non-empty heading".to_string());
                }
                if self.content.trim().is_empty() {
                    return Err("InsertSection requires non-empty content".to_string());
                }
                // Empty `after_heading` means "append to end" (sentinel convention).
                let after_heading = if self.after_heading.trim().is_empty() {
                    None
                } else {
                    Some(self.after_heading)
                };
                Ok(WikiSectionOp::InsertSection {
                    after_heading,
                    heading: self.heading,
                    content: self.content,
                })
            }
            other => Err(format!(
                "Unknown op '{}' — expected NoChange, AppendToSection, ReplaceSection, or InsertSection",
                other
            )),
        }
    }
}

/// Internal representation of a parsed section.
#[derive(Debug, Clone)]
struct Section {
    /// Markdown level (2 for `##`, 3 for `###`).
    level: u8,
    /// Heading text, with `##`/`###` prefix and leading whitespace stripped.
    heading: String,
    /// Body text *without* the heading line, but including any trailing blank
    /// lines so round-tripping stays stable for untouched sections.
    body: String,
}

/// Apply a list of section operations to an existing article body.
///
/// Returns the merged markdown. Errors only if *every* meaningful op fails
/// (e.g. every heading is hallucinated) — individual misses are skipped with a
/// tracing warning.
pub fn apply_section_ops(existing: &str, ops: &[WikiSectionOp]) -> Result<String, String> {
    let (preamble, mut sections) = parse_sections(existing);

    // Count non-NoChange ops so we can detect "all failed" later.
    let meaningful = ops
        .iter()
        .filter(|o| !matches!(o, WikiSectionOp::NoChange))
        .count();
    let mut skipped: Vec<String> = Vec::new();

    for op in ops {
        match op {
            WikiSectionOp::NoChange => {
                continue;
            }
            WikiSectionOp::AppendToSection { heading, content } => {
                match find_section_idx(&sections, heading) {
                    Some(idx) => {
                        append_to_body(&mut sections[idx].body, content);
                    }
                    None => {
                        let msg = format!(
                            "AppendToSection: heading '{}' not found. Existing headings: [{}]",
                            heading,
                            list_headings(&sections)
                        );
                        tracing::warn!(error = %msg, "[wiki] Skipping op with unmatched heading");
                        skipped.push(msg);
                    }
                }
            }
            WikiSectionOp::ReplaceSection { heading, content } => {
                match find_section_idx(&sections, heading) {
                    Some(idx) => {
                        sections[idx].body = ensure_trailing_blank(content);
                    }
                    None => {
                        let msg = format!(
                            "ReplaceSection: heading '{}' not found. Existing headings: [{}]",
                            heading,
                            list_headings(&sections)
                        );
                        tracing::warn!(error = %msg, "[wiki] Skipping op with unmatched heading");
                        skipped.push(msg);
                    }
                }
            }
            WikiSectionOp::InsertSection {
                after_heading,
                heading,
                content,
            } => match after_heading {
                Some(h) => match find_section_idx(&sections, h) {
                    Some(idx) => {
                        // Inherit the level of the anchor section so that
                        // inserting after an H3 produces another H3, not H2.
                        let level = sections[idx].level;
                        let insert_idx = after_subtree_idx(&sections, idx);
                        sections.insert(
                            insert_idx,
                            Section {
                                level,
                                heading: heading.clone(),
                                body: ensure_trailing_blank(content),
                            },
                        );
                    }
                    None => {
                        let msg = format!(
                            "InsertSection: after_heading '{}' not found. Existing headings: [{}]",
                            h,
                            list_headings(&sections)
                        );
                        tracing::warn!(error = %msg, "[wiki] Skipping op with unmatched heading");
                        skipped.push(msg);
                    }
                },
                None => {
                    sections.push(Section {
                        level: 2,
                        heading: heading.clone(),
                        body: ensure_trailing_blank(content),
                    });
                }
            },
        }
    }

    // Hard-error only when every meaningful op was skipped.
    if meaningful > 0 && skipped.len() == meaningful {
        return Err(skipped.join("; "));
    }

    Ok(serialize_sections(&preamble, &sections))
}

fn after_subtree_idx(sections: &[Section], anchor_idx: usize) -> usize {
    let anchor_level = sections[anchor_idx].level;
    sections
        .iter()
        .enumerate()
        .skip(anchor_idx + 1)
        .find_map(|(idx, section)| (section.level <= anchor_level).then_some(idx))
        .unwrap_or(sections.len())
}

/// Parse the article into (preamble, sections). The preamble is any content
/// before the first heading at level 2–6. Headings at levels 2–6 each begin a
/// new section; H1 stays in the preamble; content under an H3+ heading belongs
/// to that section's own body, not its parent's.
fn parse_sections(content: &str) -> (String, Vec<Section>) {
    let mut preamble = String::new();
    let mut sections: Vec<Section> = Vec::new();
    let mut current: Option<Section> = None;

    for line in content.split_inclusive('\n') {
        if let Some((level, heading)) = parse_heading(line) {
            if level >= 2 {
                if let Some(sec) = current.take() {
                    sections.push(sec);
                }
                current = Some(Section {
                    level,
                    heading: heading.to_string(),
                    body: String::new(),
                });
                continue;
            }
        }

        match current.as_mut() {
            Some(sec) => sec.body.push_str(line),
            None => preamble.push_str(line),
        }
    }

    if let Some(sec) = current.take() {
        sections.push(sec);
    }

    // Normalize each section's body: strip leading blank lines (the blank
    // between heading and body will be re-emitted during serialization) and
    // ensure a trailing blank-line terminator.
    for sec in &mut sections {
        while sec.body.starts_with('\n') || sec.body.starts_with("\r\n") {
            if sec.body.starts_with("\r\n") {
                sec.body.drain(..2);
            } else {
                sec.body.drain(..1);
            }
        }
        sec.body = ensure_trailing_blank(&sec.body);
    }

    (preamble, sections)
}

/// Parse a line as a markdown heading. Returns (level, heading_text) if the
/// line starts with `## ` or `### ` (etc). Ignores `#` (level 1).
fn parse_heading(line: &str) -> Option<(u8, &str)> {
    let trimmed = line.trim_end_matches(|c| c == '\n' || c == '\r');
    let stripped = trimmed.trim_start();
    let bytes = stripped.as_bytes();
    let mut hashes = 0;
    while hashes < bytes.len() && bytes[hashes] == b'#' {
        hashes += 1;
    }
    if hashes < 2 || hashes > 6 {
        return None;
    }
    if hashes >= bytes.len() || bytes[hashes] != b' ' {
        return None;
    }
    let text = stripped[hashes + 1..].trim();
    Some((hashes as u8, text))
}

fn find_section_idx(sections: &[Section], heading: &str) -> Option<usize> {
    let target = heading.trim();
    sections.iter().position(|s| s.heading.trim() == target)
}

fn list_headings(sections: &[Section]) -> String {
    sections
        .iter()
        .map(|s| format!("'{}'", s.heading))
        .collect::<Vec<_>>()
        .join(", ")
}

fn append_to_body(body: &mut String, content: &str) {
    // Ensure there's a blank line between the existing body and the new content.
    if !body.is_empty() && !body.ends_with("\n\n") {
        if body.ends_with('\n') {
            body.push('\n');
        } else {
            body.push_str("\n\n");
        }
    }
    body.push_str(content.trim_end());
    body.push_str("\n\n");
}

fn ensure_trailing_blank(content: &str) -> String {
    let mut s = content.trim_end().to_string();
    s.push_str("\n\n");
    s
}

fn serialize_sections(preamble: &str, sections: &[Section]) -> String {
    let mut out = String::new();
    out.push_str(preamble);
    // Ensure a blank line between preamble and first section if preamble is non-empty.
    if !preamble.is_empty() && !preamble.ends_with("\n\n") {
        if preamble.ends_with('\n') {
            out.push('\n');
        } else {
            out.push_str("\n\n");
        }
    }
    for sec in sections {
        let hashes = "#".repeat(sec.level as usize);
        // Heading line + mandatory blank line between heading and body.
        out.push_str(&format!("{} {}\n\n", hashes, sec.heading));
        out.push_str(&sec.body);
        // Guarantee separation between sections.
        if !out.ends_with("\n\n") {
            if out.ends_with('\n') {
                out.push('\n');
            } else {
                out.push_str("\n\n");
            }
        }
    }
    // Trim any excess trailing blank lines down to a single trailing newline.
    while out.ends_with("\n\n\n") {
        out.pop();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
# My Article

Preamble text.

## Overview

Overview body with [1] citation.

## Details

Details body.

### Subsection

Subsection text.

## Status

Status body.
";

    #[test]
    fn no_change_preserves_content() {
        let out = apply_section_ops(SAMPLE, &[WikiSectionOp::NoChange]).unwrap();
        assert_eq!(out.trim(), SAMPLE.trim());
    }

    #[test]
    fn empty_ops_preserves_content() {
        let out = apply_section_ops(SAMPLE, &[]).unwrap();
        assert_eq!(out.trim(), SAMPLE.trim());
    }

    #[test]
    fn append_adds_to_existing_section() {
        let ops = vec![WikiSectionOp::AppendToSection {
            heading: "Details".to_string(),
            content: "New detail [3].".to_string(),
        }];
        let out = apply_section_ops(SAMPLE, &ops).unwrap();
        assert!(out.contains("Details body."));
        assert!(out.contains("### Subsection"));
        assert!(out.contains("New detail [3]."));
        // Overview is untouched and precedes the modified section.
        let overview_pos = out.find("Overview body").unwrap();
        let new_detail_pos = out.find("New detail").unwrap();
        assert!(overview_pos < new_detail_pos);
    }

    #[test]
    fn append_preserves_untouched_sections_byte_for_byte() {
        let ops = vec![WikiSectionOp::AppendToSection {
            heading: "Status".to_string(),
            content: "New status line [3].".to_string(),
        }];
        let out = apply_section_ops(SAMPLE, &ops).unwrap();
        // Overview section must appear exactly as it did in the source.
        assert!(out.contains("## Overview\n\nOverview body with [1] citation."));
    }

    #[test]
    fn replace_swaps_body_but_keeps_heading() {
        let ops = vec![WikiSectionOp::ReplaceSection {
            heading: "Status".to_string(),
            content: "Totally new status [3].".to_string(),
        }];
        let out = apply_section_ops(SAMPLE, &ops).unwrap();
        assert!(out.contains("## Status\n\nTotally new status [3]."));
        assert!(!out.contains("Status body."));
    }

    #[test]
    fn insert_after_specific_heading() {
        let ops = vec![WikiSectionOp::InsertSection {
            after_heading: Some("Overview".to_string()),
            heading: "Background".to_string(),
            content: "Background content [3].".to_string(),
        }];
        let out = apply_section_ops(SAMPLE, &ops).unwrap();
        let overview_pos = out.find("## Overview").unwrap();
        let background_pos = out.find("## Background").unwrap();
        let details_pos = out.find("## Details").unwrap();
        assert!(overview_pos < background_pos);
        assert!(background_pos < details_pos);
        assert!(out.contains("Background content [3]."));
    }

    #[test]
    fn insert_with_none_appends_to_end() {
        let ops = vec![WikiSectionOp::InsertSection {
            after_heading: None,
            heading: "Appendix".to_string(),
            content: "Appendix content [3].".to_string(),
        }];
        let out = apply_section_ops(SAMPLE, &ops).unwrap();
        let status_pos = out.find("## Status").unwrap();
        let appendix_pos = out.find("## Appendix").unwrap();
        assert!(status_pos < appendix_pos);
    }

    #[test]
    fn hallucinated_heading_returns_error() {
        let ops = vec![WikiSectionOp::AppendToSection {
            heading: "Nonexistent".to_string(),
            content: "whatever".to_string(),
        }];
        let err = apply_section_ops(SAMPLE, &ops).unwrap_err();
        assert!(err.contains("Nonexistent"));
        assert!(err.contains("Overview"));
        assert!(err.contains("Details"));
    }

    #[test]
    fn h3_sections_are_their_own_sections() {
        // ### Subsection is now a first-class section, not embedded in Details.
        let (_, sections) = parse_sections(SAMPLE);
        let headings: Vec<&str> = sections.iter().map(|s| s.heading.as_str()).collect();
        assert_eq!(
            headings,
            vec!["Overview", "Details", "Subsection", "Status"]
        );
        let sub = sections.iter().find(|s| s.heading == "Subsection").unwrap();
        assert_eq!(sub.level, 3);
        assert!(sub.body.contains("Subsection text."));
        // Details body must NOT contain the subsection heading any more.
        let details = sections.iter().find(|s| s.heading == "Details").unwrap();
        assert!(!details.body.contains("### Subsection"));
    }

    #[test]
    fn multi_op_sequence_applied_in_order() {
        let ops = vec![
            WikiSectionOp::AppendToSection {
                heading: "Overview".to_string(),
                content: "Added to overview [3].".to_string(),
            },
            WikiSectionOp::InsertSection {
                after_heading: Some("Details".to_string()),
                heading: "Notes".to_string(),
                content: "Notes content [4].".to_string(),
            },
            WikiSectionOp::ReplaceSection {
                heading: "Status".to_string(),
                content: "Replaced status [5].".to_string(),
            },
        ];
        let out = apply_section_ops(SAMPLE, &ops).unwrap();
        assert!(out.contains("Added to overview [3]."));
        assert!(out.contains("## Notes\n\nNotes content [4]."));
        assert!(out.contains("## Status\n\nReplaced status [5]."));
        assert!(!out.contains("Status body."));

        // Verify order: Overview, Details subtree, Notes, Status.
        let overview_pos = out.find("## Overview").unwrap();
        let details_pos = out.find("## Details").unwrap();
        let subsection_pos = out.find("### Subsection").unwrap();
        let notes_pos = out.find("## Notes").unwrap();
        let status_pos = out.find("## Status").unwrap();
        assert!(overview_pos < details_pos);
        assert!(details_pos < subsection_pos);
        assert!(subsection_pos < notes_pos);
        assert!(notes_pos < status_pos);
    }

    #[test]
    fn wire_shape_no_change_ignores_sentinels() {
        // Sentinel empty strings on a NoChange op must not cause errors.
        let wire = WikiSectionOpWire {
            op: "NoChange".into(),
            heading: "".into(),
            after_heading: "".into(),
            content: "".into(),
        };
        assert_eq!(wire.into_op().unwrap(), WikiSectionOp::NoChange);
    }

    #[test]
    fn wire_shape_append_validates_required_fields() {
        let wire = WikiSectionOpWire {
            op: "AppendToSection".into(),
            heading: "Details".into(),
            after_heading: "".into(),
            content: "new material [3]".into(),
        };
        let op = wire.into_op().unwrap();
        assert_eq!(
            op,
            WikiSectionOp::AppendToSection {
                heading: "Details".into(),
                content: "new material [3]".into(),
            }
        );
    }

    #[test]
    fn wire_shape_append_rejects_empty_heading() {
        let wire = WikiSectionOpWire {
            op: "AppendToSection".into(),
            heading: "".into(),
            after_heading: "".into(),
            content: "x".into(),
        };
        let err = wire.into_op().unwrap_err();
        assert!(err.contains("heading"));
    }

    #[test]
    fn wire_shape_insert_with_empty_after_heading_is_append_to_end() {
        let wire = WikiSectionOpWire {
            op: "InsertSection".into(),
            heading: "Appendix".into(),
            after_heading: "".into(),
            content: "body".into(),
        };
        let op = wire.into_op().unwrap();
        assert_eq!(
            op,
            WikiSectionOp::InsertSection {
                after_heading: None,
                heading: "Appendix".into(),
                content: "body".into(),
            }
        );
    }

    #[test]
    fn wire_shape_insert_with_after_heading_preserves_it() {
        let wire = WikiSectionOpWire {
            op: "InsertSection".into(),
            heading: "New".into(),
            after_heading: "Overview".into(),
            content: "body".into(),
        };
        let op = wire.into_op().unwrap();
        assert_eq!(
            op,
            WikiSectionOp::InsertSection {
                after_heading: Some("Overview".into()),
                heading: "New".into(),
                content: "body".into(),
            }
        );
    }

    #[test]
    fn wire_shape_rejects_unknown_op() {
        let wire = WikiSectionOpWire {
            op: "RewriteEverything".into(),
            heading: "".into(),
            after_heading: "".into(),
            content: "".into(),
        };
        let err = wire.into_op().unwrap_err();
        assert!(err.contains("Unknown op"));
    }

    #[test]
    fn wire_shape_deserializes_from_flat_json_with_all_fields() {
        // Exactly the shape the LLM structured-output schema asks for.
        let json =
            r#"{"op":"AppendToSection","heading":"Details","after_heading":"","content":"x [3]"}"#;
        let wire: WikiSectionOpWire = serde_json::from_str(json).unwrap();
        assert_eq!(wire.op, "AppendToSection");
        let op = wire.into_op().unwrap();
        assert!(matches!(op, WikiSectionOp::AppendToSection { .. }));
    }

    #[test]
    fn serde_roundtrip_tagged_enum() {
        let ops = vec![
            WikiSectionOp::NoChange,
            WikiSectionOp::AppendToSection {
                heading: "X".into(),
                content: "y".into(),
            },
            WikiSectionOp::ReplaceSection {
                heading: "X".into(),
                content: "y".into(),
            },
            WikiSectionOp::InsertSection {
                after_heading: Some("X".into()),
                heading: "Y".into(),
                content: "z".into(),
            },
            WikiSectionOp::InsertSection {
                after_heading: None,
                heading: "Y".into(),
                content: "z".into(),
            },
        ];
        let json = serde_json::to_string(&ops).unwrap();
        let roundtrip: Vec<WikiSectionOp> = serde_json::from_str(&json).unwrap();
        assert_eq!(ops, roundtrip);
    }

    const MULTI_LEVEL: &str = "\
## Parent A\n\
\n\
Parent A body.\n\
\n\
### Child A1\n\
\n\
Child A1 body.\n\
\n\
### Child A2\n\
\n\
Child A2 body.\n\
\n\
## Parent B\n\
\n\
Parent B body.\n\
";

    #[test]
    fn append_to_h3_section() {
        let ops = vec![WikiSectionOp::AppendToSection {
            heading: "Child A1".to_string(),
            content: "New A1 line [3].".to_string(),
        }];
        let out = apply_section_ops(MULTI_LEVEL, &ops).unwrap();
        assert!(out.contains("Child A1 body."));
        assert!(out.contains("New A1 line [3]."));
        // Child A2 and Parent B are untouched.
        assert!(out.contains("### Child A2\n\nChild A2 body."));
        assert!(out.contains("## Parent B\n\nParent B body."));
    }

    #[test]
    fn serialize_preserves_levels() {
        // Round-trip with NoChange must produce byte-identical output.
        let out = apply_section_ops(MULTI_LEVEL, &[WikiSectionOp::NoChange]).unwrap();
        assert_eq!(out.trim(), MULTI_LEVEL.trim());
    }

    #[test]
    fn insert_section_after_h3_inherits_level() {
        let ops = vec![WikiSectionOp::InsertSection {
            after_heading: Some("Child A1".to_string()),
            heading: "Child A1a".to_string(),
            content: "A1a body.".to_string(),
        }];
        let out = apply_section_ops(MULTI_LEVEL, &ops).unwrap();
        // New section must be emitted as ### (level 3), not ## (level 2).
        assert!(out.contains("### Child A1a\n\nA1a body."));
        // It appears between Child A1 and Child A2.
        let a1_pos = out.find("### Child A1\n").unwrap();
        let a1a_pos = out.find("### Child A1a").unwrap();
        let a2_pos = out.find("### Child A2").unwrap();
        assert!(a1_pos < a1a_pos);
        assert!(a1a_pos < a2_pos);
    }

    #[test]
    fn apply_section_ops_tolerates_single_bad_op() {
        // One valid op + one hallucinated heading: valid op must land.
        let ops = vec![
            WikiSectionOp::AppendToSection {
                heading: "Nonexistent".to_string(),
                content: "ghost content".to_string(),
            },
            WikiSectionOp::AppendToSection {
                heading: "Overview".to_string(),
                content: "real addition [3].".to_string(),
            },
        ];
        let out = apply_section_ops(SAMPLE, &ops).unwrap();
        assert!(out.contains("real addition [3]."));
        assert!(!out.contains("ghost content"));
    }

    #[test]
    fn apply_section_ops_aborts_when_all_ops_invalid() {
        let ops = vec![
            WikiSectionOp::AppendToSection {
                heading: "Ghost1".to_string(),
                content: "c1".to_string(),
            },
            WikiSectionOp::AppendToSection {
                heading: "Ghost2".to_string(),
                content: "c2".to_string(),
            },
        ];
        let err = apply_section_ops(SAMPLE, &ops).unwrap_err();
        assert!(err.contains("Ghost1"));
        assert!(err.contains("Ghost2"));
    }

    // Tests using SAMPLE's existing ### Subsection

    #[test]
    fn replace_h3_section() {
        let ops = vec![WikiSectionOp::ReplaceSection {
            heading: "Subsection".to_string(),
            content: "Replaced subsection [4].".to_string(),
        }];
        let out = apply_section_ops(SAMPLE, &ops).unwrap();
        assert!(out.contains("### Subsection\n\nReplaced subsection [4]."));
        assert!(!out.contains("Subsection text."));
    }

    #[test]
    fn insert_after_h3_inherits_level() {
        let ops = vec![WikiSectionOp::InsertSection {
            after_heading: Some("Subsection".to_string()),
            heading: "Another Sub".to_string(),
            content: "More sub content [4].".to_string(),
        }];
        let out = apply_section_ops(SAMPLE, &ops).unwrap();
        assert!(out.contains("### Another Sub\n\nMore sub content [4]."));
        let sub_pos = out.find("### Subsection").unwrap();
        let another_pos = out.find("### Another Sub").unwrap();
        let status_pos = out.find("## Status").unwrap();
        assert!(sub_pos < another_pos);
        assert!(another_pos < status_pos);
    }

    #[test]
    fn insert_after_h2_still_produces_h2() {
        let ops = vec![WikiSectionOp::InsertSection {
            after_heading: Some("Overview".to_string()),
            heading: "Background".to_string(),
            content: "Background content [4].".to_string(),
        }];
        let out = apply_section_ops(SAMPLE, &ops).unwrap();
        assert!(out.contains("## Background\n\nBackground content [4]."));
    }

    #[test]
    fn insert_after_h2_keeps_child_sections_with_parent() {
        let ops = vec![WikiSectionOp::InsertSection {
            after_heading: Some("Details".to_string()),
            heading: "Follow Up".to_string(),
            content: "Follow-up content [4].".to_string(),
        }];
        let out = apply_section_ops(SAMPLE, &ops).unwrap();

        let details_pos = out.find("## Details").unwrap();
        let subsection_pos = out.find("### Subsection").unwrap();
        let follow_up_pos = out.find("## Follow Up").unwrap();
        let status_pos = out.find("## Status").unwrap();

        assert!(details_pos < subsection_pos);
        assert!(subsection_pos < follow_up_pos);
        assert!(follow_up_pos < status_pos);
    }

    #[test]
    fn all_bad_ops_returns_error() {
        let ops = vec![
            WikiSectionOp::AppendToSection {
                heading: "Ghost Section".to_string(),
                content: "x".to_string(),
            },
            WikiSectionOp::ReplaceSection {
                heading: "Phantom".to_string(),
                content: "y".to_string(),
            },
        ];
        let err = apply_section_ops(SAMPLE, &ops).unwrap_err();
        assert!(err.contains("Ghost Section"));
    }
}
