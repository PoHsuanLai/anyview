//! The semantic class of a run of code: what it is, never what colour it is. The viewer maps a
//! class to a design-system colour token, because consumer code does not hard-code colours.

use ds_core::word::Word;

/// What a token of source code is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word)]
pub enum TokenClass {
    /// Not styled: identifiers, whitespace, anything the syntax gives no class.
    Plain,
    /// A keyword or storage word: `fn`, `if`, `pub`, `int`.
    Keyword,
    /// An operator: `+`, `==`, `->`.
    Operator,
    /// A string literal, its quotes included.
    String,
    /// An escape inside a string: `\n`.
    Escape,
    /// A comment.
    Comment,
    /// A number literal.
    Number,
    /// A named constant or language value: `true`, `null`, `PI`.
    Constant,
    /// A type, class, trait or interface name.
    Type,
    /// A function or method name.
    Function,
    /// A variable or parameter.
    Variable,
    /// Punctuation that is not an operator: brackets, commas, semicolons.
    Punctuation,
    /// An attribute, annotation or decorator.
    Attribute,
    /// A markup tag name.
    Tag,
    /// A heading in a markup language.
    Heading,
    /// Emphasised text in a markup language.
    Emphasis,
    /// Strongly emphasised text in a markup language.
    Strong,
    /// A link in a markup language.
    Link,
    /// A line a diff adds.
    Inserted,
    /// A line a diff removes.
    Deleted,
}

/// Scope prefix and class, most specific first. The first entry whose prefix is the scope or a
/// dotted ancestor of it decides.
const RULES: &[(&str, TokenClass)] = &[
    ("comment", TokenClass::Comment),
    ("punctuation.definition.comment", TokenClass::Comment),
    ("punctuation.definition.string", TokenClass::String),
    ("punctuation.definition.tag", TokenClass::Tag),
    ("punctuation.definition.annotation", TokenClass::Attribute),
    ("punctuation", TokenClass::Punctuation),
    ("string", TokenClass::String),
    ("constant.character.escape", TokenClass::Escape),
    ("constant.numeric", TokenClass::Number),
    ("constant", TokenClass::Constant),
    ("support.constant", TokenClass::Constant),
    ("keyword.operator", TokenClass::Operator),
    ("keyword", TokenClass::Keyword),
    ("storage", TokenClass::Keyword),
    ("entity.name.function", TokenClass::Function),
    ("support.function", TokenClass::Function),
    ("variable.function", TokenClass::Function),
    ("entity.name.tag", TokenClass::Tag),
    ("entity.other.attribute-name", TokenClass::Attribute),
    ("variable.annotation", TokenClass::Attribute),
    ("entity.name.type", TokenClass::Type),
    ("entity.name.class", TokenClass::Type),
    ("entity.name.struct", TokenClass::Type),
    ("entity.name.enum", TokenClass::Type),
    ("entity.name.trait", TokenClass::Type),
    ("entity.name.interface", TokenClass::Type),
    ("entity.other.inherited-class", TokenClass::Type),
    ("support.type", TokenClass::Type),
    ("support.class", TokenClass::Type),
    ("variable", TokenClass::Variable),
    ("markup.heading", TokenClass::Heading),
    ("markup.bold", TokenClass::Strong),
    ("markup.italic", TokenClass::Emphasis),
    ("markup.underline.link", TokenClass::Link),
    ("markup.inserted", TokenClass::Inserted),
    ("markup.deleted", TokenClass::Deleted),
    ("markup.raw", TokenClass::String),
    ("markup.quote", TokenClass::Comment),
];

/// Whether `prefix` is `scope` or one of its dotted ancestors (`keyword` is an ancestor of
/// `keyword.control.rust`; `key` is not).
fn is_ancestor(prefix: &str, scope: &str) -> bool {
    scope
        .strip_prefix(prefix)
        .is_some_and(|rest| rest.is_empty() || rest.starts_with('.'))
}

/// The class of one scope name such as `keyword.control.rust`, or `None` when it names no token
/// (`source.rust`, `meta.function.rust`).
pub(super) fn class_of(scope: &str) -> Option<TokenClass> {
    RULES
        .iter()
        .find(|(prefix, _)| is_ancestor(prefix, scope))
        .map(|(_, class)| *class)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scopes_map_to_classes_at_dot_boundaries() {
        // name, scope, class
        const CASES: &[(&str, &str, Option<TokenClass>)] = &[
            (
                "control keyword",
                "keyword.control.rust",
                Some(TokenClass::Keyword),
            ),
            (
                "operator before keyword",
                "keyword.operator.arithmetic.rust",
                Some(TokenClass::Operator),
            ),
            (
                "integer",
                "constant.numeric.integer.decimal.rust",
                Some(TokenClass::Number),
            ),
            (
                "escape before constant",
                "constant.character.escape.rust",
                Some(TokenClass::Escape),
            ),
            (
                "language constant",
                "constant.language.rust",
                Some(TokenClass::Constant),
            ),
            (
                "string",
                "string.quoted.double.rust",
                Some(TokenClass::String),
            ),
            (
                "string quote is a string",
                "punctuation.definition.string.begin.rust",
                Some(TokenClass::String),
            ),
            (
                "comment marker is a comment",
                "punctuation.definition.comment.rust",
                Some(TokenClass::Comment),
            ),
            (
                "separator",
                "punctuation.separator.rust",
                Some(TokenClass::Punctuation),
            ),
            (
                "comment",
                "comment.line.double-slash.rust",
                Some(TokenClass::Comment),
            ),
            (
                "function name",
                "entity.name.function.rust",
                Some(TokenClass::Function),
            ),
            (
                "fn is storage",
                "storage.type.function.rust",
                Some(TokenClass::Keyword),
            ),
            (
                "type name",
                "entity.name.type.struct.rust",
                Some(TokenClass::Type),
            ),
            ("support type", "support.type.rust", Some(TokenClass::Type)),
            ("tag", "entity.name.tag.html", Some(TokenClass::Tag)),
            (
                "attribute",
                "entity.other.attribute-name.html",
                Some(TokenClass::Attribute),
            ),
            (
                "variable",
                "variable.parameter.rust",
                Some(TokenClass::Variable),
            ),
            (
                "heading",
                "markup.heading.markdown",
                Some(TokenClass::Heading),
            ),
            (
                "inserted",
                "markup.inserted.diff",
                Some(TokenClass::Inserted),
            ),
            ("a source scope names no token", "source.rust", None),
            ("a meta scope names no token", "meta.function.rust", None),
            (
                "a longer first atom is not a prefix match",
                "keywordish.rust",
                None,
            ),
            ("a shorter atom is not a match", "str.rust", None),
        ];
        for (name, scope, want) in CASES {
            assert_eq!(class_of(scope), *want, "{name}");
        }
    }

    #[test]
    fn class_slugs_are_the_names_a_stylesheet_maps_to_colour_tokens() {
        let slugs: Vec<&str> = TokenClass::ALL.iter().map(|class| class.slug()).collect();
        for slug in &slugs {
            assert!(
                !slug.is_empty() && slug.chars().all(|c| c.is_ascii_lowercase() || c == '-'),
                "{slug}"
            );
        }
        let mut sorted = slugs.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), slugs.len(), "no slug repeats");
        assert_eq!(TokenClass::Keyword.slug(), "keyword");
        assert_eq!(TokenClass::Plain.slug(), "plain");
    }
}
