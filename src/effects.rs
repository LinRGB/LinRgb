use crate::drivers::{Interface, Lane, Target, Ticket};
use std::{
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::{Duration, Instant},
};

#[derive(Clone, Copy)]
pub enum Effect {
    Solid([u8; 3]),
    Fade([u8; 3], [u8; 3], f64),
    Breathing([u8; 3], Option<[u8; 3]>, f64),
    ColorCycle(f64, f64),
}

impl Effect {
    pub fn from_inputs(
        mode: u32,
        first: &str,
        second: &str,
        alternate: bool,
        saturation: f64,
        seconds: f64,
    ) -> Option<Self> {
        use crate::color_wheel::parse_hex;
        if !(2.0..=60.0).contains(&seconds) || !(0.0..=100.0).contains(&saturation) {
            return None;
        }
        match mode {
            0 => Some(Self::Solid(parse_hex(first)?)),
            1 => Some(Self::Fade(parse_hex(first)?, parse_hex(second)?, seconds)),
            2 => Some(Self::Breathing(
                parse_hex(first)?,
                if alternate {
                    Some(parse_hex(second)?)
                } else {
                    None
                },
                seconds,
            )),
            3 => Some(Self::ColorCycle(saturation / 100.0, seconds)),
            _ => None,
        }
    }

    pub fn animated(self) -> bool {
        !matches!(self, Self::Solid(_))
    }

    pub fn sample(self, elapsed: f64) -> [u8; 3] {
        match self {
            Self::Solid(color) => color,
            Self::Fade(a, b, seconds) => fade(a, b, elapsed / seconds),
            Self::Breathing(a, b, seconds) => {
                let phase = (elapsed / seconds).rem_euclid(2.0);
                let color = if phase >= 1.0 { b.unwrap_or(a) } else { a };
                let brightness = (1.0 - (phase * std::f64::consts::TAU).cos()) / 2.0;
                color.map(|c| (f64::from(c) * brightness).round() as u8)
            }
            Self::ColorCycle(saturation, seconds) => {
                let hue = (elapsed / seconds).rem_euclid(1.0) * 6.0;
                [5.0, 3.0, 1.0].map(|offset| {
                    let channel = (offset + hue) % 6.0;
                    (255.0 * (1.0 - saturation * channel.min(4.0 - channel).clamp(0.0, 1.0)))
                        .round() as u8
                })
            }
        }
    }
}

fn fade(start: [u8; 3], end: [u8; 3], phase: f64) -> [u8; 3] {
    let half_cycle = phase.rem_euclid(1.0) * 2.0;
    let (from, to) = if half_cycle < 1.0 {
        (start, end)
    } else {
        (end, start)
    };
    // Hold the exact color for the first 70% of each half-cycle.
    let progress = ((half_cycle.fract() - 0.7) / 0.3).clamp(0.0, 1.0);
    let mix = progress * progress * (3.0 - 2.0 * progress);
    std::array::from_fn(|i| {
        (f64::from(from[i]) * (1.0 - mix) + f64::from(to[i]) * mix).round() as u8
    })
}

const FRAME_INTERVAL: Duration = Duration::from_nanos(16_666_667);
const WDL_FRAME_INTERVAL: Duration = Duration::from_micros(12_500);

type Targets = Vec<(String, Target)>;

fn next_frame(previous: Instant, now: Instant, interval: Duration) -> Instant {
    let scheduled = previous + interval;
    if scheduled >= now {
        return scheduled;
    }
    let late = now.duration_since(scheduled).as_nanos() % interval.as_nanos();
    now + interval - Duration::from_nanos(late as u64)
}

pub fn run(
    targets: Targets,
    effect: Effect,
    stop: &AtomicBool,
    ticket: &Ticket,
    report: &(impl Fn(String, String) + Sync),
) {
    let start = Instant::now();
    // Kernel LED drivers may use the same hardware as a direct backend.
    if !effect.animated()
        || targets
            .iter()
            .any(|(_, target)| matches!(target.driver().interface, Interface::SysfsAttributes))
    {
        return run_group(targets, effect, stop, ticket, start, FRAME_INTERVAL, report);
    }
    let mut groups: [Targets; Lane::ALL.len()] = Default::default();
    for item in targets {
        groups[item.1.driver().lane as usize].push(item);
    }
    let mut groups = Lane::ALL
        .into_iter()
        .zip(groups)
        .filter(|(_, group)| !group.is_empty());
    let Some((first, primary)) = groups.next() else {
        return;
    };
    let interval = |group| {
        if group == Lane::Wdl {
            WDL_FRAME_INTERVAL
        } else {
            FRAME_INTERVAL
        }
    };
    thread::scope(|scope| {
        let workers: Vec<_> = groups
            .map(|(group, targets)| {
                scope.spawn(move || {
                    run_group(
                        targets,
                        effect,
                        stop,
                        ticket,
                        start,
                        interval(group),
                        report,
                    )
                })
            })
            .collect();
        run_group(
            primary,
            effect,
            stop,
            ticket,
            start,
            interval(first),
            report,
        );
        for worker in workers {
            if worker.join().is_err() {
                report(
                    "LinRgb".into(),
                    crate::i18n::tr("RGB worker failed", "Le moteur RGB a échoué").into(),
                );
            }
        }
    })
}

fn run_group(
    targets: Targets,
    effect: Effect,
    stop: &AtomicBool,
    ticket: &Ticket,
    start: Instant,
    interval: Duration,
    report: &impl Fn(String, String),
) {
    let mut active = Vec::new();
    let mut connections = Default::default();
    for (name, target) in targets {
        if stop.load(Ordering::Relaxed) {
            break;
        }
        match target
            .connect(stop, ticket, &mut connections)
            .and_then(|mut writer| {
                let color = effect.sample(start.elapsed().as_secs_f64());
                writer.set_rgb(color)?;
                Ok((writer, color))
            }) {
            Ok((writer, color)) => {
                let state = if effect.animated() {
                    crate::i18n::tr("effect running", "effet en cours")
                } else {
                    crate::i18n::tr("color sent", "couleur envoyée")
                };
                let message = writer.capabilities().description().map_or_else(
                    || state.to_owned(),
                    |details| format!("{state} · {details}"),
                );
                report(name.clone(), message);
                active.push((name, writer, color));
            }
            Err(error) => report(
                name,
                format!("{}: {error}", crate::i18n::tr("failed", "échec")),
            ),
        }
    }
    drop(connections); // active sessions now own their transports
    if effect.animated() {
        let mut deadline = start;
        while !active.is_empty() && !stop.load(Ordering::Relaxed) {
            let now = Instant::now();
            deadline = next_frame(deadline, now, interval);
            thread::sleep(deadline.saturating_duration_since(now));
            if stop.load(Ordering::Relaxed) {
                break;
            }
            let rgb = effect.sample(start.elapsed().as_secs_f64());
            active.retain_mut(|(name, writer, previous)| {
                if rgb == *previous || stop.load(Ordering::Relaxed) {
                    return true;
                }
                match writer.set_rgb(rgb) {
                    Ok(sent) => {
                        if sent {
                            *previous = rgb;
                        }
                        true
                    }
                    Err(error) => {
                        report(
                            name.clone(),
                            format!(
                                "{}: {error}",
                                crate::i18n::tr("effect interrupted", "effet interrompu")
                            ),
                        );
                        false
                    }
                }
            });
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn reports_an_unavailable_target_without_starting_an_effect() {
        use super::{Effect, Target};
        let messages = std::sync::Mutex::new(Vec::new());
        super::run(
            vec![(
                "Missing LED".into(),
                Target::Kernel(
                    "/dev/null/linrgb-led".into(),
                    0,
                    crate::drivers::kernel_led::Layout::parse("red green blue", 255).unwrap(),
                ),
            )],
            Effect::ColorCycle(1.0, 6.0),
            &std::sync::atomic::AtomicBool::new(false),
            &crate::drivers::Generation::default().ticket(),
            &|name, status| messages.lock().unwrap().push((name, status)),
        );
        let messages = messages.into_inner().unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].0, "Missing LED");
        assert!(messages[0].1.starts_with("failed:"));
    }

    #[test]
    fn breathing_alternates_at_black_and_cycle_loops() {
        use super::Effect;
        let breath = Effect::Breathing([255, 0, 0], Some([0, 0, 255]), 4.0);
        assert_eq!(breath.sample(2.0), [255, 0, 0]);
        assert_eq!(breath.sample(4.0), [0; 3]);
        assert_eq!(breath.sample(6.0), [0, 0, 255]);
        let cycle = Effect::ColorCycle(1.0, 6.0);
        assert_eq!(cycle.sample(0.0), [255, 0, 0]);
        assert_eq!(cycle.sample(2.0), [0, 255, 0]);
        assert_eq!(cycle.sample(6.0), cycle.sample(0.0));
        assert_eq!(Effect::ColorCycle(0.0, 6.0).sample(3.0), [255; 3]);
    }

    #[test]
    fn frame_schedule_keeps_cadence_without_catching_up_in_bursts() {
        use super::{FRAME_INTERVAL, next_frame};
        let start = std::time::Instant::now();
        let late = start + FRAME_INTERVAL * 3;
        assert_eq!(
            next_frame(start, start + FRAME_INTERVAL / 2, FRAME_INTERVAL),
            start + FRAME_INTERVAL
        );
        assert_eq!(
            next_frame(start, late, FRAME_INTERVAL),
            late + FRAME_INTERVAL
        );
        assert_eq!(
            next_frame(start, late + FRAME_INTERVAL / 2, FRAME_INTERVAL),
            late + FRAME_INTERVAL
        );
    }

    #[test]
    fn ram_shares_a_worker_and_the_wdl_hub_does_not_block_other_transports() {
        use super::Target;
        let transport_group = |target: &Target| target.driver().lane;
        assert_eq!(
            transport_group(&Target::Ram(0x72)),
            transport_group(&Target::Ram(0x73))
        );
        let hub = transport_group(&Target::LampArray("hub".into()));
        for target in [Target::EneGpu("gpu".into()), Target::Ram(0x72)] {
            assert_ne!(transport_group(&target), hub);
        }
    }

    #[test]
    fn fade_holds_both_colors_and_blends_only_during_short_transitions() {
        let red = [200, 0, 0];
        let blue = [0, 0, 200];
        for (phase, color) in [
            (0.0, red),
            (0.25, red),
            (0.35, red),
            (0.425, [100, 0, 100]),
            (0.5, blue),
            (0.75, blue),
            (0.925, [100, 0, 100]),
            (1.0, red),
        ] {
            assert_eq!(super::fade(red, blue, phase), color);
        }
    }
}
