//! Incremental JavaScript and TypeScript code metrics.
//!
//! ```
//! use ccrs::{Document, Language};
//! let mut doc = Document::new(Language::TypeScript, "const f = () => 1;".into())?;
//! assert_eq!(doc.analyze().scopes[1].complexity, Some(1));
//! doc.edit(16, 17, "x ? 1 : 0")?;
//! assert_eq!(doc.analyze().scopes[1].complexity, Some(2));
//! assert_eq!(doc.analyze().scopes[1].max_nesting, Some(0));
//! assert_eq!(doc.analyze().scopes[1].statement_count, Some(0));
//! assert_eq!(doc.analyze().scopes[1].cognitive_complexity, Some(1));
//! # Ok::<(), String>(())
//! ```
mod metrics;

use metrics::{cognitive_increment, increases_depth, nests_cognitively};
use serde::{Deserialize, Serialize};
use tree_sitter::{InputEdit, Node, Parser, Point, Tree};

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    JavaScript,
    TypeScript,
    Tsx,
}

/// Metrics selectable independently for each analysis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Metric {
    Cyclomatic,
    Nesting,
    Statements,
    Cognitive,
}

impl Metric {
    pub const ALL: [Self; 4] = [
        Self::Cyclomatic,
        Self::Nesting,
        Self::Statements,
        Self::Cognitive,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Cyclomatic => "cyclomatic",
            Self::Nesting => "nesting",
            Self::Statements => "statements",
            Self::Cognitive => "cognitive",
        }
    }
}

impl std::str::FromStr for Metric {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::ALL.into_iter().find(|metric| metric.name() == value)
            .ok_or_else(|| format!("Unknown metric '{value}'; expected cyclomatic, nesting, statements, or cognitive"))
    }
}

/// A lexical scope. Unselected metrics are None and omitted from serialized reports.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Scope {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub complexity: Option<usize>,
    /// Maximum control-flow nesting, with each scope starting at zero.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_nesting: Option<usize>,
    /// Number of block entries, following ESLint max-statements.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub statement_count: Option<usize>,
    /// SonarJS-style cognitive complexity, starting at zero.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cognitive_complexity: Option<usize>,
    pub start_byte: usize,
    pub end_byte: usize,
    /// One-based line number.
    pub line: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Report {
    /// Counts are provisional when the parser recovered from syntax errors.
    pub has_errors: bool,
    pub scopes: Vec<Scope>,
}

#[derive(Clone, Copy, Default)]
struct Context {
    scope: usize,
    depth: usize,
    cognitive_depth: usize,
}

pub struct Document {
    parser: Parser,
    source: String,
    tree: Tree,
}

impl Document {
    pub fn new(language: Language, source: String) -> Result<Self, String> {
        let mut parser = Parser::new();
        let grammar = match language {
            Language::JavaScript => tree_sitter_javascript::LANGUAGE,
            Language::TypeScript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT,
            Language::Tsx => tree_sitter_typescript::LANGUAGE_TSX,
        };
        parser
            .set_language(&grammar.into())
            .map_err(|e| e.to_string())?;
        let tree = parser.parse(&source, None).ok_or("Parsing failed")?;
        Ok(Self {
            parser,
            source,
            tree,
        })
    }

    /// Replace a half-open UTF-8 byte range, retaining the previous parse tree.
    pub fn edit(&mut self, start: usize, end: usize, replacement: &str) -> Result<(), String> {
        if start > end || !self.source.is_char_boundary(start) || !self.source.is_char_boundary(end)
        {
            return Err("Edit must be an ordered range on UTF-8 byte boundaries".into());
        }
        let start_position = advance(Point::new(0, 0), &self.source[..start]);
        let edit = InputEdit {
            start_byte: start,
            old_end_byte: end,
            new_end_byte: start + replacement.len(),
            start_position,
            old_end_position: advance(start_position, &self.source[start..end]),
            new_end_position: advance(start_position, replacement),
        };
        let mut source = self.source.clone();
        source.replace_range(start..end, replacement);
        let mut tree = self.tree.clone();
        tree.edit(&edit);
        let tree = self
            .parser
            .parse(&source, Some(&tree))
            .ok_or("Parsing failed")?;
        self.source = source;
        self.tree = tree;
        Ok(())
    }

    /// Calculate all four metrics.
    pub fn analyze(&self) -> Report {
        self.analyze_with(&Metric::ALL)
    }

    /// Calculate only selected metrics. Disabled values are None and omitted from JSON.
    /// An empty selection returns scope locations and parser status only.
    pub fn analyze_with(&self, metrics: &[Metric]) -> Report {
        let root = self.tree.root_node();
        let mut scopes = vec![scope(root, "<module>".into(), metrics)];
        let mut pending = vec![(root, Context::default(), false)];
        while let Some((node, outer, initializer)) = pending.pop() {
            let Context {
                scope: mut enclosing,
                mut depth,
                mut cognitive_depth,
            } = outer;
            if initializer {
                depth = 0;
                cognitive_depth = 0;
                scopes.push(scope(node, "<field initializer>".into(), metrics));
                enclosing = scopes.len() - 1;
            }
            let current = if is_function(node) || node.kind() == "class_static_block" {
                depth = 0;
                cognitive_depth = 0;
                let name = node
                    .child_by_field_name("name")
                    .or_else(|| {
                        node.parent()
                            .filter(|p| p.kind() == "variable_declarator")
                            .and_then(|p| p.child_by_field_name("name"))
                    })
                    .map(|n| self.source[n.byte_range()].to_owned())
                    .unwrap_or_else(|| {
                        if node.kind() == "class_static_block" {
                            "<static block>".into()
                        } else {
                            "<anonymous>".into()
                        }
                    });
                scopes.push(scope(node, name, metrics));
                scopes.len() - 1
            } else {
                enclosing
            };
            if let Some(value) = &mut scopes[current].complexity {
                *value += usize::from(is_decision(node));
            }
            if let Some(value) = &mut scopes[current].cognitive_complexity {
                *value += cognitive_increment(node, cognitive_depth);
            }
            if let Some(value) = &mut scopes[current].max_nesting {
                depth += usize::from(increases_depth(node));
                *value = (*value).max(depth);
            }
            let function = is_function(node);
            let name = if function {
                node.child_by_field_name("name")
            } else {
                None
            };
            let value = if matches!(node.kind(), "field_definition" | "public_field_definition") {
                node.child_by_field_name("value")
            } else {
                None
            };
            let counts_statements = scopes[current].statement_count.is_some()
                && matches!(node.kind(), "program" | "statement_block");
            let mut cursor = node.walk();
            if cursor.goto_last_child() {
                loop {
                    let child = cursor.node();
                    if child.is_named() {
                        if counts_statements
                            && !matches!(child.kind(), "comment" | "hash_bang_line")
                            && let Some(value) = &mut scopes[current].statement_count
                        {
                            *value += 1;
                        }
                        let owner =
                            if function && (name == Some(child) || child.kind() == "decorator") {
                                enclosing
                            } else {
                                current
                            };
                        let context = if owner == current {
                            Context {
                                scope: current,
                                depth,
                                cognitive_depth: cognitive_depth
                                    + usize::from(
                                        scopes[current].cognitive_complexity.is_some()
                                            && nests_cognitively(node, child),
                                    ),
                            }
                        } else {
                            outer
                        };
                        pending.push((child, context, value == Some(child)));
                    }
                    if !cursor.goto_previous_sibling() {
                        break;
                    }
                }
            }
        }
        Report {
            has_errors: root.has_error(),
            scopes,
        }
    }
}

fn scope(node: Node<'_>, name: String, metrics: &[Metric]) -> Scope {
    Scope {
        name,
        complexity: metrics.contains(&Metric::Cyclomatic).then_some(1),
        max_nesting: metrics.contains(&Metric::Nesting).then_some(0),
        statement_count: metrics.contains(&Metric::Statements).then_some(0),
        cognitive_complexity: metrics.contains(&Metric::Cognitive).then_some(0),
        start_byte: node.start_byte(),
        end_byte: node.end_byte(),
        line: node.start_position().row + 1,
    }
}

fn is_decision(node: Node<'_>) -> bool {
    match node.kind() {
        "if_statement"
        | "for_statement"
        | "for_in_statement"
        | "while_statement"
        | "do_statement"
        | "catch_clause"
        | "switch_case"
        | "ternary_expression"
        | "optional_chain"
        | "assignment_pattern"
        | "object_assignment_pattern" => true,
        "binary_expression" | "augmented_assignment_expression" => node
            .child_by_field_name("operator")
            .is_some_and(|op| matches!(op.kind(), "&&" | "||" | "??" | "&&=" | "||=" | "??=")),
        "required_parameter" | "optional_parameter" => node.child_by_field_name("value").is_some(),
        "call_expression" => {
            let mut cursor = node.walk();
            node.children(&mut cursor).any(|child| child.kind() == "?.")
        }
        _ => false,
    }
}

fn is_function(node: Node<'_>) -> bool {
    matches!(
        node.kind(),
        "function_declaration"
            | "function_expression"
            | "arrow_function"
            | "generator_function_declaration"
            | "generator_function"
            | "method_definition"
    )
}

fn advance(mut point: Point, text: &str) -> Point {
    for byte in text.bytes() {
        if byte == b'\n' {
            point.row += 1;
            point.column = 0;
        } else {
            point.column += 1;
        }
    }
    point
}
