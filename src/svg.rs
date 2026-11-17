use std::fmt::Write as _;
use std::fs::{self, OpenOptions};
use std::io::{self, BufWriter, Write as _};
use std::path::Path;

use crate::{Event, Outcome, Profile, Run, Side};

const WIDTH: usize = 920;
const LEFT_X: usize = 250;
const RIGHT_X: usize = 670;
const FIRST_ROW: usize = 132;
const ROW_HEIGHT: usize = 44;
const TICKS_PER_SECOND: f64 = 4.0;
const INITIAL_TICKS: usize = 5;
const PROPOSAL_TICKS: usize = 6;
const FINAL_TICKS: usize = 11;

/// Writes the initial state, one frame per proposal, and the final matching.
pub fn write_svg_frames(directory: &Path, profile: &Profile, run: &Run) -> io::Result<()> {
    fs::create_dir_all(directory)?;
    let digits = run.events.len().to_string().len().max(4);

    replay_frames(profile, run, |frame, held| {
        let filename = match frame {
            Frame::Initial => format!("frame-{:0digits$}.svg", 0),
            Frame::Proposal(step, _) => format!("frame-{step:0digits$}.svg"),
            Frame::Final(_) => "final.svg".to_owned(),
        };
        new_svg_file(&directory.join(filename))?
            .write_all(render(profile, run.proposer_side, held, frame).as_bytes())
    })
}

/// Writes a looping SVG replay of the runs in order, using traces from this profile.
pub fn write_animated_svg(path: &Path, profile: &Profile, runs: &[Run]) -> io::Result<()> {
    if runs.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "the animation needs at least one run",
        ));
    }
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }
    let mut output = BufWriter::new(new_svg_file(path)?);
    let result = render_animation(&mut output, profile, runs).and_then(|()| output.flush());
    drop(output);
    if result.is_err() {
        let _ = fs::remove_file(path);
    }
    result
}

fn new_svg_file(path: &Path) -> io::Result<fs::File> {
    OpenOptions::new().write(true).create_new(true).open(path)
}

#[derive(Clone, Copy)]
enum Frame<'a> {
    Initial,
    Proposal(usize, &'a Event),
    Final(usize),
}

fn replay_frames(
    profile: &Profile,
    run: &Run,
    mut visit: impl FnMut(Frame<'_>, &[Option<usize>]) -> io::Result<()>,
) -> io::Result<()> {
    let mut held = vec![None; profile.len()];
    visit(Frame::Initial, &held)?;
    for (index, event) in run.events.iter().enumerate() {
        if event.outcome != Outcome::Rejected {
            held[event.receiver] = Some(event.proposer);
        }
        visit(Frame::Proposal(index + 1, event), &held)?;
    }
    visit(Frame::Final(run.rounds), &held)
}

fn canvas_height(profile: &Profile) -> usize {
    FIRST_ROW + ROW_HEIGHT * profile.len() + 46
}

fn render_animation(
    output: &mut impl io::Write,
    profile: &Profile,
    runs: &[Run],
) -> io::Result<()> {
    let height = canvas_height(profile);
    let description = animation_description(profile, runs);
    let mut svg = open_document(height, "Deferred acceptance replay", &description);
    svg.push_str("  <g id=\"replay\">\n");
    output.write_all(svg.as_bytes())?;

    let total_ticks: usize = runs
        .iter()
        .map(|run| INITIAL_TICKS + PROPOSAL_TICKS * run.events.len() + FINAL_TICKS)
        .sum();
    let duration = total_ticks as f64 / TICKS_PER_SECOND;
    let mut elapsed = 0;
    let mut index = 0;
    for run in runs {
        let side_heading = match run.proposer_side {
            Side::Left => "Left proposes",
            Side::Right => "Right proposes",
        };
        replay_frames(profile, run, |frame, held| {
            let ticks = match frame {
                Frame::Initial => INITIAL_TICKS,
                Frame::Proposal(..) => PROPOSAL_TICKS,
                Frame::Final(_) => FINAL_TICKS,
            };
            let start = elapsed as f64 / total_ticks as f64;
            let is_first = elapsed == 0;
            elapsed += ticks;
            let end = elapsed as f64 / total_ticks as f64;
            // Give every group the same loop duration. Its opacity is 1 only
            // during the interval assigned to that frame.
            let (opacity, values, key_times) = if is_first {
                (1, "1;0;0", format!("0;{end:.9};1"))
            } else if elapsed == total_ticks {
                (0, "0;1;1", format!("0;{start:.9};1"))
            } else {
                (0, "0;1;0;0", format!("0;{start:.9};{end:.9};1"))
            };
            writeln!(
                output,
                r#"    <g id="frame-{index}" opacity="{opacity}">
      <animate attributeName="opacity" dur="{duration}s" repeatCount="indefinite" calcMode="discrete" values="{values}" keyTimes="{key_times}" />"#
            )?;

            let (heading, description, caption) =
                frame_text(profile, run.proposer_side, held, frame);
            let heading = format!("{side_heading} · {heading}");
            writeln!(output, "      <desc>{}</desc>", xml_escape(&description))?;
            svg.clear();
            draw_frame(&mut svg, run.proposer_side, held, frame, &heading, &caption);
            for line in svg.lines() {
                writeln!(output, "    {line}")?;
            }
            writeln!(output, "    </g>")?;
            index += 1;
            Ok(())
        })?;
    }

    svg.clear();
    svg.push_str("  </g>\n");
    draw_diagram_labels(&mut svg, profile, height);
    svg.push_str("</svg>\n");
    output.write_all(svg.as_bytes())
}

fn animation_description(profile: &Profile, runs: &[Run]) -> String {
    let mut description = String::from(
        "Looping replay of the initial state, proposals, and final matching of each run.",
    );
    let left_names = profile.names(Side::Left);
    let right_names = profile.names(Side::Right);
    for run in runs {
        let _ = write!(
            description,
            " The {} side proposes. {} proposals and {} rejections. Final matching:",
            run.proposer_side,
            run.events.len(),
            run.rejections(),
        );
        for (left, &right) in run.matching.iter().enumerate() {
            let _ = write!(
                description,
                " {} with {}.",
                left_names[left], right_names[right]
            );
        }
    }
    description
}

fn render(
    profile: &Profile,
    proposer_side: Side,
    held: &[Option<usize>],
    frame: Frame<'_>,
) -> String {
    let height = canvas_height(profile);
    let (heading, mut description, caption) = frame_text(profile, proposer_side, held, frame);
    let proposers = profile.names(proposer_side);
    let receivers = profile.names(proposer_side.opposite());
    for (receiver, proposer) in held.iter().enumerate() {
        if let Some(proposer) = proposer {
            let _ = write!(
                description,
                " {} is paired with {}.",
                proposers[*proposer], receivers[receiver]
            );
        }
    }
    let mut svg = open_document(height, &heading, &description);
    draw_frame(&mut svg, proposer_side, held, frame, &heading, &caption);
    draw_diagram_labels(&mut svg, profile, height);
    svg.push_str("</svg>\n");
    svg
}

fn frame_text(
    profile: &Profile,
    proposer_side: Side,
    held: &[Option<usize>],
    frame: Frame<'_>,
) -> (String, String, String) {
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
    (
        heading,
        format!("The {proposer_side} side proposes. {description}"),
        caption,
    )
}

fn open_document(height: usize, heading: &str, description: &str) -> String {
    let heading = xml_escape(heading);
    let description = xml_escape(description);
    format!(
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
"##
    )
}

fn draw_frame(
    svg: &mut String,
    proposer_side: Side,
    held: &[Option<usize>],
    frame: Frame<'_>,
    heading: &str,
    caption: &str,
) {
    let heading = xml_escape(heading);
    let caption = xml_escape(caption);
    let _ = writeln!(
        svg,
        r#"  <text class="heading" x="32" y="40">{heading}</text>
  <text class="caption" x="32" y="70">{caption}</text>"#
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
            draw_pair(svg, proposer_side, *proposer, receiver, "held");
        }
    }
    if let Frame::Proposal(_, event) = frame {
        let class = match event.outcome {
            Outcome::Held => "accepted",
            Outcome::Rejected => "rejected",
            Outcome::Replaced { previous } => {
                draw_pair(svg, proposer_side, previous, event.receiver, "displaced");
                "accepted"
            }
        };
        draw_pair(svg, proposer_side, event.proposer, event.receiver, class);
    }
}

fn draw_diagram_labels(svg: &mut String, profile: &Profile, height: usize) {
    draw_people(svg, profile.names(Side::Left), LEFT_X, LEFT_X - 14, "end");
    draw_people(
        svg,
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
