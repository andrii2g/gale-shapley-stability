use gale_shapley_stability::{Profile, Side};

pub fn parse_profile(contents: &str) -> Result<Profile, String> {
    let mut groups: [Vec<(String, Vec<String>)>; 2] = [Vec::new(), Vec::new()];
    let mut section = None;
    let mut seen = [false; 2];

    for (line, text) in input_lines(contents) {
        let header = match text {
            "[left]" => Some(0),
            "[right]" => Some(1),
            _ => None,
        };
        if let Some(index) = header {
            if seen[index] {
                return Err(format!("line {line}: repeated section {text}"));
            }
            seen[index] = true;
            section = Some(index);
            continue;
        }

        let index = section.ok_or_else(|| format!("line {line}: expected [left] or [right]"))?;
        let (name, choices) = parse_row(line, text)?;
        groups[index].push((
            name.to_owned(),
            choices.split_whitespace().map(str::to_owned).collect(),
        ));
    }

    if !seen.into_iter().all(|section| section) {
        return Err("expected both [left] and [right] sections".to_owned());
    }
    let [left, right] = groups;
    let (left_names, left_preferences) = left.into_iter().unzip();
    let (right_names, right_preferences) = right.into_iter().unzip();
    Profile::new(left_names, right_names, left_preferences, right_preferences)
}

pub fn parse_matching(contents: &str, profile: &Profile) -> Result<Vec<usize>, String> {
    let left_names = profile.names(Side::Left);
    let right_names = profile.names(Side::Right);
    let mut matching = vec![None; profile.len()];
    let mut used_right = vec![false; profile.len()];

    for (line, text) in input_lines(contents) {
        let (left, right) = parse_row(line, text)?;
        let left_id = left_names
            .iter()
            .position(|name| name == left)
            .ok_or_else(|| format!("line {line}: unknown left participant {left}"))?;
        let right_id = right_names
            .iter()
            .position(|name| name == right)
            .ok_or_else(|| format!("line {line}: unknown right participant {right}"))?;
        if matching[left_id].is_some() || used_right[right_id] {
            return Err(format!(
                "line {line}: participants can appear in only one pair"
            ));
        }
        matching[left_id] = Some(right_id);
        used_right[right_id] = true;
    }

    matching
        .into_iter()
        .enumerate()
        .map(|(left_id, right)| {
            right.ok_or_else(|| format!("missing partner for {}", left_names[left_id]))
        })
        .collect()
}

fn input_lines(contents: &str) -> impl Iterator<Item = (usize, &str)> {
    contents
        .trim_start_matches('\u{feff}')
        .lines()
        .enumerate()
        .filter_map(|(index, text)| {
            let text = text
                .split_once('#')
                .map_or(text, |(before, _)| before)
                .trim();
            (!text.is_empty()).then_some((index + 1, text))
        })
}

fn parse_row(line: usize, text: &str) -> Result<(&str, &str), String> {
    let (name, values) = text
        .split_once(':')
        .ok_or_else(|| format!("line {line}: expected a participant followed by ':'"))?;
    let name = name.trim();
    let values = values.trim();
    if name.is_empty() || name.chars().any(char::is_whitespace) || values.contains(':') {
        return Err(format!(
            "line {line}: invalid participant or preference row"
        ));
    }
    Ok((name, values))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_ordered_utf8_rows_and_rejects_incomplete_files() {
        let profile = parse_profile(
            "\u{feff}[right]\r\nY: B Á # first row\r\nX: Á B\r\n\r\n[left]\r\nÁ: X Y\r\nB: Y X\r\n",
        )
        .unwrap();
        assert_eq!(profile.names(Side::Right), ["Y", "X"]);
        assert_eq!(parse_matching("B: Y\nÁ: X", &profile).unwrap(), [1, 0]);
        assert!(parse_matching("B: Y\nÁ: Y", &profile).is_err());
        assert!(parse_matching("B: Y", &profile).is_err());
        assert!(parse_profile("[left]\nA: X").is_err());
        assert!(parse_profile("[left]\nA: X\n[left]\n[right]\nX: A").is_err());
    }
}
