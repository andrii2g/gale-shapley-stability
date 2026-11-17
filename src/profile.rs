use std::collections::HashMap;
use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Side {
    Left,
    Right,
}

impl Side {
    pub const fn opposite(self) -> Self {
        match self {
            Self::Left => Self::Right,
            Self::Right => Self::Left,
        }
    }

    const fn index(self) -> usize {
        match self {
            Self::Left => 0,
            Self::Right => 1,
        }
    }
}

impl fmt::Display for Side {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Left => "left",
            Self::Right => "right",
        })
    }
}

#[derive(Debug)]
struct Group {
    names: Vec<String>,
    preferences: Vec<Vec<usize>>,
    ranks: Vec<Vec<usize>>,
}

/// Two equally sized groups with complete strict preferences over one another.
#[derive(Debug)]
pub struct Profile {
    groups: [Group; 2],
}

impl Profile {
    /// The preference rows follow the order of the participant names on each side.
    pub fn new(
        left: Vec<String>,
        right: Vec<String>,
        left_preferences: Vec<Vec<String>>,
        right_preferences: Vec<Vec<String>>,
    ) -> Result<Self, String> {
        if left.is_empty() || left.len() != right.len() {
            return Err("the two groups must have the same positive size".into());
        }

        let left_names = index_names(Side::Left, &left)?;
        let right_names = index_names(Side::Right, &right)?;
        let (left_preferences, left_ranks) =
            encode_preferences(Side::Left, &left, &right_names, &left_preferences)?;
        let (right_preferences, right_ranks) =
            encode_preferences(Side::Right, &right, &left_names, &right_preferences)?;

        Ok(Self {
            groups: [
                Group {
                    names: left,
                    preferences: left_preferences,
                    ranks: left_ranks,
                },
                Group {
                    names: right,
                    preferences: right_preferences,
                    ranks: right_ranks,
                },
            ],
        })
    }

    pub fn len(&self) -> usize {
        self.groups[0].names.len()
    }

    pub fn is_empty(&self) -> bool {
        self.groups[0].names.is_empty()
    }

    pub fn names(&self, side: Side) -> &[String] {
        &self.groups[side.index()].names
    }

    /// Each row lists opposite-side participant IDs in order of preference.
    pub fn preferences(&self, side: Side) -> &[Vec<usize>] {
        &self.groups[side.index()].preferences
    }

    /// Each row maps opposite-side participant IDs to ranks, with zero as best.
    pub fn ranks(&self, side: Side) -> &[Vec<usize>] {
        &self.groups[side.index()].ranks
    }
}

fn index_names(side: Side, names: &[String]) -> Result<HashMap<&str, usize>, String> {
    let mut indices = HashMap::with_capacity(names.len());
    for (index, name) in names.iter().enumerate() {
        if name.trim().is_empty()
            || name.chars().any(|character| {
                character.is_control() || matches!(character, '\u{fffe}' | '\u{ffff}')
            })
        {
            return Err(format!(
                "invalid participant name on the {side} side: {name:?}"
            ));
        }
        if indices.insert(name.as_str(), index).is_some() {
            return Err(format!("duplicate participant on the {side} side: {name}"));
        }
    }
    Ok(indices)
}

type PreferenceMatrices = (Vec<Vec<usize>>, Vec<Vec<usize>>);

fn encode_preferences(
    side: Side,
    names: &[String],
    opposite_names: &HashMap<&str, usize>,
    rows: &[Vec<String>],
) -> Result<PreferenceMatrices, String> {
    let size = names.len();
    if rows.len() != size {
        return Err(format!(
            "expected {size} preference rows for the {side} side"
        ));
    }

    let mut preferences = Vec::with_capacity(size);
    let mut ranks = Vec::with_capacity(size);
    for (name, row) in names.iter().zip(rows) {
        if row.len() != size {
            return Err(format!(
                "{side} participant {name} must rank all {size} opposite-side participants"
            ));
        }

        let mut choices = Vec::with_capacity(size);
        let mut rank_by_choice = vec![usize::MAX; size];
        for (rank, choice) in row.iter().enumerate() {
            let &choice_id = opposite_names.get(choice.as_str()).ok_or_else(|| {
                format!("{side} participant {name} ranks unknown participant {choice}")
            })?;
            if rank_by_choice[choice_id] != usize::MAX {
                return Err(format!(
                    "{side} participant {name} ranks {choice} more than once"
                ));
            }
            choices.push(choice_id);
            rank_by_choice[choice_id] = rank;
        }
        preferences.push(choices);
        ranks.push(rank_by_choice);
    }

    Ok((preferences, ranks))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(words: &str) -> Vec<String> {
        words.split_whitespace().map(str::to_owned).collect()
    }

    #[test]
    fn rejects_incomplete_duplicate_and_unknown_rankings() {
        for bad_row in ["X", "X X", "X Z"] {
            let result = Profile::new(
                strings("A B"),
                strings("X Y"),
                vec![strings(bad_row), strings("X Y")],
                vec![strings("A B"), strings("A B")],
            );
            assert!(result.is_err(), "{bad_row} should fail validation");
        }
    }

    #[test]
    fn rejects_empty_unequal_and_duplicate_groups() {
        for (left, right) in [("", ""), ("A", "X Y"), ("A A", "X Y"), ("A B", "X X")] {
            let result = Profile::new(strings(left), strings(right), vec![], vec![]);
            assert!(
                result.is_err(),
                "{left:?} and {right:?} should fail validation"
            );
        }
    }
}
