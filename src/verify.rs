use crate::{Profile, Side};

pub const MAX_EXHAUSTIVE_SIZE: usize = 5;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Verification {
    pub checked_matchings: usize,
    pub stable_matchings: usize,
    pub is_stable: bool,
    pub is_proposer_optimal: bool,
}

pub fn blocking_pairs(
    profile: &Profile,
    matching: &[usize],
) -> Result<Vec<(usize, usize)>, String> {
    let inverse = validate_and_invert(profile.len(), matching)?;
    let left_ranks = profile.ranks(Side::Left);
    let right_ranks = profile.ranks(Side::Right);
    let mut pairs = Vec::new();

    for (left, &current_right) in matching.iter().enumerate() {
        for (right, &current_left) in inverse.iter().enumerate() {
            if left_ranks[left][right] < left_ranks[left][current_right]
                && right_ranks[right][left] < right_ranks[right][current_left]
            {
                pairs.push((left, right));
            }
        }
    }

    Ok(pairs)
}

pub fn verify_exhaustively(
    profile: &Profile,
    matching: &[usize],
    proposer_side: Side,
) -> Result<Verification, String> {
    let size = profile.len();
    if size > MAX_EXHAUSTIVE_SIZE {
        return Err(format!(
            "exhaustive verification supports at most {MAX_EXHAUSTIVE_SIZE} participants per side"
        ));
    }
    let inverse = validate_and_invert(size, matching)?;
    let left_preferences = profile.preferences(Side::Left);
    let right_preferences = profile.preferences(Side::Right);
    let proposer_preferences = profile.preferences(proposer_side);
    let mut result = Verification {
        checked_matchings: 0,
        stable_matchings: 0,
        is_stable: false,
        is_proposer_optimal: false,
    };
    let mut best_ranks = vec![usize::MAX; size];
    let mut permutation: Vec<_> = (0..size).collect();

    enumerate_permutations(&mut permutation, 0, &mut |candidate| {
        result.checked_matchings += 1;
        let mut candidate_inverse = vec![0; size];
        for (left, &right) in candidate.iter().enumerate() {
            candidate_inverse[right] = left;
        }

        // Read the original preference order to keep this check independent of
        // the inverse rank tables used by deferred acceptance and blocking_pairs.
        let has_blocking_pair = candidate.iter().enumerate().any(|(left, &partner)| {
            left_preferences[left]
                .iter()
                .take_while(|&&right| right != partner)
                .any(|&right| {
                    right_preferences[right]
                        .iter()
                        .take_while(|&&other_left| other_left != candidate_inverse[right])
                        .any(|&other_left| other_left == left)
                })
        });
        if has_blocking_pair {
            return;
        }

        result.stable_matchings += 1;
        result.is_stable |= candidate == matching;
        let partners = match proposer_side {
            Side::Left => candidate,
            Side::Right => &candidate_inverse,
        };
        for (proposer, &partner) in partners.iter().enumerate() {
            best_ranks[proposer] =
                best_ranks[proposer].min(rank(proposer_preferences, proposer, partner));
        }
    });

    let current_partners = match proposer_side {
        Side::Left => matching,
        Side::Right => &inverse,
    };
    result.is_proposer_optimal = result.is_stable
        && current_partners
            .iter()
            .enumerate()
            .all(|(proposer, &partner)| {
                rank(proposer_preferences, proposer, partner) == best_ranks[proposer]
            });
    Ok(result)
}

fn validate_and_invert(size: usize, matching: &[usize]) -> Result<Vec<usize>, String> {
    if matching.len() != size {
        return Err(format!("matching must contain exactly {size} pairs"));
    }

    let mut inverse = vec![usize::MAX; size];
    for (left, &right) in matching.iter().enumerate() {
        let partner = inverse
            .get_mut(right)
            .ok_or_else(|| format!("right participant index {right} is out of range"))?;
        if *partner != usize::MAX {
            return Err(format!("right participant index {right} is matched twice"));
        }
        *partner = left;
    }
    Ok(inverse)
}

fn rank(preferences: &[Vec<usize>], person: usize, partner: usize) -> usize {
    preferences[person]
        .iter()
        .position(|&other| other == partner)
        .expect("a complete preference list includes every partner")
}

fn enumerate_permutations(values: &mut [usize], fixed: usize, visit: &mut impl FnMut(&[usize])) {
    if fixed == values.len() {
        visit(values);
        return;
    }

    for index in fixed..values.len() {
        values.swap(fixed, index);
        enumerate_permutations(values, fixed + 1, visit);
        values.swap(fixed, index);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_profile() -> Profile {
        let names = |row: &str| row.split_whitespace().map(str::to_owned).collect();
        Profile::new(
            names("A B C"),
            names("X Y Z"),
            ["X Y Z", "X Y Z", "Y Z X"].into_iter().map(names).collect(),
            ["C A B", "A B C", "A B C"].into_iter().map(names).collect(),
        )
        .unwrap()
    }

    #[test]
    fn finds_all_blocking_pairs_in_left_then_right_order() {
        let profile = sample_profile();
        assert_eq!(
            blocking_pairs(&profile, &[2, 0, 1]).unwrap(),
            [(0, 0), (0, 1)]
        );
        assert!(blocking_pairs(&profile, &[0, 1, 2]).unwrap().is_empty());
        assert!(blocking_pairs(&profile, &[1, 2, 0]).unwrap().is_empty());
    }

    #[test]
    fn checks_stability_and_optimality_for_each_proposing_side() {
        let profile = sample_profile();
        for (matching, optimal_side) in [([0, 1, 2], Side::Left), ([1, 2, 0], Side::Right)] {
            for proposer_side in [Side::Left, Side::Right] {
                let result = verify_exhaustively(&profile, &matching, proposer_side).unwrap();
                assert_eq!(result.checked_matchings, 6);
                assert_eq!(result.stable_matchings, 2);
                assert!(result.is_stable);
                assert_eq!(result.is_proposer_optimal, proposer_side == optimal_side);
            }
        }
        let unstable = verify_exhaustively(&profile, &[2, 0, 1], Side::Left).unwrap();
        assert!(!unstable.is_stable);
        assert!(!unstable.is_proposer_optimal);
    }

    #[test]
    fn rejects_invalid_matchings() {
        let profile = sample_profile();
        for matching in [&[0, 1][..], &[0, 1, 3], &[0, 1, 0]] {
            assert!(blocking_pairs(&profile, matching).is_err());
            assert!(verify_exhaustively(&profile, matching, Side::Left).is_err());
        }
    }

    #[test]
    fn enumerates_every_matching_at_the_limit_and_rejects_larger_groups() {
        for size in [MAX_EXHAUSTIVE_SIZE, MAX_EXHAUSTIVE_SIZE + 1] {
            let left: Vec<_> = (0..size).map(|index| format!("L{index}")).collect();
            let right: Vec<_> = (0..size).map(|index| format!("R{index}")).collect();
            let profile = Profile::new(
                left.clone(),
                right.clone(),
                vec![right; size],
                vec![left; size],
            )
            .unwrap();
            let identity: Vec<_> = (0..size).collect();
            let result = verify_exhaustively(&profile, &identity, Side::Left);
            if size == MAX_EXHAUSTIVE_SIZE {
                let result = result.unwrap();
                assert_eq!(result.checked_matchings, 120);
                assert_eq!(result.stable_matchings, 1);
                assert!(result.is_proposer_optimal);
            } else {
                assert!(result.is_err());
            }
        }
    }
}
