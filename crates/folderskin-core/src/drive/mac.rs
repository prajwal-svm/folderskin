//! The Mac's drives: upright disks with a strip along their foot, a memory card and a disc.
//!
//! Finder draws every disk as the same upright box, rounded at the top, standing on a strip with
//! a light in it, and tells the kinds apart by colour and by a mark pressed into the front. These
//! follow that idea with FolderSkin's own marks (a platter, a house, a plug, an eject mark, a
//! globe, a clock, a framed disc) and no logo. The face is the whole front above the strip, so a
//! picture covers most of the disk while the strip and the rounded sides keep it a disk.

use super::draw::*;
use super::DriveKind;
use tiny_skia::Path;

/// The disk's sides, the top of its front and the strip it stands on.
const X0: f32 = 168.0;
const X1: f32 = 856.0;
const TOP: f32 = 92.0;
const SEAM: f32 = 820.0;
const FOOT: f32 = 932.0;
/// Rounding of the front's top corners and of the strip's bottom ones.
const TOP_R: f32 = 76.0;
const FOOT_R: f32 = 50.0;

/// Where a mark is pressed into the front: its middle.
const MARK: (f32, f32) = (512.0, 440.0);

/// A disk's colours: its front from top to bottom, and its strip.
struct Finish {
    front: [[u8; 4]; 3],
    strip: [[u8; 4]; 2],
    /// The strip's light.
    light: [u8; 4],
}

const ALUMINIUM: Finish = Finish {
    front: [rgb(0xf2f3f5), rgb(0xdadce0), rgb(0xbec1c6)],
    strip: [rgb(0x74787e), rgb(0x46494e)],
    light: rgb(0xf4f6f8),
};

/// The light strip the coloured disks stand on.
const SILVER: [[u8; 4]; 2] = [rgb(0xf3f4f6), rgb(0xc7cad0)];

const AMBER: Finish = Finish {
    front: [rgb(0xffd352), rgb(0xf8a91e), rgb(0xee900b)],
    strip: SILVER,
    light: rgb(0xffffff),
};

const PEARL: Finish = Finish {
    front: [rgb(0xfdfdfe), rgb(0xe8eaed), rgb(0xd2d5da)],
    strip: SILVER,
    light: rgb(0xffffff),
};

const CYAN: Finish = Finish {
    front: [rgb(0x74dcf4), rgb(0x38bce8), rgb(0x1a9fd4)],
    strip: SILVER,
    light: rgb(0xffffff),
};

const GREEN: Finish = Finish {
    front: [rgb(0x6fd9a4), rgb(0x31b277), rgb(0x1b9460)],
    strip: SILVER,
    light: rgb(0xffffff),
};

const LILAC: Finish = Finish {
    front: [rgb(0xf1effa), rgb(0xd9d4ef), rgb(0xbdb4e0)],
    strip: SILVER,
    light: rgb(0xffffff),
};

pub(super) fn draw(kind: DriveKind, d: &mut Drawing) {
    match kind {
        DriveKind::Card => card(d),
        DriveKind::Optical | DriveKind::OpticalDrive => disc(d),
        DriveKind::External => disk(d, &AMBER, &plug()),
        DriveKind::Removable => disk(d, &PEARL, &eject()),
        DriveKind::Network | DriveKind::Server => disk(d, &CYAN, &globe()),
        DriveKind::TimeMachine => disk(d, &GREEN, &clock()),
        DriveKind::DiskImage => disk(d, &LILAC, &framed_disc()),
        DriveKind::Startup => disk(d, &ALUMINIUM, &house()),
        _ => disk(d, &ALUMINIUM, &platter()),
    }
}

/// The front, rounded at the top and square where it meets the strip.
fn front() -> Path {
    rrect4(X0, TOP, X1, SEAM, [TOP_R, TOP_R, 0.0, 0.0])
}

/// An upright disk of `finish`, with `mark` pressed into its front.
fn disk(d: &mut Drawing, finish: &Finish, mark: &Path) {
    let front = front();
    let [hi, mid, lo] = finish.front;
    d.part("face");
    d.fill(
        Layer::Body,
        &front,
        &down(TOP, SEAM, &[(0.0, hi), (0.45, mid), (1.0, lo)]),
    );
    // The mark is a deeper shade of the front, the way Finder's own disks carry theirs.
    d.part("mark");
    d.press(Layer::Body, mark, mix(lo, rgb(0x000000), 0.1), 7.0);
    d.set_face(&front);
    d.edge(&front);

    // The strip it stands on, its light, and the seam between them.
    let strip = rrect4(X0, SEAM, X1, FOOT, [0.0, 0.0, FOOT_R, FOOT_R]);
    let [s0, s1] = finish.strip;
    d.part("strip");
    d.fill(
        Layer::Body,
        &strip,
        &down(SEAM, FOOT, &[(0.0, s0), (1.0, s1)]),
    );
    let top_of_strip = polyline(&[(X0, SEAM), (X1, SEAM)]);
    d.rim(Layer::Body, &top_of_strip, &strip, rgba(0xffffff, 150), 5.0);
    let foot_edge = open_polygon(&[(X0, SEAM), (X0, FOOT), (X1, FOOT), (X1, SEAM)], FOOT_R);
    d.rim_ink(
        Layer::Body,
        &foot_edge,
        &strip,
        &down(
            SEAM + 30.0,
            FOOT - 20.0,
            &[(0.0, rgba(0x000000, 0)), (1.0, rgba(0x000000, 60))],
        ),
        7.0,
    );
    let light = (X1 - 62.0, (SEAM + FOOT) / 2.0 + 2.0);
    d.part("light");
    d.fill(
        Layer::Body,
        &circle(light.0, light.1, 22.0),
        &radial(
            light.0,
            light.1,
            22.0,
            &[
                (0.0, fade(finish.light, 0.5)),
                (1.0, fade(finish.light, 0.0)),
            ],
        ),
    );
    d.fill(
        Layer::Body,
        &circle(light.0, light.1, 10.0),
        &solid(finish.light),
    );
    d.edge(&strip);

    // Over whatever is on the front: its sides turning away, each shade easing out so no line
    // shows where it ends, the light along its top edge and its foot darkening into the seam.
    d.part("face");
    let ease = |a: u8| {
        let at = |k: f32| rgba(0x000000, (f32::from(a) * k).round() as u8);
        [
            (0.0, at(1.0)),
            (0.3, at(0.5)),
            (0.65, at(0.16)),
            (1.0, at(0.0)),
        ]
    };
    d.fill_in(
        Layer::Over,
        &front,
        &across(X0, X0 + 110.0, &ease(46)),
        &front,
    );
    d.fill_in(
        Layer::Over,
        &front,
        &across(X1, X1 - 110.0, &ease(56)),
        &front,
    );
    let top_edge = open_polygon(&[(X0, SEAM), (X0, TOP), (X1, TOP), (X1, SEAM)], TOP_R);
    d.rim_ink(
        Layer::Over,
        &top_edge,
        &front,
        &fading(TOP + 20.0, TOP + 420.0, rgba(0xffffff, 170)),
        6.0,
    );
    d.fill_in(
        Layer::Over,
        &front,
        &down(
            SEAM - 70.0,
            SEAM,
            &[(0.0, rgba(0x000000, 0)), (1.0, rgba(0x000000, 60))],
        ),
        &front,
    );
    d.stroke(
        Layer::Over,
        &polyline(&[(X0 + 2.0, SEAM), (X1 - 2.0, SEAM)]),
        &solid(rgba(0x000000, 90)),
        4.0,
    );
    // A fine edge all round, so a light disk keeps its top on a white window.
    d.stroke(
        Layer::Over,
        &rrect4(X0, TOP, X1, FOOT, [TOP_R, TOP_R, FOOT_R, FOOT_R]),
        &solid(rgba(0x1d2530, 34)),
        3.0,
    );
}

/// A platter and the arm that reads it: an internal disk.
fn platter() -> Path {
    let (x, y) = MARK;
    let arm = outline_of(
        &polyline(&[(x + 150.0, y - 150.0), (x + 82.0, y - 42.0)]),
        30.0,
    );
    join(&[ring(x, y, 128.0, 104.0), circle(x, y, 34.0), arm])
}

/// A house: the disk the Mac starts from.
fn house() -> Path {
    let (x, y) = MARK;
    polygon(
        &[
            (x, y - 140.0),
            (x + 150.0, y - 10.0),
            (x + 104.0, y - 10.0),
            (x + 104.0, y + 130.0),
            (x - 104.0, y + 130.0),
            (x - 104.0, y - 10.0),
            (x - 150.0, y - 10.0),
        ],
        14.0,
    )
}

/// A plug on the end of its cable: a disk plugged in.
fn plug() -> Path {
    let (x, y) = MARK;
    let head = rrect(x - 58.0, y - 110.0, x + 58.0, y + 20.0, 24.0);
    let tip = rrect(x - 32.0, y - 168.0, x + 32.0, y - 108.0, 10.0);
    let cable = outline_of(&polyline(&[(x, y + 18.0), (x, y + 150.0)]), 34.0);
    join(&[head, tip, cable])
}

/// The eject mark: media that comes out.
fn eject() -> Path {
    let (x, y) = MARK;
    let arrow = polygon(
        &[(x, y - 126.0), (x + 132.0, y + 36.0), (x - 132.0, y + 36.0)],
        16.0,
    );
    let bar = rrect(x - 132.0, y + 78.0, x + 132.0, y + 126.0, 14.0);
    join(&[arrow, bar])
}

/// A globe: a share on the network.
fn globe() -> Path {
    let (x, y) = MARK;
    let r = 124.0;
    let w = 22.0;
    let mut lines = vec![
        outline_of(&ellipse(x, y, r, r), w),
        outline_of(&ellipse(x, y, r * 0.44, r), w),
    ];
    lines.push(outline_of(&polyline(&[(x - r, y), (x + r, y)]), w));
    let lat = r * 0.52;
    let half = (r * r - lat * lat).sqrt();
    lines.push(outline_of(
        &polyline(&[(x - half, y - lat), (x + half, y - lat)]),
        w * 0.9,
    ));
    lines.push(outline_of(
        &polyline(&[(x - half, y + lat), (x + half, y + lat)]),
        w * 0.9,
    ));
    join(&lines)
}

/// A clock with an arrow running back round it: backups kept over time.
fn clock() -> Path {
    let (x, y) = MARK;
    let r = 124.0;
    let w = 26.0;
    // The circle stops short at the top left, where the arrow's head points back along it.
    let mut pb = tiny_skia::PathBuilder::new();
    crate::geometry::arc(&mut pb, x, y, r, 250.0, 520.0);
    let rim = outline_of(&pb.finish().expect("an arc"), w);
    let a = 250f32.to_radians();
    let (hx, hy) = (x + r * a.cos(), y + r * a.sin());
    let head = polygon(
        &[
            (hx - 58.0, hy - 6.0),
            (hx + 30.0, hy - 52.0),
            (hx + 22.0, hy + 48.0),
        ],
        8.0,
    );
    let hands = outline_of(&polyline(&[(x, y - 70.0), (x, y), (x + 54.0, y + 34.0)]), w);
    join(&[rim, head, hands])
}

/// A disc in a frame: an image of a disk.
fn framed_disc() -> Path {
    let (x, y) = MARK;
    let frame = outline_of(
        &rrect(x - 124.0, y - 124.0, x + 124.0, y + 124.0, 34.0),
        24.0,
    );
    join(&[frame, ring(x, y, 70.0, 48.0), circle(x, y, 16.0)])
}

/// An ellipse, as a closed path.
fn ellipse(cx: f32, cy: f32, rx: f32, ry: f32) -> Path {
    circle(0.0, 0.0, 1.0)
        .transform(tiny_skia::Transform::from_row(rx, 0.0, 0.0, ry, cx, cy))
        .expect("an ellipse")
}

/// A memory card, the kind a Mac's card reader takes: dark, with a label on its front and a
/// corner cut off.
fn card(d: &mut Drawing) {
    let (x0, y0, x1, y1) = (220.0, 100.0, 804.0, 904.0);
    let body = polygon(
        &[
            (x0, y0),
            (x1 - 128.0, y0),
            (x1, y0 + 128.0),
            (x1, y1),
            (x0, y1),
        ],
        40.0,
    );
    d.part("case");
    d.fill(
        Layer::Body,
        &body,
        &down(y0, y1, &[(0.0, rgb(0x5a5e65)), (1.0, rgb(0x303338))]),
    );
    d.fill_in(
        Layer::Body,
        &body,
        &radial(
            x0 + 60.0,
            y0 + 40.0,
            520.0,
            &[(0.0, rgba(0xffffff, 40)), (1.0, rgba(0xffffff, 0))],
        ),
        &body,
    );
    // The write-protect switch on its left edge, and the arrow that says which way it goes in.
    let switch = rrect(x0 - 16.0, 430.0, x0 + 10.0, 540.0, 8.0);
    d.part("switch");
    d.fill(
        Layer::Body,
        &switch,
        &down(430.0, 540.0, &[(0.0, rgb(0xd9dce0)), (1.0, rgb(0xa9adb3))]),
    );
    let arrow = polygon(
        &[
            (x0 + 70.0, y0 + 70.0),
            (x0 + 130.0, y0 + 70.0),
            (x0 + 100.0, y0 + 122.0),
        ],
        6.0,
    );
    d.part("mark");
    d.fill(Layer::Body, &arrow, &solid(rgba(0xffffff, 150)));
    d.edge(&body);

    let label = rrect(x0 + 40.0, y0 + 176.0, x1 - 40.0, y1 - 38.0, 24.0);
    d.part("face");
    d.fill(
        Layer::Body,
        &label,
        &down(
            y0 + 176.0,
            y1 - 38.0,
            &[(0.0, rgb(0xf6f7f9)), (1.0, rgb(0xdfe2e7))],
        ),
    );
    d.set_face(&label);

    d.rim(Layer::Over, &label, &label, rgba(0x000000, 70), 5.0);
    d.part("case");
    let top = open_polygon(
        &[
            (x0, y1),
            (x0, y0),
            (x1 - 128.0, y0),
            (x1, y0 + 128.0),
            (x1, y1),
        ],
        40.0,
    );
    d.rim_ink(
        Layer::Over,
        &top,
        &body,
        &fading(y0 + 140.0, y1 - 80.0, rgba(0xffffff, 90)),
        5.0,
    );
}

/// A disc, printable between its hub and its rim.
fn disc(d: &mut Drawing) {
    let (x, y) = (512.0, 512.0);
    let (outer, print_out, print_in, hub, hole) = (428.0, 414.0, 164.0, 150.0, 60.0);
    let whole = ring(x, y, outer, hole);
    d.part("disc");
    d.fill(
        Layer::Body,
        &whole,
        &radial(
            x,
            y,
            outer,
            &[
                (0.0, rgb(0xfafbfc)),
                (0.6, rgb(0xe9ebee)),
                (1.0, rgb(0xd3d6db)),
            ],
        ),
    );
    // The rainbow a disc throws back, twice round.
    let spectrum = [
        (0.0, rgba(0xff6b6b, 0)),
        (0.06, rgba(0xff8a65, 110)),
        (0.11, rgba(0xffd54f, 120)),
        (0.16, rgba(0x81e6a0, 110)),
        (0.21, rgba(0x64c8ff, 110)),
        (0.26, rgba(0xb39ddb, 100)),
        (0.32, rgba(0xb39ddb, 0)),
        (0.5, rgba(0xff6b6b, 0)),
        (0.56, rgba(0xff8a65, 90)),
        (0.61, rgba(0xffd54f, 100)),
        (0.66, rgba(0x81e6a0, 90)),
        (0.71, rgba(0x64c8ff, 90)),
        (0.76, rgba(0xb39ddb, 80)),
        (0.82, rgba(0xb39ddb, 0)),
        (1.0, rgba(0xff6b6b, 0)),
    ];
    let band = ring(x, y, print_out, print_in);
    d.part("face");
    d.fill(Layer::Body, &band, &sweep(x, y, -20.0, &spectrum));
    d.fill(
        Layer::Body,
        &band,
        &radial(
            x,
            y,
            print_out,
            &[
                (0.0, rgba(0xffffff, 0)),
                (0.5, rgba(0xffffff, 60)),
                (1.0, rgba(0xffffff, 0)),
            ],
        ),
    );
    d.set_face(&band);
    d.set_face_point(x, y - (print_out + print_in) / 2.0);
    d.edge(&circle(x, y, outer));
    d.edge(&circle(x, y, hole));

    // The clear hub, its stacking ring and the hole.
    d.part("hub");
    let hub_ring = ring(x, y, hub, hole);
    d.fill(
        Layer::Body,
        &hub_ring,
        &radial(
            x,
            y,
            hub,
            &[
                (0.0, rgb(0xe8eaee)),
                (0.7, rgb(0xf7f8fa)),
                (1.0, rgb(0xd9dce1)),
            ],
        ),
    );
    d.stroke(
        Layer::Body,
        &circle(x, y, 104.0),
        &solid(rgba(0x8a9099, 70)),
        6.0,
    );
    d.stroke(
        Layer::Body,
        &circle(x, y, hole + 2.0),
        &solid(rgba(0x5d636c, 140)),
        4.0,
    );

    // Over the print: the clear lip round the rim and the groove where the print ends.
    d.part("disc");
    let lip = ring(x, y, outer, print_out);
    d.fill(
        Layer::Over,
        &lip,
        &radial(
            x,
            y,
            outer,
            &[(0.9, rgba(0xffffff, 150)), (1.0, rgba(0xc9ccd2, 220))],
        ),
    );
    d.stroke(
        Layer::Over,
        &circle(x, y, outer - 1.5),
        &solid(rgba(0x7c828b, 150)),
        3.0,
    );
    d.part("face");
    d.stroke(
        Layer::Over,
        &circle(x, y, print_in),
        &solid(rgba(0x7c828b, 80)),
        4.0,
    );
    d.stroke(
        Layer::Over,
        &circle(x, y, print_out),
        &solid(rgba(0x7c828b, 60)),
        3.0,
    );
}
