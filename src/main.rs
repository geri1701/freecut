mod models;
use {
    fltk::{
        app,
        browser::Browser,
        button::Button,
        dialog::{alert_default, message_default},
        draw,
        enums::{CallbackTrigger, Color, Cursor, Event, Font, FrameType},
        frame::Frame,
        group::{Flex, FlexType, Wizard},
        image::SvgImage,
        input::{Input, InputType},
        menu::{Choice, MenuButton, MenuButtonType},
        misc::Tooltip,
        prelude::*,
        text::{TextBuffer, TextDisplay},
        window::Window,
    },
    std::{cell::RefCell, rc::Rc},
};

pub const NAME: &str = "FreeCut";
const PAD: i32 = 10;
const HEIGHT: i32 = PAD * 3;
const WIDTH: i32 = HEIGHT * 3;

fn main() -> Result<(), FltkError> {
    let app = app::App::default();
    app::set_scheme(app::Scheme::Base);
    app::set_frame_type2(FrameType::UpBox, FrameType::ThinUpBox);
    app::set_frame_type2(FrameType::DownBox, FrameType::ThinDownBox);
    app::set_background_color(238, 232, 213);
    app::set_background2_color(253, 246, 227);
    app::set_foreground_color(7, 54, 66);
    app::set_selection_color(203, 75, 22);
    app::set_inactive_color(181, 137, 0);
    Tooltip::set_color(Color::Background2);
    Tooltip::set_text_color(Color::Foreground);
    for (color, (r, g, b)) in [
        (Color::Red, (220, 50, 47)),
        (Color::Magenta, (211, 54, 130)),
        (Color::Blue, (38, 139, 210)),
        (Color::Cyan, (42, 161, 152)),
        (Color::Green, (133, 153, 0)),
    ] {
        app::set_color(color, r, g, b);
    }
    app::set_visible_focus(false);
    let mut wgt = Window::default().with_size(360, 640).center_screen();
    wgt.set_label(NAME);
    wgt.set_xclass("freecut");
    wgt.size_range(360, 640, 0, 0);
    wgt.set_icon(Some(
        SvgImage::from_data(include_str!("../assets/logo.svg")).unwrap(),
    ));
    wgt.set_callback(move |window| {
        if app::event() == Event::Close {
            window.child(0).unwrap().do_callback();
            app::quit();
        }
    });
    wgt.make_resizable(true);
    wgt.add(&{
        let mut wgt = Wizard::default_fill();
        wgt.set_callback(move |wizard| wizard.child(0).unwrap().do_callback());
        wgt.add(&page_optimizer());
        wgt.add(&page_doc("Manual", include_str!("../README.md")));
        wgt.add(&page_doc("License", include_str!("../LICENSE")));
        wgt.end();
        wgt.handle(add_menu);
        wgt
    });
    wgt.end();
    wgt.show();
    app::set_font(Font::CourierBold);
    app.run()
}

const UPDATE: Event = Event::from_i32(404);

fn page_optimizer() -> Flex {
    const PATTERNS: [&str; 3] = ["none", "width", "length"];
    const KINDS: [&str; 2] = ["cutpiece", "stockpiece"];
    const UNITS: [&str; 3] = ["mm", "inch", "foot"];
    const ERROR_RANGE: &str = "Value is out of range!";
    const ERROR_PIECE: &str = "Add at least one stockpiece to the draft list!\nAdd at least one cutpiece to the draft list!\n";
    let state = Rc::new(RefCell::new(models::Model::default()));
    let mut wgt = Flex::default_fill().with_label("Optimizer").column();
    wgt.set_margin(PAD);
    wgt.set_pad(0);
    wgt.set_callback({
        let state = Rc::clone(&state);
        move |_| state.borrow().save()
    });
    wgt.add(&{
        let mut wgt = Flex::default_fill();
        wgt.set_margin(0);
        wgt.set_pad(PAD);
        wgt.fixed(&Frame::default(), WIDTH * 2);
        wgt.add(&{
            let mut wgt = Flex::default_fill().column();
            wgt.set_margin(0);
            wgt.set_pad(PAD);
            wgt.fixed(
                &{
                    let mut wgt = Choice::default().with_label("piece type");
                    wgt.add_choice(&KINDS.join("|"));
                    wgt.set_callback({
                        let state = Rc::clone(&state);
                        move |choice| {
                            state.borrow_mut().piece_kind(choice.value());
                            app::handle_main(UPDATE).unwrap();
                        }
                    });
                    wgt.handle({
                        let state = Rc::clone(&state);
                        move |choice, event| {
                            if event == UPDATE {
                                choice.set_value(state.borrow().piece().kind());
                            }
                            false
                        }
                    });
                    wgt.handle_event(UPDATE);
                    wgt
                },
                HEIGHT,
            );
            wgt.fixed(
                &{
                    let mut wgt = Input::default()
                        .with_label("width:")
                        .with_type(InputType::Float);
                    wgt.set_color(Color::Background2);
                    wgt.set_trigger(CallbackTrigger::Changed);
                    wgt.set_callback({
                        let state = Rc::clone(&state);
                        move |input| {
                            if let Ok(value) = input.value().parse::<f32>() {
                                if state.borrow().allowed_range(&value) {
                                    state.borrow_mut().piece_width(value);
                                    input.set_color(Color::Background2);
                                } else {
                                    input.set_color(Color::Red);
                                    alert_default(ERROR_RANGE);
                                }
                            }
                            app::handle_main(UPDATE).unwrap();
                        }
                    });
                    wgt.handle({
                        let state = Rc::clone(&state);
                        move |input, event| {
                            if event == UPDATE {
                                input.set_value(&state.borrow().piece().width());
                            }
                            false
                        }
                    });
                    wgt.handle_event(UPDATE);
                    wgt
                },
                HEIGHT,
            );
            wgt.fixed(
                &{
                    let mut wgt = Input::default()
                        .with_label("length:")
                        .with_type(InputType::Float);
                    wgt.set_color(Color::Background2);
                    wgt.set_trigger(CallbackTrigger::Changed);
                    wgt.set_callback({
                        let state = Rc::clone(&state);
                        move |input| {
                            if let Ok(value) = input.value().parse::<f32>() {
                                if state.borrow().allowed_range(&value) {
                                    state.borrow_mut().piece_length(value);
                                    input.set_color(Color::Background2);
                                } else {
                                    input.set_color(Color::Red);
                                    alert_default(ERROR_RANGE);
                                }
                            }
                            app::handle_main(UPDATE).unwrap();
                        }
                    });
                    wgt.handle({
                        let state = Rc::clone(&state);
                        move |input, event| {
                            if event == UPDATE {
                                input.set_value(&state.borrow().piece().length());
                            }
                            false
                        }
                    });
                    wgt.handle_event(UPDATE);
                    wgt
                },
                HEIGHT,
            );
            wgt.fixed(
                &{
                    let mut wgt = Input::default()
                        .with_label("amount:")
                        .with_type(InputType::Int);
                    wgt.set_color(Color::Background2);
                    wgt.set_trigger(CallbackTrigger::Changed);
                    wgt.set_callback({
                        let state = Rc::clone(&state);
                        move |input| {
                            if let Ok(value) = input.value().parse::<f32>() {
                                if state.borrow().allowed_range(&value) {
                                    state.borrow_mut().piece_amount(value as usize);
                                    input.set_color(Color::Background2);
                                } else {
                                    input.set_color(Color::Red);
                                    alert_default(ERROR_RANGE);
                                }
                            }
                            app::handle_main(UPDATE).unwrap();
                        }
                    });
                    wgt.handle({
                        let state = Rc::clone(&state);
                        move |input, event| {
                            if event == UPDATE {
                                input.set_value(&state.borrow().piece().amount());
                            }
                            false
                        }
                    });
                    wgt.handle_event(UPDATE);
                    wgt
                },
                HEIGHT,
            );
            wgt.fixed(
                &{
                    let mut wgt = Choice::default().with_label("pattern (parallel to):");
                    wgt.add_choice(&PATTERNS.join("|"));
                    wgt.set_callback({
                        let state = Rc::clone(&state);
                        move |choice| {
                            state.borrow_mut().piece_pattern(choice.value());
                            app::handle_main(UPDATE).unwrap();
                        }
                    });
                    wgt.handle({
                        let state = Rc::clone(&state);
                        move |choice, event| {
                            if event == UPDATE {
                                choice.set_value(state.borrow().piece().pattern());
                            }
                            false
                        }
                    });
                    wgt.handle_event(UPDATE);
                    wgt
                },
                HEIGHT,
            );
            wgt.fixed(
                &{
                    let mut wgt = Flex::default_fill();
                    wgt.set_margin(0);
                    wgt.set_pad(0);
                    wgt.add(&{
                        let mut wgt = Button::default().with_label("@+");
                        wgt.set_label_color(Color::Green);
                        wgt.set_tooltip("ADD PIECE");
                        wgt.set_callback({
                            let state = Rc::clone(&state);
                            move |_| {
                                state.borrow_mut().add();
                                app::handle_main(UPDATE).unwrap();
                            }
                        });
                        wgt
                    });
                    wgt.add(&{
                        let mut wgt = Button::default().with_label("@1+");
                        wgt.set_label_color(Color::Red);
                        wgt.set_tooltip("POP");
                        wgt.set_callback({
                            let state = Rc::clone(&state);
                            move |_| {
                                state.borrow_mut().pop();
                                app::handle_main(UPDATE).unwrap();
                            }
                        });
                        wgt
                    });
                    wgt.end();
                    wgt
                },
                HEIGHT,
            );
            wgt.fixed(
                &{
                    let mut wgt = Choice::default().with_label("unit");
                    wgt.add_choice(&UNITS.join("|"));
                    wgt.set_value(state.borrow().unit());
                    wgt.set_callback({
                        let state = Rc::clone(&state);
                        move |choice| {
                            state.borrow_mut().set_unit(choice.value());
                            app::handle_main(UPDATE).unwrap();
                        }
                    });
                    wgt
                },
                HEIGHT,
            );
            wgt.fixed(
                &{
                    let mut wgt = Input::default()
                        .with_label("cut_width:")
                        .with_type(InputType::Float);
                    wgt.set_color(Color::Background2);
                    wgt.set_value(&state.borrow().width());
                    wgt.set_trigger(CallbackTrigger::Changed);
                    wgt.set_callback({
                        let state = Rc::clone(&state);
                        move |input| {
                            if let Ok(value) = input.value().parse::<f32>() {
                                if (0.0..15.0).contains(&value) {
                                    state.borrow_mut().set_width(value);
                                    input.set_color(Color::Background2);
                                } else {
                                    input.set_color(Color::Red);
                                    alert_default(ERROR_RANGE);
                                }
                            }
                            app::handle_main(UPDATE).unwrap();
                        }
                    });
                    wgt
                },
                HEIGHT,
            );
            wgt.fixed(
                &{
                    let mut wgt = Choice::default().with_label("layout:");
                    wgt.add_choice("guillotine|nested");
                    wgt.set_value(state.borrow().layout());
                    wgt.set_callback({
                        let state = Rc::clone(&state);
                        move |choice| {
                            state.borrow_mut().set_layout(choice.value());
                        }
                    });
                    wgt
                },
                HEIGHT,
            );
            wgt.fixed(
                &{
                    let mut wgt = Button::default().with_label("@#circle");
                    wgt.set_tooltip("OPTIMIZE");
                    wgt.set_callback({
                        let state = Rc::clone(&state);
                        move |_| {
                            let list: Vec<i32> =
                                state.borrow().pieces().iter().map(|x| x.kind()).collect();
                            if list.len() > 1 && list.contains(&0) && list.contains(&1) {
                                message_default(&state.borrow_mut().optimize());
                            } else {
                                alert_default(ERROR_PIECE);
                            }
                            app::handle_main(UPDATE).unwrap();
                        }
                    });
                    wgt
                },
                HEIGHT,
            );
            wgt.add(&Frame::default());
            wgt.end();
            wgt
        });
        wgt
    });
    wgt.add(&Frame::default());
    wgt.add(&{
        let mut tbl = Browser::default();
        tbl.set_tooltip("Output");
        tbl.set_column_widths(&[100, 100, 120, 80, 100]);
        tbl.handle({
            let state = Rc::clone(&state);
            move |tbl, event| {
                if event == UPDATE {
                    let state = state.borrow();
                    let unit = UNITS[state.unit() as usize];
                    tbl.clear();
                    tbl.add(&format!(
                        "@uTYPE\t@uWIDTH ({unit})\t@uLENGTH ({unit})\t@uAMOUNT\t@uPATTERN"
                    ));
                    for piece in state.pieces() {
                        tbl.add(&format!(
                            "{}\t{}\t{}\t{}\t{}",
                            KINDS[piece.kind() as usize],
                            &piece.width(),
                            &piece.length(),
                            &piece.amount(),
                            PATTERNS[piece.pattern() as usize],
                        ));
                    }
                }
                false
            }
        });
        tbl.handle_event(UPDATE);
        tbl
    });
    wgt.end();
    wgt.handle(add_orientation);
    wgt.handle_event(Event::Resize);
    wgt
}

fn add_orientation(flex: &mut Flex, event: Event) -> bool {
    if event == Event::Resize {
        if let Some(window) = flex.window() {
            flex.set_type(match window.w() < window.h() {
                true => FlexType::Column,
                false => FlexType::Row,
            });
            flex.fixed(&flex.child(0).unwrap(), 11 * HEIGHT + 10 * PAD);
            flex.fixed(&flex.child(1).unwrap(), PAD);
        }
        return true;
    }
    false
}

fn page_doc(title: &str, body: &str) -> Flex {
    let mut wgt = Flex::default_fill().with_label(title);
    wgt.set_margin(PAD);
    wgt.add(&{
        let mut wgt = TextDisplay::default();
        wgt.set_buffer(TextBuffer::default());
        wgt.insert(body);
        wgt
    });
    wgt.end();
    wgt
}

fn add_menu(wizard: &mut Wizard, event: Event) -> bool {
    match event {
        Event::Push => match app::event_mouse_button() {
            app::MouseButton::Right => {
                let mut wgt = MenuButton::default();
                wgt.add_choice(
                    &(0..wizard.children())
                        .map(|x| {
                            let label = wizard.child(x).unwrap().label();
                            if wizard.try_current_widget().unwrap().label() == label {
                                format!("@->  {}", label)
                            } else {
                                format!("@-  {}", label)
                            }
                        })
                        .collect::<Vec<String>>()
                        .join("|"),
                );
                wgt.set_type(MenuButtonType::Popup3);
                wgt.set_callback({
                    let mut wizard = wizard.clone();
                    move |menu| {
                        wizard.try_current_widget().unwrap().do_callback();
                        wizard.set_current_widget(&wizard.child(menu.value()).unwrap());
                    }
                });
                wgt.popup();
                true
            }
            _ => false,
        },
        Event::Enter => {
            draw::set_cursor(Cursor::Hand);
            true
        }
        Event::Leave => {
            draw::set_cursor(Cursor::Arrow);
            true
        }
        _ => false,
    }
}
