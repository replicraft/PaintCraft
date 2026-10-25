//! Independent selection owner used only on explicitly supplied isolated servers.
use super::*;
struct Payload {
    window: Window,
    property: Atom,
    type_: Atom,
    format: u8,
    bytes: Vec<u8>,
    position: usize,
}
pub(super) fn serve(
    display: &str,
    selection: &[u8],
    offer: Offer,
    log: Arc<Mutex<Vec<String>>>,
    stop: Arc<AtomicBool>,
    ready: mpsc::Sender<()>,
    image: mpsc::Sender<()>,
) {
    let (conn, screen) = x11rb::connect(Some(display)).unwrap();
    let intern = |name: &[u8]| conn.intern_atom(false, name).unwrap().reply().unwrap().atom;
    let (selection, targets, timestamp, png, tiff, incr) = (
        intern(selection),
        intern(b"TARGETS"),
        intern(b"TIMESTAMP"),
        intern(b"image/png"),
        intern(b"image/tiff"),
        intern(b"INCR"),
    );
    let window = conn.generate_id().unwrap();
    conn.create_window(
        0,
        window,
        conn.setup().roots[screen].root,
        0,
        0,
        1,
        1,
        0,
        WindowClass::INPUT_ONLY,
        0,
        &CreateWindowAux::new(),
    )
    .unwrap()
    .check()
    .unwrap();
    conn.set_selection_owner(window, selection, x11rb::CURRENT_TIME)
        .unwrap()
        .check()
        .unwrap();
    assert_eq!(
        conn.get_selection_owner(selection)
            .unwrap()
            .reply()
            .unwrap()
            .owner,
        window
    );
    ready.send(()).unwrap();
    let mut active: Option<Payload> = None;
    let mut stamp = 42_u32;
    while !stop.load(Ordering::Acquire) {
        let Some(event) = conn.poll_for_event().unwrap() else {
            thread::park_timeout(Duration::from_millis(2));
            continue;
        };
        match event {
            Event::SelectionRequest(req) if req.selection == selection => {
                let name = if req.target == targets {
                    "TARGETS"
                } else if req.target == timestamp {
                    "TIMESTAMP"
                } else if req.target == png {
                    "image/png"
                } else if req.target == tiff {
                    "image/tiff"
                } else {
                    "unexpected"
                };
                log.lock().unwrap().push(name.into());
                let is_image = req.target == png || req.target == tiff;
                let (type_, format, bytes) = if req.target == targets {
                    let mut list = vec![targets, timestamp];
                    if offer.png.is_some() {
                        list.push(png);
                    }
                    if offer.tiff.is_some() {
                        list.push(tiff);
                    }
                    (
                        u32::from(AtomEnum::ATOM),
                        32,
                        list.into_iter().flat_map(u32::to_ne_bytes).collect(),
                    )
                } else if req.target == timestamp {
                    (
                        u32::from(AtomEnum::INTEGER),
                        32,
                        stamp.to_ne_bytes().to_vec(),
                    )
                } else if is_image {
                    if offer.changed_stamp {
                        stamp += 1;
                    }
                    (
                        req.target,
                        8,
                        if req.target == png {
                            offer.png.clone().unwrap()
                        } else {
                            offer.tiff.clone().unwrap()
                        },
                    )
                } else {
                    panic!("Unexpected selection target")
                };
                let refused = is_image && offer.refuse_image;
                if !refused {
                    if (is_image && offer.incremental)
                        || (req.target == targets && offer.incr_targets)
                    {
                        conn.change_window_attributes(
                            req.requestor,
                            &ChangeWindowAttributesAux::new()
                                .event_mask(EventMask::PROPERTY_CHANGE),
                        )
                        .unwrap();
                        conn.change_property32(
                            PropMode::REPLACE,
                            req.requestor,
                            req.property,
                            incr,
                            &[bytes.len() as u32],
                        )
                        .unwrap();
                        active = Some(Payload {
                            window: req.requestor,
                            property: req.property,
                            type_,
                            format,
                            bytes,
                            position: 0,
                        });
                    } else {
                        conn.change_property(
                            PropMode::REPLACE,
                            req.requestor,
                            req.property,
                            type_,
                            format,
                            (bytes.len() / (usize::from(format) / 8)) as u32,
                            &bytes,
                        )
                        .unwrap();
                    }
                }
                conn.send_event(
                    false,
                    req.requestor,
                    EventMask::NO_EVENT,
                    SelectionNotifyEvent {
                        response_type: SELECTION_NOTIFY_EVENT,
                        sequence: 0,
                        time: req.time,
                        requestor: req.requestor,
                        selection,
                        target: req.target,
                        property: if refused { x11rb::NONE } else { req.property },
                    },
                )
                .unwrap();
                if is_image && offer.lose_owner {
                    conn.set_selection_owner(x11rb::NONE, selection, x11rb::CURRENT_TIME)
                        .unwrap();
                }
                conn.flush().unwrap();
                if is_image {
                    let _ = image.send(());
                }
            }
            Event::PropertyNotify(event) if event.state == Property::DELETE => {
                let Some(payload) = active.as_mut() else {
                    continue;
                };
                if event.window != payload.window
                    || event.atom != payload.property
                    || (offer.stall && payload.format == 8)
                {
                    continue;
                }
                let end = (payload.position + 16 * 1024).min(payload.bytes.len());
                let bytes = &payload.bytes[payload.position..end];
                conn.change_property(
                    PropMode::REPLACE,
                    payload.window,
                    payload.property,
                    payload.type_,
                    payload.format,
                    (bytes.len() / (usize::from(payload.format) / 8)) as u32,
                    bytes,
                )
                .unwrap();
                conn.flush().unwrap();
                if bytes.is_empty() {
                    active = None;
                } else {
                    payload.position = end;
                }
            }
            _ => {}
        }
    }
}
