use crate::{Profile, Side};
use std::collections::VecDeque;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Outcome {
    Held,
    Rejected,
    /// The receiver drops its incumbent proposer in favor of the new one.
    Replaced {
        previous: usize,
    },
}

/// Participant IDs are relative to their role in this run.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Event {
    pub round: usize,
    pub proposer: usize,
    pub receiver: usize,
    pub outcome: Outcome,
}

#[derive(Debug)]
pub struct Run {
    pub proposer_side: Side,
    /// The final right-side partner ID for each left-side participant, in input order.
    pub matching: Vec<usize>,
    pub events: Vec<Event>,
    pub rounds: usize,
}

impl Run {
    /// Counts both immediate rejections and displaced incumbents.
    pub fn rejections(&self) -> usize {
        self.events
            .iter()
            .filter(|event| event.outcome != Outcome::Held)
            .count()
    }
}

/// A round gives each proposer free at its start exactly one turn. Within a
/// round, proposals are resolved immediately and newly free proposers wait.
pub fn solve(profile: &Profile, proposer_side: Side) -> Run {
    let size = profile.len();
    let preferences = profile.preferences(proposer_side);
    let receiver_ranks = profile.ranks(proposer_side.opposite());
    let mut free: VecDeque<_> = (0..size).collect();
    let mut next_choice = vec![0; size];
    let mut held: Vec<Option<usize>> = vec![None; size];
    let mut events = Vec::new();
    let mut rounds = 0;

    while !free.is_empty() {
        rounds += 1;
        let turns = free.len();
        for _ in 0..turns {
            let proposer = free.pop_front().expect("a proposer for this turn");
            let receiver = preferences[proposer][next_choice[proposer]];
            next_choice[proposer] += 1;

            let outcome = match held[receiver] {
                None => {
                    held[receiver] = Some(proposer);
                    Outcome::Held
                }
                Some(previous)
                    if receiver_ranks[receiver][proposer] < receiver_ranks[receiver][previous] =>
                {
                    held[receiver] = Some(proposer);
                    free.push_back(previous);
                    Outcome::Replaced { previous }
                }
                Some(_) => {
                    free.push_back(proposer);
                    Outcome::Rejected
                }
            };
            events.push(Event {
                round: rounds,
                proposer,
                receiver,
                outcome,
            });
        }
    }

    // Complete preferences and equal group sizes guarantee every receiver is held.
    let mut matching = vec![0; size];
    for (receiver, proposer) in held.into_iter().enumerate() {
        let proposer = proposer.expect("every receiver has a partner");
        match proposer_side {
            Side::Left => matching[proposer] = receiver,
            Side::Right => matching[receiver] = proposer,
        }
    }

    Run {
        proposer_side,
        matching,
        events,
        rounds,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(line: &str) -> Vec<String> {
        line.split_whitespace().map(str::to_owned).collect()
    }

    #[test]
    fn switches_partners_and_records_immediate_and_deferred_rejections() {
        let profile = Profile::new(
            words("A B C"),
            words("X Y Z"),
            ["X Y Z", "X Y Z", "Y Z X"].map(words).to_vec(),
            ["C A B", "A B C", "A B C"].map(words).to_vec(),
        )
        .unwrap();

        let left = solve(&profile, Side::Left);
        assert_eq!(left.matching, [0, 1, 2]);
        assert_eq!(left.rounds, 3);
        assert_eq!(left.events.len(), 5);
        assert_eq!(left.rejections(), 2);
        assert_eq!(left.events[1].outcome, Outcome::Rejected);
        assert_eq!(left.events[3].outcome, Outcome::Replaced { previous: 2 });
        assert_eq!(left.events[4].round, 3);

        let right = solve(&profile, Side::Right);
        assert_eq!(right.matching, [1, 2, 0]);
        assert_eq!(right.rounds, 2);
        assert_eq!(right.events.len(), 4);
        assert_eq!(right.rejections(), 1);
    }
}
