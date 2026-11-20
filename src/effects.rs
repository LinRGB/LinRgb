use crate::drivers::{Interface, Lane, Target, Ticket};
use std::{
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::{Duration, Instant},
};

/// Bounded at edit/load time; sampling never allocates or parses a color.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Palette {
    colors: [[u8; 3]; 5],
    len: usize,
}
impl Palette {
    pub fn parse<'a>(colors: impl IntoIterator<Item = &'a str>) -> Option<Self> {
        let mut p = Self {
            colors: [[0; 3]; 5],
            len: 0,
        };
        for color in colors {
            *p.colors.get_mut(p.len)? = crate::color_wheel::parse_hex(color)?;
            p.len += 1;
        }
        (p.len >= 2).then_some(p)
    }
    fn sample(self, phase: f64) -> [u8; 3] {
        let index = phase.rem_euclid(1.0) * self.len as f64;
        let i = index as usize % self.len;
        blend(
            self.colors[i],
            self.colors[(i + 1) % self.len],
            smooth(index.fract()),
        )
    }
}
fn smooth(x: f64) -> f64 {
    x * x * (3.0 - 2.0 * x)
}
fn blend(a: [u8; 3], b: [u8; 3], w: f64) -> [u8; 3] {
    std::array::from_fn(|i| (f64::from(a[i]) * (1. - w) + f64::from(b[i]) * w).round() as u8)
}
#[derive(Clone, Copy)]
pub enum Effect {
    Solid([u8; 3]),
    Fade([u8; 3], [u8; 3], f64),
    Breathing([u8; 3], Option<[u8; 3]>, f64),
    ColorCycle(f64, f64), // classic spectrum (legacy mode 3)
    PaletteCycle(Palette, f64),
    Aurora(Palette, f64),
    Gradient([u8; 3], [u8; 3], f64),
    Pulse([u8; 3], [u8; 3], f64),
}

impl Effect {
    pub fn from_inputs(
        mode: u32,
        first: &str,
        second: &str,
        alternate: bool,
        saturation: f64,
        seconds: f64,
        palette: Option<Palette>,
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
            3 => Some(match palette {
                Some(p) => Self::PaletteCycle(p, seconds),
                None => Self::ColorCycle(saturation / 100.0, seconds),
            }),
            4 => Some(Self::Aurora(palette?, seconds)),
            5 => Some(Self::Gradient(
                parse_hex(first)?,
                parse_hex(second)?,
                seconds,
            )),
            6 => Some(Self::Pulse(parse_hex(first)?, parse_hex(second)?, seconds)),
            _ => None,
        }
    }

    pub fn at(self, epoch: Instant, now: Instant) -> [u8; 3] {
        self.sample(now.saturating_duration_since(epoch).as_secs_f64())
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
            Self::PaletteCycle(p, s) => p.sample(elapsed / s),
            Self::Aurora(p, s) => {
                let phase = (elapsed / s).rem_euclid(1.0);
                let a = phase * std::f64::consts::TAU;
                // Two overlapping color bands drift at different speeds but loop together.
                blend(
                    p.sample(phase + 0.06 * a.sin()),
                    p.sample(2. * phase + 0.25),
                    0.22 + 0.16 * a.cos(),
                )
            }
            Self::Gradient(a, b, s) => blend(
                a,
                b,
                (1. - (std::f64::consts::TAU * elapsed / s).cos()) * 0.5,
            ),
            Self::Pulse(peak, background, s) => {
                let t = (elapsed / s).rem_euclid(1.0);
                let w = if t < 0.12 {
                    smooth(t / 0.12)
                } else {
                    1. - smooth(((t - 0.12) / 0.48).min(1.))
                };
                blend(background, peak, w)
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
    blend(from, to, smooth(progress))
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
    start: Instant,
    stop: &AtomicBool,
    ticket: &Ticket,
    report: &(impl Fn(String, String) + Sync),
) {
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
                let color = effect.at(start, Instant::now());
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
            active.retain_mut(|(name, writer, previous)| {
                // A previous synchronous write may have blocked. Sample the current
                // common phase, never replay its stale color on the next controller.
                let rgb = effect.at(start, Instant::now());
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
    #[ignore = "manual sampler timing, not a hardware benchmark"]
    fn time_aurora_sampler() {
        use super::*;
        use std::hint::black_box;
        let effect = Effect::Aurora(
            Palette::parse(["#123456", "#ABCDEF", "#FF55AA", "#104020", "#765432"]).unwrap(),
            6.,
        );
        let mut times = [0.; 5];
        for ns in &mut times {
            let start = Instant::now();
            for i in 0..1_000_000 {
                black_box(effect.sample(black_box(i as f64 / 60.)));
            }
            *ns = start.elapsed().as_nanos() as f64 / 1_000_000.;
        }
        times.sort_by(f64::total_cmp);
        println!("Aurora: {:.1} ns/sample (median of 5 × 1M)", times[2]);
    }
    #[test]
    fn palettes_enforce_bounds_and_follow_all_edited_stops() {
        use super::*;
        for invalid in [
            vec![],
            vec!["#abcdef"],
            vec!["#abcdef", "wrong"],
            vec!["#abcdef"; 6],
        ] {
            assert!(Palette::parse(invalid).is_none());
        }
        let p = Palette::parse(["#ff0000", "#00ff00", "#0000ff"]).unwrap();
        for (t, rgb) in [
            (0., [255, 0, 0]),
            (2., [0, 255, 0]),
            (4., [0, 0, 255]),
            (6., [255, 0, 0]),
        ] {
            assert_eq!(Effect::PaletteCycle(p, 6.).sample(t), rgb);
        }
    }
    #[test]
    fn pulse_gradient_and_aurora_use_user_colors_and_loop() {
        use super::*;
        let a = [21, 37, 92];
        let b = [110, 52, 14];
        assert_eq!(Effect::Gradient(a, b, 4.).sample(2.), b);
        assert_eq!(Effect::Pulse(a, b, 10.).sample(1.2), a);
        assert_eq!(Effect::Pulse(a, b, 10.).sample(9.), b);
        let p = Palette::parse(["#15255C", "#6E340E"]).unwrap();
        let e = Effect::Aurora(p, 6.);
        assert_ne!(e.sample(0.), e.sample(2.));
        assert_eq!(e.sample(0.), e.sample(6.));
    }
    #[test]
    fn delayed_worker_joins_preview_phase_without_replaying_missed_frames() {
        use super::*;
        let epoch = Instant::now();
        let late = epoch + Duration::from_millis(2250);
        let e = Effect::Gradient([0; 3], [200; 3], 4.);
        assert_ne!(e.at(epoch, epoch), e.at(epoch, late));
        assert_eq!(e.at(epoch, late), e.sample(2.25));
        let next = next_frame(epoch, late, FRAME_INTERVAL);
        assert!(next >= late && next <= late + FRAME_INTERVAL);
    }
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
            std::time::Instant::now(),
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
