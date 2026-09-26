//! Linux's drives: the devices GNOME's and KDE's icon themes draw, each as FolderSkin draws it.
//!
//! The freedesktop icon names a file manager asks for (docs/DRIVES.md) cover more kinds than the
//! Mac or Windows draw: a spinning disk, a solid-state one, a disk on a USB cable, a USB stick, a
//! memory card, a disc drive and a disc, a server, a network folder and a RAID set. They are drawn
//! flat and clean, the way Adwaita and Breeze draw devices: light fronts, a darker band with vents
//! or a light, soft gradients and crisp edges. The face is each device's front or its label.

use super::draw::*;
use super::DriveKind;
const LIGHT: [[u8; 4]; 2] = [rgb(0xf8f9fa), rgb(0xdde1e5)];
const DARK: [[u8; 4]; 2] = [rgb(0x535a61), rgb(0x2f3439)];
/// The light a device shows, in the blue both desktops use for it.
const BLUE: [u8; 4] = rgb(0x3daee9);

pub(super) fn draw(kind: DriveKind, d: &mut Drawing) {
    match kind {
        DriveKind::SolidState => solid_state(d),
        DriveKind::External => usb_disk(d),
        DriveKind::Removable | DriveKind::DiskImage => stick(d),
        DriveKind::Card => card(d),
        DriveKind::OpticalDrive => disc_drive(d),
        DriveKind::Optical => disc(d),
        DriveKind::Server => server(d),
        DriveKind::Network => network_folder(d),
        DriveKind::MultiDisk => multi_disk(d),
        _ => {
            disk(d, 108.0, 236.0, 916.0, 790.0, Layer::Body, true);
        }
    }
}

/// A hard disk from `(x0, y0)` to `(x1, y1)`: a light front with screws at its corners over a dark
/// band with vents and a light. Its front is the face when `face` says so.
fn disk(d: &mut Drawing, x0: f32, y0: f32, x1: f32, y1: f32, layer: Layer, face: bool) {
    let r = 40.0;
    let band = y1 - (y1 - y0) * 0.18;
    let whole = rrect(x0, y0, x1, y1, r);
    d.fill(
        layer,
        &whole,
        &down(band, y1, &[(0.0, DARK[0]), (1.0, DARK[1])]),
    );
    let front = rrect4(x0, y0, x1, band, [r, r, 0.0, 0.0]);
    d.fill(
        layer,
        &front,
        &down(y0, band, &[(0.0, LIGHT[0]), (1.0, LIGHT[1])]),
    );
    // Vents on the left of the band and its light on the right.
    let (vy0, vy1) = (band + (y1 - band) * 0.3, y1 - (y1 - band) * 0.3);
    for i in 0..6 {
        let vx = x0 + 46.0 + i as f32 * 40.0;
        d.fill(
            layer,
            &rrect(vx, vy0, vx + 20.0, vy1, 8.0),
            &solid(rgba(0x000000, 90)),
        );
    }
    let (lx, ly) = (x1 - 58.0, (band + y1) / 2.0);
    d.fill(
        layer,
        &circle(lx, ly, 30.0),
        &radial(
            lx,
            ly,
            30.0,
            &[(0.0, fade(BLUE, 0.55)), (1.0, fade(BLUE, 0.0))],
        ),
    );
    d.fill(layer, &circle(lx, ly, 12.0), &solid(rgb(0x8fd6fb)));
    d.edge(&whole);
    let over = if face {
        d.set_face(&front);
        Layer::Over
    } else {
        layer
    };
    // Over the front: its top lit, the seam into the band, and a screw in each corner.
    let top = open_polygon(&[(x0, band), (x0, y0), (x1, y0), (x1, band)], r);
    d.rim_ink(
        over,
        &top,
        &front,
        &fading(y0 + 20.0, y0 + (band - y0) * 0.7, rgba(0xffffff, 200)),
        5.0,
    );
    d.fill_in(
        over,
        &front,
        &down(
            band - 40.0,
            band,
            &[(0.0, rgba(0x000000, 0)), (1.0, rgba(0x000000, 40))],
        ),
        &front,
    );
    d.stroke(
        over,
        &polyline(&[(x0 + 2.0, band), (x1 - 2.0, band)]),
        &solid(rgba(0x000000, 110)),
        4.0,
    );
    // A fine edge all round, so a light front keeps its top on a white window.
    d.stroke(over, &whole, &solid(rgba(0x1d2530, 40)), 3.0);
    let inset = 40.0;
    for (sx, sy) in [
        (x0 + inset, y0 + inset),
        (x1 - inset, y0 + inset),
        (x0 + inset, band - inset),
        (x1 - inset, band - inset),
    ] {
        d.fill(
            over,
            &circle(sx, sy, 15.0),
            &down(
                sy - 15.0,
                sy + 15.0,
                &[(0.0, rgb(0xe2e5e8)), (1.0, rgb(0x9aa0a7))],
            ),
        );
        d.stroke(
            over,
            &polyline(&[(sx - 8.0, sy + 8.0), (sx + 8.0, sy - 8.0)]),
            &solid(rgba(0x3a3f45, 170)),
            4.5,
        );
    }
}

/// A solid-state drive as the card it is now: a dark board with its label across most of it, the
/// gold fingers of its connector along one end with the key cut between them, and at the other
/// end the half-round notch its screw holds. A dark case around a light label would read as a
/// tablet; the board's fingers and notch read as nothing but a drive.
fn solid_state(d: &mut Drawing) {
    let (x0, y0, x1, y1) = (96.0, 312.0, 928.0, 712.0);
    let (r, cy, notch) = (18.0, (y0 + y1) / 2.0, 34.0);
    // The key: a slot cut into the connector's end, a third of the way down.
    let (ky0, ky1, kd) = (y0 + 118.0, y0 + 146.0, 46.0);
    let board = {
        let k = 0.5523 * notch;
        let mut pb = tiny_skia::PathBuilder::new();
        pb.move_to(x0 + r, y0);
        pb.line_to(x1 - r, y0);
        pb.quad_to(x1, y0, x1, y0 + r);
        pb.line_to(x1, ky0);
        pb.line_to(x1 - kd, ky0);
        pb.line_to(x1 - kd, ky1);
        pb.line_to(x1, ky1);
        pb.line_to(x1, y1 - r);
        pb.quad_to(x1, y1, x1 - r, y1);
        pb.line_to(x0 + r, y1);
        pb.quad_to(x0, y1, x0, y1 - r);
        pb.line_to(x0, cy + notch);
        pb.cubic_to(x0 + k, cy + notch, x0 + notch, cy + k, x0 + notch, cy);
        pb.cubic_to(x0 + notch, cy - k, x0 + k, cy - notch, x0, cy - notch);
        pb.line_to(x0, y0 + r);
        pb.quad_to(x0, y0, x0 + r, y0);
        pb.close();
        pb.finish().expect("the board")
    };
    d.fill(
        Layer::Body,
        &board,
        &down(y0, y1, &[(0.0, rgb(0x3d444b)), (1.0, rgb(0x22262a))]),
    );
    d.rim(Layer::Body, &board, &board, rgba(0xffffff, 60), 4.0);
    d.edge(&board);
    // The gold fingers, one after another down the connector's end, and none across the key.
    let from = x1 - 64.0;
    let mut y = y0 + 22.0;
    while y + 12.0 <= y1 - 22.0 {
        if y + 12.0 < ky0 - 6.0 || y > ky1 + 6.0 {
            d.fill(
                Layer::Body,
                &rrect(from, y, x1 - 6.0, y + 12.0, 3.0),
                &across(
                    from,
                    x1,
                    &[
                        (0.0, rgb(0xc9982e)),
                        (0.5, rgb(0xf2d27a)),
                        (1.0, rgb(0xd6a940)),
                    ],
                ),
            );
        }
        y += 22.0;
    }
    // Two small parts on the board between the label and the fingers.
    for (py0, py1) in [(y0 + 70.0, y0 + 102.0), (y1 - 150.0, y1 - 118.0)] {
        d.fill(
            Layer::Body,
            &rrect(from - 58.0, py0, from - 22.0, py1, 4.0),
            &down(py0, py1, &[(0.0, rgb(0x5b636b)), (1.0, rgb(0x33393f))]),
        );
    }
    let label = rrect(x0 + 60.0, y0 + 34.0, from - 76.0, y1 - 34.0, 12.0);
    d.fill(
        Layer::Body,
        &label,
        &down(y0 + 34.0, y1 - 34.0, &[(0.0, LIGHT[0]), (1.0, LIGHT[1])]),
    );
    d.set_face(&label);
    d.rim(Layer::Over, &label, &label, rgba(0x000000, 70), 4.0);
    d.rim_ink(
        Layer::Over,
        &open_polygon(
            &[
                (x0 + 60.0, y1 - 34.0),
                (x0 + 60.0, y0 + 34.0),
                (from - 76.0, y0 + 34.0),
                (from - 76.0, y1 - 34.0),
            ],
            12.0,
        ),
        &label,
        &fading(y0 + 50.0, y1 - 80.0, rgba(0xffffff, 170)),
        4.0,
    );
}

/// A hard disk on a USB cable.
fn usb_disk(d: &mut Drawing) {
    // The cable from the disk's right side down to its plug, under the disk.
    let cable = {
        let mut pb = tiny_skia::PathBuilder::new();
        pb.move_to(820.0, 620.0);
        pb.cubic_to(940.0, 620.0, 950.0, 720.0, 872.0, 790.0);
        pb.finish().expect("a curve")
    };
    d.stroke(Layer::Body, &cable, &solid(rgb(0x3b4046)), 30.0);
    let plug = rrect(826.0, 776.0, 918.0, 884.0, 16.0);
    d.fill(
        Layer::Body,
        &plug,
        &down(776.0, 884.0, &[(0.0, rgb(0x555b62)), (1.0, rgb(0x33373c))]),
    );
    let tip = rrect(842.0, 880.0, 902.0, 944.0, 6.0);
    d.fill(
        Layer::Body,
        &tip,
        &across(
            842.0,
            902.0,
            &[
                (0.0, rgb(0xaeb4bb)),
                (0.5, rgb(0xeef0f2)),
                (1.0, rgb(0xa2a8af)),
            ],
        ),
    );
    d.fill(
        Layer::Body,
        &rrect(856.0, 900.0, 888.0, 922.0, 3.0),
        &solid(rgb(0x5a6068)),
    );
    d.edge(&plug);
    disk(d, 100.0, 184.0, 852.0, 690.0, Layer::Body, true);
}

/// A USB stick standing up: its metal connector at the top, a body of blue plastic whose far end is
/// round with a hole for a key ring through it, and a label on its front. The round end, the ring
/// hole in it and the plastic around the label are what keep it a stick and not a phone, which a
/// dark case around a picture looks like.
fn stick(d: &mut Drawing) {
    let plug = rrect(420.0, 100.0, 604.0, 300.0, 12.0);
    d.fill(
        Layer::Body,
        &plug,
        &across(
            420.0,
            604.0,
            &[
                (0.0, rgb(0xb3b9c0)),
                (0.45, rgb(0xf4f5f7)),
                (1.0, rgb(0xa9afb6)),
            ],
        ),
    );
    for hx in [452.0, 530.0] {
        d.fill(
            Layer::Body,
            &rrect(hx, 146.0, hx + 42.0, 190.0, 6.0),
            &solid(rgb(0x4c5258)),
        );
    }
    d.edge(&plug);
    // The body, with the ring hole through its round end.
    let (x0, x1, top, bottom) = (356.0, 668.0, 282.0, 944.0);
    let end = (x1 - x0) / 2.0;
    let body = {
        let mut pb = tiny_skia::PathBuilder::new();
        pb.push_path(&rrect4(x0, top, x1, bottom, [30.0, 30.0, end, end]));
        circle_path(&mut pb, 512.0, bottom - 70.0, 24.0, false);
        pb.finish().expect("the body")
    };
    d.fill(
        Layer::Body,
        &body,
        &down(top, bottom, &[(0.0, rgb(0x4fb8ee)), (1.0, rgb(0x1f7cc4))]),
    );
    // The plastic is rounded: lit down its left side and in shadow down its right.
    d.fill_in(
        Layer::Body,
        &body,
        &across(
            x0,
            x1,
            &[
                (0.0, rgba(0xffffff, 70)),
                (0.18, rgba(0xffffff, 0)),
                (0.8, rgba(0x000000, 0)),
                (1.0, rgba(0x000000, 60)),
            ],
        ),
        &body,
    );
    d.rim(Layer::Body, &body, &body, rgba(0xffffff, 110), 5.0);
    // The ring hole's wall, in shadow.
    d.stroke(
        Layer::Body,
        &circle(512.0, bottom - 70.0, 26.0),
        &solid(rgba(0x0b3d66, 120)),
        5.0,
    );
    d.edge(&body);
    let label = rrect(392.0, 330.0, 632.0, 800.0, 22.0);
    d.fill(
        Layer::Body,
        &label,
        &down(330.0, 800.0, &[(0.0, LIGHT[0]), (1.0, LIGHT[1])]),
    );
    d.set_face(&label);
    d.rim(Layer::Over, &label, &label, rgba(0x000000, 70), 4.0);
    d.rim_ink(
        Layer::Over,
        &open_polygon(
            &[
                (392.0, 800.0),
                (392.0, 330.0),
                (632.0, 330.0),
                (632.0, 800.0),
            ],
            22.0,
        ),
        &label,
        &fading(350.0, 640.0, rgba(0xffffff, 160)),
        5.0,
    );
}

/// A memory card, flat, with grooves along its top and a label.
fn card(d: &mut Drawing) {
    let (x0, y0, x1, y1) = (232.0, 108.0, 792.0, 904.0);
    let body = polygon(
        &[
            (x0, y0),
            (x1 - 136.0, y0),
            (x1, y0 + 136.0),
            (x1, y1),
            (x0, y1),
        ],
        38.0,
    );
    d.fill(
        Layer::Body,
        &body,
        &down(y0, y1, &[(0.0, rgb(0x5d646c)), (1.0, rgb(0x3a3f45))]),
    );
    for i in 0..5 {
        let gx = x0 + 60.0 + i as f32 * 60.0;
        d.fill(
            Layer::Body,
            &rrect(gx, y0 + 48.0, gx + 28.0, y0 + 142.0, 10.0),
            &solid(rgba(0xffffff, 40)),
        );
    }
    d.edge(&body);
    let label = rrect(x0 + 40.0, y0 + 190.0, x1 - 40.0, y1 - 40.0, 22.0);
    d.fill(
        Layer::Body,
        &label,
        &down(
            y0 + 190.0,
            y1 - 40.0,
            &[(0.0, rgb(0xf3f5f7)), (1.0, rgb(0xd9dde2))],
        ),
    );
    d.fill(
        Layer::Body,
        &rrect(x0 + 40.0, y0 + 190.0, x1 - 40.0, y0 + 250.0, 22.0),
        &down(y0 + 190.0, y0 + 250.0, &[(0.0, BLUE), (1.0, rgb(0x1d8fd8))]),
    );
    d.set_face(&label);
    d.rim(Layer::Over, &label, &label, rgba(0x000000, 60), 4.0);
}

/// A disc drive: a light top over a front with the tray's slot and the eject button.
fn disc_drive(d: &mut Drawing) {
    let (x0, y0, x1, y1) = (92.0, 300.0, 932.0, 720.0);
    let band = 594.0;
    let whole = rrect(x0, y0, x1, y1, 36.0);
    d.fill(
        Layer::Body,
        &whole,
        &down(band, y1, &[(0.0, DARK[0]), (1.0, DARK[1])]),
    );
    let top = rrect4(x0, y0, x1, band, [36.0, 36.0, 0.0, 0.0]);
    d.fill(
        Layer::Body,
        &top,
        &down(y0, band, &[(0.0, LIGHT[0]), (1.0, LIGHT[1])]),
    );
    let slot = rrect(150.0, 642.0, 700.0, 666.0, 12.0);
    d.fill(Layer::Body, &slot, &solid(rgb(0x1b1e21)));
    d.stroke(
        Layer::Body,
        &polyline(&[(160.0, 670.0), (690.0, 670.0)]),
        &solid(rgba(0xffffff, 50)),
        3.0,
    );
    let button = rrect(756.0, 628.0, 872.0, 680.0, 16.0);
    d.fill(
        Layer::Body,
        &button,
        &down(628.0, 680.0, &[(0.0, rgb(0x7a8189)), (1.0, rgb(0x5a6168))]),
    );
    let eject = join(&[
        polygon(&[(814.0, 638.0), (838.0, 660.0), (790.0, 660.0)], 3.0),
        rrect(790.0, 664.0, 838.0, 670.0, 2.0),
    ]);
    d.fill(Layer::Body, &eject, &solid(rgb(0xe6e9ec)));
    d.edge(&whole);
    d.set_face(&top);
    d.rim_ink(
        Layer::Over,
        &open_polygon(&[(x0, band), (x0, y0), (x1, y0), (x1, band)], 36.0),
        &top,
        &fading(y0 + 20.0, y0 + (band - y0) * 0.7, rgba(0xffffff, 200)),
        5.0,
    );
    d.fill_in(
        Layer::Over,
        &top,
        &down(
            band - 36.0,
            band,
            &[(0.0, rgba(0x000000, 0)), (1.0, rgba(0x000000, 40))],
        ),
        &top,
    );
    d.stroke(
        Layer::Over,
        &polyline(&[(x0 + 2.0, band), (x1 - 2.0, band)]),
        &solid(rgba(0x000000, 110)),
        4.0,
    );
    d.stroke(Layer::Over, &whole, &solid(rgba(0x1d2530, 40)), 3.0);
}

/// A disc, drawn flat, printable between its hub and its rim.
fn disc(d: &mut Drawing) {
    let (x, y) = (512.0, 512.0);
    let (outer, print_out, print_in, hub, hole) = (424.0, 408.0, 160.0, 146.0, 56.0);
    d.fill(Layer::Body, &ring(x, y, outer, hole), &solid(rgb(0xe9ecef)));
    let band = ring(x, y, print_out, print_in);
    d.fill(
        Layer::Body,
        &band,
        &sweep(
            x,
            y,
            30.0,
            &[
                (0.0, rgb(0xe7eaee)),
                (0.12, rgb(0xa9d8f7)),
                (0.25, rgb(0xc7b8f2)),
                (0.37, rgb(0xf6c1d8)),
                (0.5, rgb(0xe7eaee)),
                (0.62, rgb(0xa9e8d0)),
                (0.75, rgb(0xf7e2a3)),
                (0.87, rgb(0xa9d8f7)),
                (1.0, rgb(0xe7eaee)),
            ],
        ),
    );
    d.set_face(&band);
    d.set_face_point(x, y - (print_out + print_in) / 2.0);
    d.fill(Layer::Body, &ring(x, y, hub, hole), &solid(rgb(0xf6f7f8)));
    d.stroke(
        Layer::Body,
        &circle(x, y, 100.0),
        &solid(rgba(0x8a9099, 80)),
        6.0,
    );
    d.stroke(
        Layer::Body,
        &circle(x, y, hole + 2.0),
        &solid(rgba(0x4d535a, 160)),
        4.0,
    );
    d.edge(&circle(x, y, outer));
    d.edge(&circle(x, y, hole));
    d.fill(
        Layer::Over,
        &ring(x, y, outer, print_out),
        &solid(rgba(0xf4f6f8, 230)),
    );
    d.stroke(
        Layer::Over,
        &circle(x, y, outer - 1.5),
        &solid(rgba(0x6f767e, 170)),
        3.0,
    );
    d.stroke(
        Layer::Over,
        &circle(x, y, print_in),
        &solid(rgba(0x6f767e, 90)),
        4.0,
    );
}

/// A server: three units in a rack, each held by the ears either side of it. The top unit is the
/// tall one, its light front the face over a dark band of vents and lights, and the two under it
/// are the thin dark ones with their own. A tall dark case with a light panel read as a phone; a
/// stack of units with lights down their right reads as nothing but a server, even at 16 px.
fn server(d: &mut Drawing) {
    let (x0, x1, r) = (124.0, 900.0, 26.0);
    let units = [(116.0, 596.0), (624.0, 748.0), (776.0, 900.0)];
    // The ears first, under the units, each with the hole its screw goes through.
    for &(y0, y1) in &units {
        let holes: &[f32] = if y1 - y0 > 200.0 {
            &[y0 + 64.0, y1 - 64.0]
        } else {
            &[(y0 + y1) / 2.0]
        };
        for (ex0, ex1, hx) in [
            (x0 - 36.0, x0 + 20.0, x0 - 16.0),
            (x1 - 20.0, x1 + 36.0, x1 + 16.0),
        ] {
            let ear = rrect(ex0, y0 + 12.0, ex1, y1 - 12.0, 10.0);
            d.fill(
                Layer::Body,
                &ear,
                &down(y0, y1, &[(0.0, rgb(0x7a8189)), (1.0, rgb(0x565c63))]),
            );
            d.edge(&ear);
            for &hy in holes {
                d.fill(Layer::Body, &circle(hx, hy, 9.0), &solid(rgb(0x24282c)));
            }
        }
    }
    // Each unit's lights, down its right: a blue one and a green one.
    let lights = |d: &mut Drawing, y: f32| {
        for (lx, lit) in [(x1 - 56.0, rgb(0x8fd6fb)), (x1 - 104.0, rgb(0x7ee08f))] {
            d.fill(
                Layer::Body,
                &circle(lx, y, 26.0),
                &radial(lx, y, 26.0, &[(0.0, fade(lit, 0.5)), (1.0, fade(lit, 0.0))]),
            );
            d.fill(Layer::Body, &circle(lx, y, 10.0), &solid(lit));
        }
    };
    let vents = |d: &mut Drawing, y0: f32, y1: f32| {
        for i in 0..7 {
            let vx = x0 + 48.0 + i as f32 * 38.0;
            d.fill(
                Layer::Body,
                &rrect(vx, y0, vx + 18.0, y1, 7.0),
                &solid(rgba(0x000000, 100)),
            );
        }
    };
    // The two thin units.
    for &(y0, y1) in &units[1..] {
        let unit = rrect(x0, y0, x1, y1, r);
        d.fill(
            Layer::Body,
            &unit,
            &down(y0, y1, &[(0.0, DARK[0]), (1.0, DARK[1])]),
        );
        d.rim_ink(
            Layer::Body,
            &unit,
            &unit,
            &fading(y0, y0 + 50.0, rgba(0xffffff, 70)),
            4.0,
        );
        vents(d, y0 + 38.0, y1 - 38.0);
        lights(d, (y0 + y1) / 2.0);
        d.edge(&unit);
    }
    // The tall one: its front over its band.
    let (y0, y1) = units[0];
    let band = y1 - 92.0;
    let whole = rrect(x0, y0, x1, y1, r);
    d.fill(
        Layer::Body,
        &whole,
        &down(band, y1, &[(0.0, DARK[0]), (1.0, DARK[1])]),
    );
    vents(d, band + 30.0, y1 - 30.0);
    lights(d, (band + y1) / 2.0);
    let front = rrect4(x0, y0, x1, band, [r, r, 0.0, 0.0]);
    d.fill(
        Layer::Body,
        &front,
        &down(y0, band, &[(0.0, LIGHT[0]), (1.0, LIGHT[1])]),
    );
    d.edge(&whole);
    d.set_face(&front);
    d.rim_ink(
        Layer::Over,
        &open_polygon(&[(x0, band), (x0, y0), (x1, y0), (x1, band)], r),
        &front,
        &fading(y0 + 20.0, y0 + (band - y0) * 0.7, rgba(0xffffff, 200)),
        5.0,
    );
    d.fill_in(
        Layer::Over,
        &front,
        &down(
            band - 40.0,
            band,
            &[(0.0, rgba(0x000000, 0)), (1.0, rgba(0x000000, 40))],
        ),
        &front,
    );
    d.stroke(
        Layer::Over,
        &polyline(&[(x0 + 2.0, band), (x1 - 2.0, band)]),
        &solid(rgba(0x000000, 110)),
        4.0,
    );
    d.stroke(Layer::Over, &whole, &solid(rgba(0x1d2530, 40)), 3.0);
}

/// A folder on the network: the Linux folder, as FolderSkin draws it for the folder look
/// ([`crate::geometry_linux`]), with a network mark on its corner.
fn network_folder(d: &mut Drawing) {
    use crate::geometry_linux as l;
    let back = l::back_panel_path(1.0);
    let front = l::front_panel_path(1.0);
    d.fill(
        Layer::Body,
        &back,
        &down(
            l::TAB_TOP,
            l::BOTTOM,
            &[(0.0, rgb(0x3f82d8)), (1.0, rgb(0x2a62b0))],
        ),
    );
    d.rim(
        Layer::Body,
        &l::back_top_edge_path(1.0),
        &back,
        rgba(0xffffff, 60),
        4.0,
    );
    d.rim(
        Layer::Body,
        &l::front_top_edge_path(1.0),
        &back,
        rgba(0x000000, 34),
        16.0,
    );
    d.edge(&back);
    d.fill(
        Layer::Body,
        &front,
        &down(
            l::FRONT_TOP,
            l::BOTTOM,
            &[(0.0, rgb(0x78b8f7)), (1.0, rgb(0x4592e6))],
        ),
    );
    d.set_face(&front);
    d.set_face_point(420.0, 560.0);
    d.edge(&front);
    d.rim(
        Layer::Over,
        &l::front_top_edge_path(1.0),
        &front,
        rgba(0xffffff, 150),
        5.0,
    );
    // The network mark: three joined nodes on a white disc, on the front's bottom right corner.
    let (bx, by, br) = (l::RIGHT - 124.0, l::BOTTOM - 128.0, 118.0);
    d.fill(
        Layer::Over,
        &circle(bx, by + 6.0, br + 4.0),
        &solid(rgba(0x0b2a55, 60)),
    );
    d.fill(
        Layer::Over,
        &circle(bx, by, br),
        &down(
            by - br,
            by + br,
            &[(0.0, rgb(0xffffff)), (1.0, rgb(0xe6edf5))],
        ),
    );
    let nodes = [
        (bx, by - 50.0),
        (bx - 50.0, by + 38.0),
        (bx + 50.0, by + 38.0),
    ];
    let links = outline_of(&polyline(&[nodes[1], nodes[0], nodes[2], nodes[1]]), 16.0);
    d.fill(Layer::Over, &links, &solid(rgb(0x2a62b0)));
    for (nx, ny) in nodes {
        d.fill(Layer::Over, &circle(nx, ny, 26.0), &solid(rgb(0x2a62b0)));
    }
}

/// A RAID set: one disk in front of another.
fn multi_disk(d: &mut Drawing) {
    disk(d, 200.0, 136.0, 912.0, 560.0, Layer::Body, false);
    disk(d, 112.0, 352.0, 824.0, 884.0, Layer::Body, true);
}
