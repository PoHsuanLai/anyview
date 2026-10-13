//! What a find is told besides its text: whether letter case and whole words matter. Text and PDF
//! search read the same options, and the viewer's find keeps one set for the window.

/// Whether letter case matters to a find.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum CaseMatch {
    /// `Fox` finds `fox` (the default).
    #[default]
    Ignore,
    /// `Fox` finds only `Fox`.
    Exact,
}

/// Whether a hit may be part of a longer word.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum WordMatch {
    /// `fox` finds the start of `foxes` (the default).
    #[default]
    Part,
    /// `fox` finds only the whole word.
    Whole,
}

/// The options a find offers, one for each of its toggles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FindOption {
    /// Letter case matters ([`CaseMatch::Exact`]).
    MatchCase,
    /// A hit is a whole word ([`WordMatch::Whole`]).
    WholeWords,
}

impl FindOption {
    /// Every option, in the order the find lists them.
    pub const ALL: [FindOption; 2] = [FindOption::MatchCase, FindOption::WholeWords];

    /// The words the palette lists the option by.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            FindOption::MatchCase => "Match Case",
            FindOption::WholeWords => "Whole Words",
        }
    }
}

/// How a find matches. It is `#[non_exhaustive]`, so an option added later breaks no caller: start
/// from [`FindOptions::default`] (ignore case, find parts of words) and change one with the
/// `with_*` methods.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct FindOptions {
    /// Whether letter case matters.
    pub case: CaseMatch,
    /// Whether a hit may be part of a longer word.
    pub words: WordMatch,
}

impl Default for FindOptions {
    fn default() -> Self {
        FindOptions::DEFAULT
    }
}

impl FindOptions {
    /// Ignore case and find parts of words: what [`Default`] answers, as a constant for a `const`
    /// that changes only some options.
    pub const DEFAULT: FindOptions = FindOptions {
        case: CaseMatch::Ignore,
        words: WordMatch::Part,
    };

    /// These options with `case`.
    #[must_use]
    pub const fn with_case(self, case: CaseMatch) -> Self {
        FindOptions { case, ..self }
    }

    /// These options with `words`.
    #[must_use]
    pub const fn with_words(self, words: WordMatch) -> Self {
        FindOptions { words, ..self }
    }

    /// Whether `option` is on.
    #[must_use]
    pub const fn has(self, option: FindOption) -> bool {
        match option {
            FindOption::MatchCase => matches!(self.case, CaseMatch::Exact),
            FindOption::WholeWords => matches!(self.words, WordMatch::Whole),
        }
    }

    /// These options with `option` switched: on if it was off, off if it was on.
    #[must_use]
    pub const fn toggled(self, option: FindOption) -> Self {
        match option {
            FindOption::MatchCase => self.with_case(match self.case {
                CaseMatch::Ignore => CaseMatch::Exact,
                CaseMatch::Exact => CaseMatch::Ignore,
            }),
            FindOption::WholeWords => self.with_words(match self.words {
                WordMatch::Part => WordMatch::Whole,
                WordMatch::Whole => WordMatch::Part,
            }),
        }
    }
}
