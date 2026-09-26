//! Windows' drives: a flat drive seen from a little above, and what goes with it.
//!
//! Explorer draws a drive as a light slab with a darker front edge and a light in it, seen from
//! just above, so its top reads as a wide panel. Kinds add to it: something on the top of the
//! system drive, a connector at the side of a USB drive, a disc standing in a disc drive, a
//! network pipe under a network drive. These follow that idea in FolderSkin's own drawing, with a
//! house on the system drive where Windows puts its logo. The face is the slab's top, or the disc
//! for a disc drive.

use super::draw::*;
use super::DriveKind;
use tiny_skia::Path;

/// Where a slab is: its top's back edge and front edge, the left and right of its front, how far
/// its top narrows towards the back, and its foot.
#[derive(Clone, Copy)]
struct Slab {
    x0: f32,
    x1: f32,
    back: f32,
    edge: f32,
    foot: f32,
    narrow: f32,
}

impl Slab {
    /// Rounding of the slab's corners.
    const R: f32 = 44.0;

    fn top(&self) -> Path {
        polygon(
            &[
                (self.x0 + self.narrow, self.back),
                (self.x1 - self.narrow, self.back),
                (self.x1, self.edge),
                (self.x0, self.edge),
            ],
            Slab::R,
        )
    }

    fn whole(&self) -> Path {
        polygon(
            &[
                (self.x0 + self.narrow, self.back),
                (self.x1 - self.narrow, self.back),
                (self.x1, self.edge),
                (self.x1, self.foot),
                (self.x0, self.foot),
                (self.x0, self.edge),
            ],
            Slab::R,
        )
    }

    fn middle(&self) -> (f32, f32) {
        ((self.x0 + self.x1) / 2.0, (self.back + self.edge) / 2.0)
    }
}

/// The local disk's slab.
const DISK: Slab = Slab {
    x0: 88.0,
    x1: 936.0,
    back: 286.0,
    edge: 640.0,
    foot: 746.0,
    narrow: 40.0,
};

pub(super) fn draw(kind: DriveKind, d: &mut Drawing) {
    match kind {
        DriveKind::Removable => usb(d),
        DriveKind::Card => card(d),
        DriveKind::Optical | DriveKind::OpticalDrive => disc_drive(d),
        DriveKind::Network | DriveKind::Server => network(d),
        DriveKind::Startup => {
            slab(d, &DISK, Layer::Body, true);
            let (x, y) = DISK.middle();
            let mark = house(x, y + 4.0, 0.92);
            d.fill(
                Layer::Body,
                &moved(&mark, 0.0, 6.0),
                &solid(rgba(0x0f3d7a, 50)),
            );
            d.fill(
                Layer::Body,
                &mark,
                &down(
                    y - 130.0,
                    y + 130.0,
                    &[(0.0, rgb(0x5aa9ff)), (1.0, rgb(0x1f6fe5))],
                ),
            );
        }
        _ => slab(d, &DISK, Layer::Body, true),
    }
}

/// A slab, drawn into `layer`, its top the face when `face` says so.
fn slab(d: &mut Drawing, s: &Slab, layer: Layer, face: bool) {
    let whole = s.whole();
    let top = s.top();
    // The front edge, then the top over it.
    d.fill(
        layer,
        &whole,
        &down(
            s.edge,
            s.foot,
            &[
                (0.0, rgb(0xa4aab2)),
                (0.55, rgb(0x858c95)),
                (1.0, rgb(0x6b727b)),
            ],
        ),
    );
    let front = rrect4(
        s.x0,
        s.edge - 10.0,
        s.x1,
        s.foot,
        [0.0, 0.0, Slab::R, Slab::R],
    );
    d.rim(
        layer,
        &open_polygon(
            &[
                (s.x1, s.foot - 50.0),
                (s.x1, s.foot),
                (s.x0, s.foot),
                (s.x0, s.foot - 50.0),
            ],
            Slab::R,
        ),
        &whole,
        rgba(0x000000, 60),
        6.0,
    );
    d.fill_in(
        layer,
        &front,
        &down(
            s.edge,
            s.edge + 22.0,
            &[(0.0, rgba(0xffffff, 90)), (1.0, rgba(0xffffff, 0))],
        ),
        &whole,
    );
    let (lx, ly) = (s.x1 - 118.0, (s.edge + s.foot) / 2.0 + 4.0);
    let light = rrect(lx - 38.0, ly - 10.0, lx + 38.0, ly + 10.0, 10.0);
    d.fill(
        layer,
        &rrect(lx - 60.0, ly - 26.0, lx + 60.0, ly + 26.0, 26.0),
        &radial(
            lx,
            ly,
            64.0,
            &[(0.0, rgba(0x6fd3ff, 90)), (1.0, rgba(0x6fd3ff, 0))],
        ),
    );
    d.fill(
        layer,
        &light,
        &across(
            lx - 38.0,
            lx + 38.0,
            &[(0.0, rgb(0x7fdcff)), (1.0, rgb(0x33b3f5))],
        ),
    );
    d.fill(
        layer,
        &top,
        &down(
            s.back,
            s.edge,
            &[
                (0.0, rgb(0xf7f8fa)),
                (0.6, rgb(0xe8ebef)),
                (1.0, rgb(0xd6dbe1)),
            ],
        ),
    );
    d.edge(&whole);
    if face {
        d.set_face(&top);
    }
    // Over the top: light along its back edge and a shade where it turns into the front.
    let over = if face { Layer::Over } else { layer };
    let back = open_polygon(
        &[
            (s.x0, s.edge),
            (s.x0 + s.narrow, s.back),
            (s.x1 - s.narrow, s.back),
            (s.x1, s.edge),
        ],
        Slab::R,
    );
    d.rim_ink(
        over,
        &back,
        &top,
        &fading(
            s.back + 30.0,
            s.back + (s.edge - s.back) * 0.8,
            rgba(0xffffff, 210),
        ),
        6.0,
    );
    d.fill_in(
        over,
        &top,
        &down(
            s.edge - 46.0,
            s.edge,
            &[(0.0, rgba(0x000000, 0)), (1.0, rgba(0x000000, 36))],
        ),
        &top,
    );
    d.fill_in(
        over,
        &top,
        &across(
            s.x0,
            s.x0 + 70.0,
            &[(0.0, rgba(0x000000, 22)), (1.0, rgba(0x000000, 0))],
        ),
        &top,
    );
    d.fill_in(
        over,
        &top,
        &across(
            s.x1 - 70.0,
            s.x1,
            &[(0.0, rgba(0x000000, 0)), (1.0, rgba(0x000000, 26))],
        ),
        &top,
    );
    // A fine edge all round, as Windows' own icons have, so a light drive holds its shape on a
    // white window.
    d.stroke(over, &whole, &solid(rgba(0x2b3440, 46)), 3.0);
}

/// A house, `scale` times the size of the one on the Mac's startup disk, centred on `(x, y)`.
fn house(x: f32, y: f32, scale: f32) -> Path {
    let k = |v: f32| v * scale;
    polygon(
        &[
            (x, y - k(128.0)),
            (x + k(138.0), y - k(8.0)),
            (x + k(96.0), y - k(8.0)),
            (x + k(96.0), y + k(118.0)),
            (x - k(96.0), y + k(118.0)),
            (x - k(96.0), y - k(8.0)),
            (x - k(138.0), y - k(8.0)),
        ],
        k(12.0),
    )
}

/// A USB drive: the slab with a connector out of its right side.
fn usb(d: &mut Drawing) {
    let s = Slab {
        x0: 70.0,
        x1: 842.0,
        ..DISK
    };
    let (y0, y1) = (402.0, 574.0);
    let plug = rrect4(s.x1 - 40.0, y0, 958.0, y1, [0.0, 26.0, 26.0, 0.0]);
    d.fill(
        Layer::Body,
        &plug,
        &down(
            y0,
            y1,
            &[
                (0.0, rgb(0xeef0f2)),
                (0.5, rgb(0xc4c9cf)),
                (1.0, rgb(0x9aa1a9)),
            ],
        ),
    );
    d.rim(Layer::Body, &plug, &plug, rgba(0x000000, 50), 4.0);
    for (hy0, hy1) in [(y0 + 34.0, y0 + 70.0), (y1 - 70.0, y1 - 34.0)] {
        d.fill(
            Layer::Body,
            &rrect(894.0, hy0, 930.0, hy1, 6.0),
            &solid(rgb(0x5a6068)),
        );
    }
    d.edge(&plug);
    slab(d, &s, Layer::Body, true);
}

/// An SD card, the way Explorer draws one in a card reader.
fn card(d: &mut Drawing) {
    let (x0, y0, x1, y1) = (236.0, 112.0, 788.0, 900.0);
    let body = polygon(
        &[
            (x0, y0),
            (x1 - 132.0, y0),
            (x1, y0 + 132.0),
            (x1, y1),
            (x0, y1),
        ],
        46.0,
    );
    d.fill(
        Layer::Body,
        &body,
        &down(y0, y1, &[(0.0, rgb(0x62728a)), (1.0, rgb(0x3a4556))]),
    );
    // Grooves along its top.
    for i in 0..4 {
        let gx = x0 + 88.0 + i as f32 * 64.0;
        d.fill(
            Layer::Body,
            &rrect(gx, y0 + 56.0, gx + 30.0, y0 + 150.0, 12.0),
            &solid(rgba(0x000000, 60)),
        );
    }
    d.edge(&body);
    let label = rrect(x0 + 44.0, y0 + 200.0, x1 - 44.0, y1 - 44.0, 28.0);
    d.fill(
        Layer::Body,
        &label,
        &down(
            y0 + 200.0,
            y1 - 44.0,
            &[(0.0, rgb(0xfbfcfd)), (1.0, rgb(0xe3e7ed))],
        ),
    );
    d.set_face(&label);
    d.rim(Layer::Over, &label, &label, rgba(0x1b2533, 60), 5.0);
    let top = open_polygon(
        &[
            (x0, y1),
            (x0, y0),
            (x1 - 132.0, y0),
            (x1, y0 + 132.0),
            (x1, y1),
        ],
        46.0,
    );
    d.rim_ink(
        Layer::Over,
        &top,
        &body,
        &fading(y0 + 140.0, y1 - 80.0, rgba(0xffffff, 110)),
        6.0,
    );
}

/// A disc drive: a disc standing in the slab, the disc its face.
fn disc_drive(d: &mut Drawing) {
    let s = Slab {
        back: 560.0,
        edge: 790.0,
        foot: 880.0,
        narrow: 30.0,
        ..DISK
    };
    let (x, y, outer, hole) = (512.0, 442.0, 356.0, 50.0);
    let (print_out, print_in) = (344.0, 134.0);
    d.fill(
        Layer::Body,
        &ring(x, y, outer, hole),
        &radial(
            x,
            y,
            outer,
            &[
                (0.0, rgb(0xfbfcfd)),
                (0.7, rgb(0xe9ecef)),
                (1.0, rgb(0xd0d4da)),
            ],
        ),
    );
    let band = ring(x, y, print_out, print_in);
    d.fill(
        Layer::Body,
        &band,
        &sweep(
            x,
            y,
            200.0,
            &[
                (0.0, rgba(0x7fd8ff, 0)),
                (0.08, rgba(0x7fd8ff, 120)),
                (0.16, rgba(0xa78bfa, 110)),
                (0.24, rgba(0xff9ec7, 110)),
                (0.32, rgba(0xffd166, 110)),
                (0.4, rgba(0xffd166, 0)),
                (1.0, rgba(0x7fd8ff, 0)),
            ],
        ),
    );
    d.set_face(&band);
    d.set_face_point(x, y - (print_out + print_in) / 2.0);
    d.fill(
        Layer::Body,
        &ring(x, y, print_in - 6.0, hole),
        &radial(
            x,
            y,
            print_in,
            &[(0.0, rgb(0xe6e9ed)), (1.0, rgb(0xf8f9fb))],
        ),
    );
    d.stroke(
        Layer::Body,
        &circle(x, y, hole + 2.0),
        &solid(rgba(0x59606a, 150)),
        4.0,
    );
    d.edge(&circle(x, y, outer));
    d.stroke(
        Layer::Over,
        &circle(x, y, outer - 1.5),
        &solid(rgba(0x7c828b, 150)),
        3.0,
    );
    d.stroke(
        Layer::Over,
        &circle(x, y, print_in),
        &solid(rgba(0x7c828b, 70)),
        4.0,
    );
    // The slab goes over the disc's lower half, as the drive it stands in.
    slab(d, &s, Layer::Over, false);
}

/// A network drive: the slab over the pipe that connects it.
fn network(d: &mut Drawing) {
    let s = Slab {
        back: 210.0,
        edge: 552.0,
        foot: 652.0,
        ..DISK
    };
    let stem = rrect(488.0, s.foot - 20.0, 536.0, 800.0, 8.0);
    d.fill(
        Layer::Body,
        &stem,
        &across(
            488.0,
            536.0,
            &[
                (0.0, rgb(0x8d949d)),
                (0.5, rgb(0xb6bcc3)),
                (1.0, rgb(0x7b828b)),
            ],
        ),
    );
    let pipe = rrect(196.0, 772.0, 828.0, 852.0, 40.0);
    d.fill(
        Layer::Body,
        &pipe,
        &down(
            772.0,
            852.0,
            &[
                (0.0, rgb(0x6fdb86)),
                (0.5, rgb(0x3fb55d)),
                (1.0, rgb(0x2a9147)),
            ],
        ),
    );
    d.rim(
        Layer::Body,
        &polyline(&[(236.0, 772.0), (788.0, 772.0)]),
        &pipe,
        rgba(0xffffff, 150),
        5.0,
    );
    d.edge(&pipe);
    slab(d, &s, Layer::Body, true);
}
