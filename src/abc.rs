use self::InfoFieldOrder::{First, Last, Middle, Second};
use crate::{
    score::{convert_midi_note_ons, smf_to_events, ScoreNote, ZERO_U7},
    ScoreVec,
};
use abc_parser::{abc, datatypes::Tune};
use abc_to_midi::midly_wrappers::Smf;
use anyhow::Result;
use std::{
    cmp::Ordering,
    collections::BTreeMap,
    fmt::{Display, Write},
    time::Duration,
};

/// Makes a copy of the score events changing all non-zero velocities to 100
/// and rounding timestamps down to the closest millisecond. This is useful
/// for simplifying tests when using Scores converted from ABC.
pub fn simplify_score(score: ScoreVec) -> ScoreVec {
    score
        .iter()
        .map(
            |ScoreNote {
                 time,
                 pitch,
                 velocity,
             }| ScoreNote {
                time: Duration::from_millis(time.as_millis() as u64),
                pitch: *pitch,
                velocity: if *velocity == ZERO_U7 {
                    ZERO_U7
                } else {
                    100.into()
                },
            },
        )
        .collect::<ScoreVec>()
}

#[derive(Debug, Eq, Hash, PartialEq)]
pub struct OrderedInfoFieldName(char);

#[derive(Ord, PartialOrd, PartialEq, Eq)]
pub enum InfoFieldOrder {
    First = 0,
    Second,
    Middle,
    Last,
}

impl OrderedInfoFieldName {
    fn decorate(&self) -> (InfoFieldOrder, char) {
        match self.0 {
            'X' => (First, 'X'),
            'T' => (Second, 'T'),
            'K' => (Last, 'K'),
            c => (Middle, c),
        }
    }
}
impl Display for OrderedInfoFieldName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_char(self.0)
    }
}

impl PartialOrd for OrderedInfoFieldName {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for OrderedInfoFieldName {
    fn cmp(&self, other: &Self) -> Ordering {
        self.decorate().cmp(&other.decorate())
    }
}

peg::parser! {
pub grammar abc_header() for str {
    // copied private rule from `abc_parser::grammar::abc`
    rule optional_space()
        = [' ' | '\t']*

    // adapted private rule from `abc_parser::grammar::abc`
    rule info_field<'a>(n: rule<&'a str>) -> (OrderedInfoFieldName, String)
        = name:n() ":" optional_space() t:$((!"\n"[_])*) "\n" {
        (
            OrderedInfoFieldName(name.chars().next().unwrap()),
            t.to_string()
        )
    }

    // adapted private rule from `abc_parser::grammar::abc`
    rule info_field_any() -> (OrderedInfoFieldName, String)
        = info_field(<$(['A'..='Z' | 'a'..='z'])>)

    #[no_eof]
    pub rule headers() -> BTreeMap<OrderedInfoFieldName, String>
        = fields:info_field_any()* {
            let mut h = BTreeMap::<OrderedInfoFieldName, String>::from_iter(fields);
            h.entry(OrderedInfoFieldName('X')).or_insert_with(|| "1".to_string());
            h.entry(OrderedInfoFieldName('T')).or_insert_with(|| "test tune".to_string());
            h.entry(OrderedInfoFieldName('K')).or_insert_with(|| "C".to_string());
            h
        }
    }
}

/// Converts an ABC formatted music notation string into a Selim score.
/// The headers required by ABC may be omitted, in which case they are replaced with these defaults:
/// ```abc
/// X: 1
/// T: test_tune
/// K: C
/// ```
pub fn abc_into_score(music: &str) -> Result<ScoreVec> {
    let headers = abc_header::headers(music)?;
    let mut abc_with_required_headers = String::new();
    for (name, value) in headers.iter() {
        writeln!(abc_with_required_headers, "{name}: {value}")?;
    }
    abc_with_required_headers.push_str(music);
    let tune: Tune = abc::tune(&abc_with_required_headers).unwrap();
    let smf = Smf::try_from_tune(&tune).unwrap();
    let events = smf_to_events(&smf.0, vec![]);
    Ok(simplify_score(convert_midi_note_ons(events)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use midly::num::u7; // required by the `notes!` macro
    use rstest::rstest;
    use std::cmp::Ordering::{Equal, Greater, Less};

    #[test]
    fn test_abc_into_score() {
        let score = abc_into_score("CDE").unwrap();
        assert_eq!(score, notes![(1, 60), (251, 62), (501, 64)]);
    }

    #[rstest(
        left, right, expect,

        case('I', 'X', Greater), // 'X' is always first
        case('K', 'X', Greater),
        case('O', 'X', Greater),
        case('T', 'X', Greater),
        case('X', 'X', Equal),
        case('Z', 'X', Greater),

        case('I', 'T', Greater), // 'T' is second after 'X'
        case('K', 'T', Greater), // 'K' is last
        case('O', 'T', Greater),
        case('T', 'T', Equal),
        case('X', 'T', Less), // 'X' is first just before 'T'
        case('Z', 'T', Greater),

        case('I', 'K', Less), // 'K' is last
        case('K', 'K', Equal),
        case('O', 'K', Less),
        case('T', 'K', Less),
        case('X', 'K', Less),
        case('Z', 'K', Less),

        case('I', 'O', Less), // Other field names are sorted alphabetically
        case('O', 'I', Greater),
        case('I', 'I', Equal),
    )]
    fn ordered_info_field_name_sorting(left: char, right: char, expect: Ordering) {
        let result = OrderedInfoFieldName(left).cmp(&OrderedInfoFieldName(right));
        assert_eq!(result, expect);
    }
}
