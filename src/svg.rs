use std::fmt::Write as _;
use std::fs::{self, OpenOptions};
use std::io::{self, Write as _};
use std::path::Path;

use crate::{Event, Outcome, Profile, Run, Side};

const WIDTH: usize = 920;
const LEFT_X: usize = 250;
const RIGHT_X: usize = 670;
const FIRST_ROW: usize = 132;
const ROW_HEIGHT: usize = 44;

/// Writes the initial state, one frame per proposal, and the final matching.
pub fn write_svg_frames(directory: &Path, profile: &Profile, run: &Run) -> io::Result<()> {
    fs::create_dir_all(directory)?;
    let mut held = vec![None; profile.names(run.proposer_side.opposite()).len()];
    let digits = run.events.len().to_string().len().max(4);

    write_frame(
        &directory.join(format!("frame-{:0digits$}.svg", 0)),
        &render(profile, run.proposer_side, &held, Frame::Initial),
    )?;

    for (index, event) in run.events.iter().enumerate() {
        if !matches!(event.outcome, Outcome::Rejected) {
            held[event.receiver] = Some(event.proposer);
        }
        let step = index + 1;
        write_frame(
            &directory.join(format!("frame-{step:0digits$}.svg")),
            &render(
                profile,
                run.proposer_side,
                &held,
                Frame::Proposal(step, event),
            ),
        )?;
    }

    write_frame(
        &directory.join("final.svg"),
        &render(profile, run.proposer_side, &held, Frame::Final(run.rounds)),
    )
}

fn write_frame(path: &Path, svg: &str) -> io::Result<()> {
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?
        .write_all(svg.as_bytes())
}

enum Frame<'a> {
    Initial,
    Proposal(usize, &'a Event),
    Final(usize),
}

fn render(
    profile: &Profile,
    proposer_side: Side,
    held: &[Option<usize>],
    frame: Frame<'_>,
) -> String {
    let height = FIRST_ROW + ROW_HEIGHT * held.len().max(1) + 46;
    let proposers = profile.names(proposer_side);
    let receivers = profile.names(proposer_side.opposite());
    let (heading, description, caption) = match frame {
        Frame::Initial => (
            "Initial state".to_owned(),
            "No proposals have been made.".to_owned(),
            "No proposals yet".to_owned(),
        ),
        Frame::Proposal(step, event) => {
            let proposal = format!(
                "{} → {}",
                label(&proposers[event.proposer]),
                label(&receivers[event.receiver])
            );
            let outcome = match event.outcome {
                Outcome::Held => "Held".to_owned(),
                Outcome::Rejected => "Rejected".to_owned(),
                Outcome::Replaced { previous } => {
                    format!("Replaced {}", label(&proposers[previous]))
                }
            };
            (
                format!("Round {} · Proposal {step}", event.round),
                format!(
                    "{} proposes to {}. {outcome}.",
                    proposers[event.proposer], receivers[event.receiver]
                ),
                format!("{proposal} · {outcome}"),
            )
        }
        Frame::Final(rounds) => (
            "Final matching".to_owned(),
            format!("The algorithm finished after {rounds} rounds."),
            format!(
                "{} matched pairs · {rounds} rounds",
                held.iter().flatten().count()
            ),
        ),
    };
    let mut description = format!("The {proposer_side} side proposes. {description}");
    for (receiver, proposer) in held.iter().enumerate() {
        if let Some(proposer) = proposer {
            let _ = write!(
                description,
                " {} is paired with {}.",
                proposers[*proposer], receivers[receiver]
            );
        }
    }
    let heading = xml_escape(&heading);
    let description = xml_escape(&description);
    let caption = xml_escape(&caption);
    let mut svg = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{WIDTH}" height="{height}" viewBox="0 0 {WIDTH} {height}" role="img" aria-labelledby="title description">
  <title id="title">{heading}</title>
  <desc id="description">{description}</desc>
  <style>
    text {{ fill: #16263f; font-family: system-ui, sans-serif; }}
    .heading {{ font-size: 24px; font-weight: 700; }}
    .caption {{ font-size: 15px; }}
    .group {{ fill: #465875; font-size: 13px; font-weight: 700; }}
    .name {{ font: 15px ui-monospace, monospace; }}
    .legend {{ font-size: 13px; }}
    .held, .accepted, .rejected, .displaced {{ fill: none; stroke-linecap: round; }}
    .held {{ stroke: #526681; stroke-width: 2; }}
    .accepted {{ stroke: #047857; stroke-width: 5; }}
    .rejected {{ stroke: #b91c1c; stroke-width: 4; stroke-dasharray: 10 6; }}
    .displaced {{ stroke: #925302; stroke-width: 4; stroke-dasharray: 1 7; }}
  </style>
  <rect width="{WIDTH}" height="{height}" rx="12" fill="#f8fafc" />
  <text class="heading" x="32" y="40">{heading}</text>
  <text class="caption" x="32" y="70">{caption}</text>
"##
    );
    for (side, name, x, anchor) in [
        (Side::Left, "Left", LEFT_X - 14, "end"),
        (Side::Right, "Right", RIGHT_X + 14, "start"),
    ] {
        let role = if side == proposer_side {
            "proposers"
        } else {
            "receivers"
        };
        let _ = writeln!(
            svg,
            r#"  <text class="group" x="{x}" y="103" text-anchor="{anchor}">{name} · {role}</text>"#
        );
    }

    for (receiver, proposer) in held.iter().enumerate() {
        if let Some(proposer) = proposer {
            draw_pair(&mut svg, proposer_side, *proposer, receiver, "held");
        }
    }
    if let Frame::Proposal(_, event) = frame {
        let class = match event.outcome {
            Outcome::Held => "accepted",
            Outcome::Rejected => "rejected",
            Outcome::Replaced { previous } => {
                draw_pair(
                    &mut svg,
                    proposer_side,
                    previous,
                    event.receiver,
                    "displaced",
                );
                "accepted"
            }
        };
        draw_pair(
            &mut svg,
            proposer_side,
            event.proposer,
            event.receiver,
            class,
        );
    }

    draw_people(
        &mut svg,
        profile.names(Side::Left),
        LEFT_X,
        LEFT_X - 14,
        "end",
    );
    draw_people(
        &mut svg,
        profile.names(Side::Right),
        RIGHT_X,
        RIGHT_X + 14,
        "start",
    );

    let legend_y = height - 24;
    for (x, class, text) in [
        (32, "held", "Tentative match"),
        (220, "accepted", "Accepted proposal"),
        (428, "rejected", "Rejected proposal"),
        (620, "displaced", "Displaced match"),
    ] {
        let line_end = x + 36;
        let text_x = x + 46;
        let text_y = legend_y + 4;
        let _ = writeln!(
            svg,
            r#"  <line class="{class}" x1="{x}" y1="{legend_y}" x2="{line_end}" y2="{legend_y}" />
  <text class="legend" x="{text_x}" y="{text_y}">{text}</text>"#
        );
    }
    svg.push_str("</svg>\n");
    svg
}

fn draw_pair(svg: &mut String, proposer_side: Side, proposer: usize, receiver: usize, class: &str) {
    let (left, right) = match proposer_side {
        Side::Left => (proposer, receiver),
        Side::Right => (receiver, proposer),
    };
    let left_y = FIRST_ROW + ROW_HEIGHT * left;
    let right_y = FIRST_ROW + ROW_HEIGHT * right;
    let _ = writeln!(
        svg,
        r#"  <line class="{class}" x1="{LEFT_X}" y1="{left_y}" x2="{RIGHT_X}" y2="{right_y}" />"#
    );
}

fn draw_people(svg: &mut String, names: &[String], node_x: usize, label_x: usize, anchor: &str) {
    for (index, name) in names.iter().enumerate() {
        let y = FIRST_ROW + ROW_HEIGHT * index;
        let label_y = y + 5;
        let full_name = xml_escape(name);
        let label = xml_escape(&label(name));
        let _ = writeln!(
            svg,
            r##"  <g>
    <title>{full_name}</title>
    <circle cx="{node_x}" cy="{y}" r="5" fill="#fff" stroke="#16263f" stroke-width="2" />
    <text class="name" x="{label_x}" y="{label_y}" text-anchor="{anchor}">{label}</text>
  </g>"##
        );
    }
}

fn label(name: &str) -> String {
    let mut chars = name.chars();
    let mut label: String = chars.by_ref().take(22).collect();
    if chars.next().is_some() {
        label.push('…');
    }
    label
}

fn xml_escape(text: &str) -> String {
    let mut escaped = String::new();
    for character in text.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&apos;"),
            '\u{fffe}' | '\u{ffff}' => escaped.push('�'),
            character if character.is_control() => escaped.push('�'),
            character => escaped.push(character),
        }
    }
    escaped
}

#[cfg(test)]
mod tests {
    use super::{Frame, label, render, xml_escape};
    use crate::{Event, Outcome, Profile, Side};

    #[test]
    fn protects_svg_text_and_handles_invalid_xml_characters() {
        assert_eq!(
            xml_escape("<script>Ā & \"'\0\u{ffff}"),
            "&lt;script&gt;Ā &amp; &quot;&apos;��"
        );
    }

    #[test]
    fn clips_long_names_at_character_boundaries() {
        assert_eq!(label(&"Å".repeat(23)), format!("{}…", "Å".repeat(22)));
    }

    #[test]
    fn keeps_left_and_right_positions_when_right_proposes() {
        let profile = Profile::new(
            vec!["A".into(), "B".into()],
            vec!["X".into(), "Y".into()],
            vec![vec!["X".into(), "Y".into()]; 2],
            vec![vec!["B".into(), "A".into()]; 2],
        )
        .unwrap();
        let event = Event {
            round: 1,
            proposer: 0,
            receiver: 1,
            outcome: Outcome::Held,
        };
        let svg = render(
            &profile,
            Side::Right,
            &[None, Some(0)],
            Frame::Proposal(1, &event),
        );
        assert!(svg.contains("X → B · Held"));
        assert!(svg.contains(r#"class="accepted" x1="250" y1="176" x2="670" y2="132""#));
    }
}
