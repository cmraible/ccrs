use tree_sitter::Node;

fn is_else_if(node: Node<'_>) -> bool {
    node.kind() == "if_statement" && node.parent().is_some_and(|p| p.kind() == "else_clause")
}

pub(crate) fn increases_depth(node: Node<'_>) -> bool {
    match node.kind() {
        "if_statement" => !is_else_if(node),
        "switch_statement" | "try_statement" | "for_statement" | "for_in_statement"
        | "while_statement" | "do_statement" | "with_statement" => true,
        _ => false,
    }
}

pub(crate) fn cognitive_increment(node: Node<'_>, depth: usize) -> usize {
    match node.kind() {
        "if_statement" if is_else_if(node) => 1,
        "if_statement" | "for_statement" | "for_in_statement" | "while_statement"
        | "do_statement" | "switch_statement" | "catch_clause" | "ternary_expression" => depth + 1,
        "else_clause" => usize::from(!has_else_if(node)),
        "binary_expression" if logical_operator(node).is_some() => logical_complexity(node),
        "break_statement" | "continue_statement" => {
            usize::from(node.child_by_field_name("label").is_some())
        }
        _ => 0,
    }
}

fn has_else_if(node: Node<'_>) -> bool {
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .any(|n| n.kind() == "if_statement")
}

pub(crate) fn nests_cognitively(node: Node<'_>, child: Node<'_>) -> bool {
    let field = match node.kind() {
        "if_statement" => "consequence",
        "else_clause" => return !has_else_if(node),
        "for_statement" | "for_in_statement" | "while_statement" | "do_statement"
        | "catch_clause" | "switch_statement" => "body",
        "ternary_expression" => {
            return node.child_by_field_name("consequence") == Some(child)
                || node.child_by_field_name("alternative") == Some(child);
        }
        _ => return false,
    };
    node.child_by_field_name(field) == Some(child)
}

fn logical_operator(node: Node<'_>) -> Option<&str> {
    if node.kind() != "binary_expression" {
        return None;
    }
    node.child_by_field_name("operator")
        .map(|n| n.kind())
        .filter(|op| matches!(*op, "&&" | "||" | "??"))
}

fn expression_parent(mut node: Node<'_>) -> Option<Node<'_>> {
    while let Some(parent) = node.parent() {
        if transparent_child(parent) != Some(node) {
            return Some(parent);
        }
        node = parent;
    }
    None
}

fn first_expression(node: Node<'_>) -> Option<Node<'_>> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .find(|n| n.kind() != "comment")
}

fn transparent_child(node: Node<'_>) -> Option<Node<'_>> {
    if node.kind() == "parenthesized_expression" {
        return first_expression(node);
    }
    // These grammars put an unparenthesized logical expression under `as`/`satisfies`/`!`.
    // TypeScript binds the assertion to its right operand instead. Explicitly
    // parenthesized assertion operands must still separate logical sequences.
    if matches!(
        node.kind(),
        "as_expression" | "satisfies_expression" | "non_null_expression"
    ) {
        let child = first_expression(node)?;
        let mut operand = child;
        while matches!(
            operand.kind(),
            "as_expression" | "satisfies_expression" | "non_null_expression"
        ) {
            operand = first_expression(operand)?;
        }
        if logical_operator(operand).is_some() {
            return Some(child);
        }
    }
    None
}

fn logical_operand(mut node: Node<'_>) -> Node<'_> {
    while let Some(child) = transparent_child(node) {
        node = child;
    }
    node
}

fn logical_complexity(root: Node<'_>) -> usize {
    // Only the root of a connected logical expression counts its operator runs.
    let parent = expression_parent(root);
    if parent.is_some_and(|p| logical_operator(p).is_some()) {
        return 0;
    }
    let mut pending = vec![(root, false)];
    let root_operator = logical_operator(root);
    let mut jsx_short_circuit = parent.is_some_and(|p| p.kind() == "jsx_expression");
    let mut previous = None;
    let mut count = 0;
    while let Some((node, visited)) = pending.pop() {
        let node = logical_operand(node);
        if let Some(operator) = logical_operator(node) {
            if visited {
                jsx_short_circuit &= Some(operator) == root_operator;
                if operator == "&&" && previous != Some(operator) {
                    count += 1;
                }
                previous = Some(operator);
            } else {
                if let Some(right) = node.child_by_field_name("right") {
                    pending.push((right, false));
                }
                pending.push((node, true));
                if let Some(left) = node.child_by_field_name("left") {
                    pending.push((left, false));
                }
            }
        } else if node.kind() == "ternary_expression" {
            jsx_short_circuit = false;
        }
    }
    if jsx_short_circuit { 0 } else { count }
}
