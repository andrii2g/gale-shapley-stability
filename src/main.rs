mod input;

use gale_shapley_stability::{
    MAX_EXHAUSTIVE_SIZE, Outcome, Profile, Run, Side, blocking_pairs, solve, verify_exhaustively,
    write_animated_svg, write_svg_frames,
};
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::{Duration, Instant};

const USAGE: &str = "Usage:
  gale-shapley-stability solve <profile> [--proposers left|right|both] [--summary]
      [--svg-dir <directory>] [--animated-svg <file>]
  gale-shapley-stability check <profile> <matching>

Solve runs with both sides proposing by default. Check reports blocking pairs
for a supplied complete matching.";

fn main() -> ExitCode {
    let arguments: Vec<_> = env::args().skip(1).collect();
    match run(&arguments) {
        Ok(code) => ExitCode::from(code),
        Err(message) => {
            eprintln!("error: {message}");
            ExitCode::from(2)
        }
    }
}

fn run(arguments: &[String]) -> Result<u8, String> {
    match arguments {
        [] => {
            println!("{USAGE}");
            Ok(0)
        }
        [flag] if flag == "--help" || flag == "-h" => {
            println!("{USAGE}");
            Ok(0)
        }
        [command, path, options @ ..] if command == "solve" => {
            let options = parse_solve_options(options)?;
            let profile = read_profile(path)?;
            solve_profile(&profile, &options)
        }
        [command, profile_path, matching_path] if command == "check" => {
            let profile = read_profile(profile_path)?;
            let matching = input::parse_matching(&read_text(matching_path)?, &profile)
                .map_err(|error| format!("{matching_path}: {error}"))?;
            print_matching(&profile, &matching);
            let pairs = blocking_pairs(&profile, &matching)?;
            print_blocking_pairs(&profile, &pairs);
            Ok(if pairs.is_empty() { 0 } else { 1 })
        }
        _ => Err(format!("invalid command\n\n{USAGE}")),
    }
}

struct SolveOptions {
    sides: &'static [Side],
    summary: bool,
    svg_directory: Option<PathBuf>,
    animated_svg: Option<PathBuf>,
}

fn parse_solve_options(arguments: &[String]) -> Result<SolveOptions, String> {
    let mut sides: &'static [Side] = &[Side::Left, Side::Right];
    let mut summary = false;
    let mut svg_directory = None;
    let mut animated_svg = None;
    let mut args = arguments.iter().map(String::as_str);
    while let Some(argument) = args.next() {
        match argument {
            "--summary" => summary = true,
            "--svg-dir" => {
                let directory = args
                    .next()
                    .filter(|directory| !directory.is_empty() && !directory.starts_with("--"))
                    .ok_or("--svg-dir expects a directory")?;
                svg_directory = Some(PathBuf::from(directory));
            }
            "--animated-svg" => {
                let file = args
                    .next()
                    .filter(|file| !file.is_empty() && !file.starts_with("--"))
                    .ok_or("--animated-svg expects a file")?;
                animated_svg = Some(PathBuf::from(file));
            }
            "--proposers" => {
                sides = match args.next() {
                    Some("left") => &[Side::Left],
                    Some("right") => &[Side::Right],
                    Some("both") => &[Side::Left, Side::Right],
                    _ => return Err("--proposers expects left, right, or both".to_owned()),
                };
            }
            _ => return Err(format!("unknown option {argument}")),
        }
    }
    Ok(SolveOptions {
        sides,
        summary,
        svg_directory,
        animated_svg,
    })
}

fn read_text(path: &str) -> Result<String, String> {
    fs::read_to_string(path).map_err(|error| format!("{path}: {error}"))
}

fn read_profile(path: &str) -> Result<Profile, String> {
    input::parse_profile(&read_text(path)?).map_err(|error| format!("{path}: {error}"))
}

fn solve_profile(profile: &Profile, options: &SolveOptions) -> Result<u8, String> {
    let mut runs: Vec<Run> = Vec::with_capacity(options.sides.len());
    let mut checks_passed = true;

    for &side in options.sides {
        let started = Instant::now();
        let run = solve(profile, side);
        let elapsed = started.elapsed();
        print_run(profile, &run, elapsed, options.summary);

        let pairs = blocking_pairs(profile, &run.matching)?;
        print_blocking_pairs(profile, &pairs);
        checks_passed &= pairs.is_empty();
        if profile.len() <= MAX_EXHAUSTIVE_SIZE {
            let verification = verify_exhaustively(profile, &run.matching, side)?;
            println!(
                "Exhaustive check: {} of {} matchings stable; result stable: {}; proposer-optimal: {}",
                verification.stable_matchings,
                verification.checked_matchings,
                yes_no(verification.is_stable),
                yes_no(verification.is_proposer_optimal),
            );
            checks_passed &= verification.is_stable && verification.is_proposer_optimal;
        } else {
            println!("Exhaustive check: available for up to {MAX_EXHAUSTIVE_SIZE} per side");
        }
        if let Some(directory) = &options.svg_directory {
            let directory = directory.join(side.to_string());
            write_svg_frames(&directory, profile, &run)
                .map_err(|error| format!("SVG export to {}: {error}", directory.display()))?;
            println!("SVG frames: {}", directory.display());
        }
        println!();

        if let Some(previous) = runs.last() {
            let changed = run
                .matching
                .iter()
                .zip(&previous.matching)
                .filter(|(a, b)| a != b)
                .count();
            println!(
                "Switching proposers changed {changed} of {} pairs.",
                profile.len()
            );
        }
        runs.push(run);
    }

    if let Some(file) = &options.animated_svg {
        write_animated_svg(file, profile, &runs)
            .map_err(|error| format!("Animated SVG export to {}: {error}", file.display()))?;
        println!("Animated SVG: {}", file.display());
    }

    Ok(if checks_passed { 0 } else { 3 })
}

fn print_run(profile: &Profile, run: &Run, elapsed: Duration, summary: bool) {
    println!("{} side proposes", run.proposer_side);
    if !summary {
        let proposers = profile.names(run.proposer_side);
        let receivers = profile.names(run.proposer_side.opposite());
        for event in &run.events {
            let detail = match event.outcome {
                Outcome::Held => "held".to_owned(),
                Outcome::Rejected => "rejected".to_owned(),
                Outcome::Replaced { previous } => {
                    format!("held, displacing {}", proposers[previous])
                }
            };
            println!(
                "  Round {}: {} -> {} ({detail})",
                event.round, proposers[event.proposer], receivers[event.receiver],
            );
        }
    }
    println!(
        "Proposals: {} | Rejections: {} | Rounds: {} | Solve: {:.3} ms",
        run.events.len(),
        run.rejections(),
        run.rounds,
        elapsed.as_secs_f64() * 1000.0,
    );
    print_matching(profile, &run.matching);
}

fn print_matching(profile: &Profile, matching: &[usize]) {
    let left_names = profile.names(Side::Left);
    let right_names = profile.names(Side::Right);
    let left_ranks = profile.ranks(Side::Left);
    let right_ranks = profile.ranks(Side::Right);
    println!("Matching (left rank / right rank, 1 = first choice)");
    for (left, &right) in matching.iter().enumerate() {
        println!(
            "  {} - {} ({}/{})",
            left_names[left],
            right_names[right],
            left_ranks[left][right] + 1,
            right_ranks[right][left] + 1,
        );
    }
}

fn print_blocking_pairs(profile: &Profile, pairs: &[(usize, usize)]) {
    println!("Blocking pairs: {}", pairs.len());
    for &(left, right) in pairs {
        println!(
            "  {} - {}",
            profile.names(Side::Left)[left],
            profile.names(Side::Right)[right],
        );
    }
}

fn yes_no(passed: bool) -> &'static str {
    if passed { "yes" } else { "no" }
}
